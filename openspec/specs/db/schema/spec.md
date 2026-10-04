# db/schema Specification

## Purpose
Esquema de base de datos de Valet: tablas y migraciones para settings, mensajes enriquecidos (tokens, colapso, indexado), eventos (categoría, todo el día, recurrencia, recordatorios) y la tabla llm_requests con datos completos de uso de OpenRouter.

## Requirements

### Requirement: Tabla settings con valores por defecto

La migración SHALL crear la tabla `settings` con las columnas `key`, `value` y `updated_at`, sembrando los valores por defecto `max_window_tokens=10000` y `system_prompt`.
**Given** una base de datos recién migrada  
**When** se ejecuta `run_migrations()`  
**Then** existe la tabla `settings` con columnas `key`, `value`, `updated_at`  
**And** contiene los valores por defecto `max_window_tokens=10000` y `system_prompt`

#### Scenario: Migración crea tabla settings
**Given** una base de datos vacía  
**When** se ejecuta `run_migrations()`  
**Then** la tabla `settings` existe  
**And** `SELECT value FROM settings WHERE key='max_window_tokens'` devuelve `10000`  
**And** `SELECT value FROM settings WHERE key='system_prompt'` devuelve el prompt por defecto

### Requirement: SettingsRepo get/set/get_all

El repositorio SHALL exponer `get`, `set` y `get_all` para leer, insertar o actualizar y listar los ajustes.
**Given** un SettingsRepo sobre una conexión  
**When** se llama `get(conn, "max_window_tokens")`  
**Then** devuelve `Some("10000")` si existe, `None` si no  
**When** se llama `set(conn, "key", "value")`  
**Then** inserta o actualiza el valor  
**When** se llama `get_all(conn)`  
**Then** devuelve un HashMap con todas las claves y valores

#### Scenario: get devuelve valor existente
**Given** settings con `max_window_tokens=10000`  
**When** `get(conn, "max_window_tokens")`  
**Then** devuelve `Some("10000")`

#### Scenario: set inserta nuevo valor
**Given** settings sin clave `foo`  
**When** `set(conn, "foo", "bar")`  
**Then** `get(conn, "foo")` devuelve `Some("bar")`

#### Scenario: set actualiza valor existente
**Given** settings con `foo=bar`  
**When** `set(conn, "foo", "baz")`  
**Then** `get(conn, "foo")` devuelve `Some("baz")`

### Requirement: Tabla messages con todas las columnas desde CREATE TABLE

La migración SHALL crear la tabla `messages` con todas las columnas de enriquecimiento en un único `CREATE TABLE`.

**Given** una base de datos recién migrada  
**When** se ejecuta `run_migrations()`  
**Then** existe la tabla `messages` con todas las columnas en un solo CREATE TABLE:
`id`, `conversation_id`, `role`, `content`, `tool_calls`, `tool_results`,
`created_at`, `tokens_count`, `collapsed_content`, `collapsed_tokens_count`,
`is_indexed`, `summary_ref`

**And** no existe la migración `20260925000002_message_enrichment.sql` en disco

#### Scenario: Migración #01 incluye columnas de enrichment
**Given** una base de datos vacía  
**When** se ejecuta `run_migrations()`  
**Then** `PRAGMA table_info(messages)` contiene `tokens_count`, `collapsed_content`,
`collapsed_tokens_count`, `is_indexed`, `summary_ref`

#### Scenario: Migración #02 no existe en disco
**Given** el directorio `migrations/`  
**When** se listan los archivos  
**Then** no existe `20260925000002_message_enrichment.sql`

### Requirement: Extend events table with new fields

La migración SHALL añadir a la tabla `events` las columnas `category`, `all_day`, `rrule` y `reminder_minutes_before`, junto con sus índices.

Add category, all_day, rrule, and reminder_minutes_before to the events table.

**Contracts:**

```sql
-- Migration: 20260925000002_extend_events.sql
ALTER TABLE events ADD COLUMN category TEXT NOT NULL DEFAULT 'default';
ALTER TABLE events ADD COLUMN all_day INTEGER NOT NULL DEFAULT 0;
ALTER TABLE events ADD COLUMN rrule TEXT;  -- RRULE string para recurrencia
ALTER TABLE events ADD COLUMN reminder_minutes_before INTEGER;  -- minutos antes para notificar

CREATE INDEX IF NOT EXISTS idx_events_category ON events(category);
CREATE INDEX IF NOT EXISTS idx_events_start_time ON events(start_time);
```

```rust
// Struct extendido
pub struct Event {
    pub id: String,
    pub profile_id: String,
    pub title: String,
    pub description: Option<String>,
    pub start_time: String,
    pub end_time: String,
    pub location: Option<String>,
    pub scope: String,          // "shared" | "personal"
    pub category: String,       // "default" | "work" | "personal" | "health" | "birthday" | "holiday"
    pub all_day: bool,
    pub rrule: Option<String>,  // "FREQ=WEEKLY;BYDAY=MO,WE,FR" | "FREQ=DAILY" | "FREQ=MONTHLY"
    pub reminder_minutes_before: Option<i32>,
    pub created_at: String,
    pub updated_at: String,
}
```

**Scenarios:**

#### Scenario: Create event with category and all_day
Given the events table has the new fields
When `create_event` is called with category "work" and all_day true
Then the event is stored with category "work" and all_day = 1

#### Scenario: Create recurring event with rrule
Given the events table has the rrule field
When `create_event` is called with title "Daily standup" and rrule "FREQ=WEEKLY;BYDAY=MO,TU,WE,TH,FR"
Then the event is stored with the rrule
And the start_time/end_time represent Monday at 09:00-09:15

#### Scenario: Create event with reminder
Given the events table has reminder_minutes_before
When `create_event` is called with title "Doctor" and reminder_minutes_before = 60
Then the event is stored with reminder_minutes_before = 60

### Requirement: EventsRepo::delete method

El repositorio SHALL exponer `EventsRepo::delete` para eliminar eventos y `EventsRepo::list_by_category` para listarlos por categoría.

Add soft-delete or hard-delete for events.

**Contracts:**

```rust
impl EventsRepo {
    pub async fn delete(pool: &SqlitePool, id: &str) -> Result<(), sqlx::Error>;
    pub async fn list_by_category(pool: &SqlitePool, profile_id: &str, category: &str) -> Result<Vec<Event>, sqlx::Error>;
}
```

**Scenarios:**

#### Scenario: Delete existing event
Given an existing event with id "evt-1"
When `EventsRepo::delete` is called with id "evt-1"
Then the event is removed from the events table

#### Scenario: Delete non-existent event
Given no event with id "nonexistent"
When `EventsRepo::delete` is called with id "nonexistent"
Then it succeeds (no-op) without error

#### Scenario: List events by category
Given events with categories "work" and "personal"
When `EventsRepo::list_by_category` is called with category "work"
Then only work events are returned

### Requirement: Expand events list_by_date_range to support recurring events

`EventsRepo::list_by_date_range` SHALL expandir los eventos recurrentes (con `rrule` no nulo) generando las instancias dentro del rango consultado.

Recurring events (those with a non-null rrule) should be expanded when queried within a date range.

**Contracts:**

```rust
// Se modifica EventsRepo::list_by_date_range para expandir recurrencias.
// Por cada evento con rrule, se generan las instancias que caen dentro del rango.
// La expansion cubre: FREQ=DAILY, FREQ=WEEKLY, FREQ=MONTHLY
// Año y límite de expansión: máximo 365 instancias por evento.

fn expand_recurring(
    base_start: &str,
    base_end: &str,
    rrule: &str,
    range_start: &str,
    range_end: &str,
) -> Vec<(String, String)>;
```

**Scenarios:**

#### Scenario: List recurring event within range
Given an event "Standup" with rrule "FREQ=WEEKLY;BYDAY=MO,WE,FR" starting Monday
When `list_by_date_range` is queried for a week containing that Monday
Then the event appears 3 times (Mon, Wed, Fri)

#### Scenario: Recurring event outside range returns nothing
Given an event with rrule "FREQ=DAILY" but base dates are a year ago
When `list_by_date_range` is queried for next week
Then the event does NOT appear in results

#### Scenario: Non-recurring event still listed normally
Given a non-recurring event on Tuesday
When `list_by_date_range` is queried for that week
Then the event appears once

### Requirement: Migration SHALL create llm_requests table with full OpenRouter usage data

La migración SHALL crear la tabla `llm_requests` con todas las columnas de uso de OpenRouter y de forma idempotente.

**Given** una base de datos vacía
**When** se ejecuta la migración `20260926000002_stats.sql`
**Then** existe la tabla `llm_requests` con las siguientes columnas:

```sql
CREATE TABLE IF NOT EXISTS llm_requests (
    id                TEXT PRIMARY KEY,
    model             TEXT NOT NULL,
    provider          TEXT,
    profile_id        TEXT REFERENCES profiles(id),
    prompt_tokens     INTEGER NOT NULL DEFAULT 0,
    completion_tokens INTEGER NOT NULL DEFAULT 0,
    total_tokens      INTEGER NOT NULL DEFAULT 0,
    cached_tokens     INTEGER NOT NULL DEFAULT 0,
    reasoning_tokens  INTEGER NOT NULL DEFAULT 0,
    cost              REAL NOT NULL DEFAULT 0.0,
    is_byok           INTEGER NOT NULL DEFAULT 0,
    duration_ms       INTEGER,
    cache_hit         INTEGER NOT NULL DEFAULT 0,
    status            TEXT NOT NULL DEFAULT 'success'
                      CHECK(status IN ('success', 'error', 'timeout')),
    error_message     TEXT,
    tool_calls        TEXT,
    created_at        TEXT NOT NULL DEFAULT (datetime('now'))
);
```

#### Scenario: Migración crea tabla correctamente
**Given** base de datos sin tabla llm_requests
**When** se ejecuta la migración
**Then** `SELECT name FROM sqlite_master WHERE type='table' AND name='llm_requests'` devuelve la tabla
**And** el schema coincide con la definición anterior

#### Scenario: Migración es idempotente
**Given** la tabla llm_requests ya existe
**When** se ejecuta la migración de nuevo
**Then** no hay error
**And** la tabla sigue existiendo con el mismo schema

### Requirement: StatsRepo SHALL read retention from settings table

El repositorio SHALL leer `stats_retention_days` de settings y usar 30 como valor por defecto si la clave no existe.

**Given** la tabla `settings` con clave `stats_retention_days`
**When** se llama a `StatsRepo::get_retention_days(pool)`
**Then** devuelve el valor como `u32`
**And** si no existe la clave, devuelve 30 (default)

#### Scenario: Retention configurada
**Given** settings con `stats_retention_days = 45`
**When** `StatsRepo::get_retention_days(pool)`
**Then** devuelve 45

#### Scenario: Retention por defecto
**Given** settings sin `stats_retention_days`
**When** `StatsRepo::get_retention_days(pool)`
**Then** devuelve 30

### Requirement: Migración siembra los prompts del sistema en settings

**Given** una base de datos recién migrada
**When** se ejecuta `run_migrations()`
**Then** la tabla `settings` SHALL contener las claves `system_prompt`, `archivist_prompt` y `collapse_prompt`
**And** `system_prompt` SHALL contener el prompt de personalidad de Valet (con "asistente personal británico", "Modo Conciso (Predeterminado)", "Expandido" y "Emojis")
**And** `archivist_prompt` SHALL contener el prompt del archivista (con "archivista de memoria" y el placeholder `{{ BLOQUE_DE_MENSAJES }}`)
**And** `collapse_prompt` SHALL contener el prompt de resumen (con "Resume el siguiente texto")
**And** la migración de siembra SHALL NOT sobreescribir valores existentes no vacíos (personalizaciones del usuario)
**And** la migración de siembra SHALL rellenar valores ausentes o vacíos
**And** una migración aditiva posterior MAY anexar contenido nuevo al final del `system_prompt` sin eliminar el texto existente del usuario

#### Scenario: Base de datos nueva recibe los tres prompts
- **WHEN** se ejecuta `run_migrations()` sobre una base de datos vacía
- **THEN** `SELECT value FROM settings WHERE key='system_prompt'` devuelve un valor no vacío que contiene "asistente personal británico"
- **AND** `SELECT value FROM settings WHERE key='archivist_prompt'` devuelve un valor no vacío que contiene "{{ BLOQUE_DE_MENSAJES }}"
- **AND** `SELECT value FROM settings WHERE key='collapse_prompt'` devuelve un valor no vacío que contiene "Resume el siguiente texto"

#### Scenario: Valor vacío existente se rellena
- **GIVEN** una base de datos con `settings.system_prompt = ''`
- **WHEN** se ejecuta la migración de prompts
- **THEN** `settings.system_prompt` pasa a contener el prompt por defecto no vacío

#### Scenario: Personalización existente se respeta
- **GIVEN** una base de datos con `settings.system_prompt = 'Mi prompt personalizado'`
- **WHEN** se ejecuta la migración de prompts
- **THEN** `settings.system_prompt` sigue siendo `'Mi prompt personalizado'`

#### Scenario: Migración idempotente
- **WHEN** se ejecuta `run_migrations()` dos veces seguidas
- **THEN** no se produce error
- **AND** los tres prompts siguen presentes con un único valor por clave

### Requirement: La migración SHALL crear la tabla virtual vec_memory con vec0

La migración SHALL crear `vec_memory` como tabla virtual `vec0` con la columna de id textual **declarada explícitamente** y una columna vectorial tipada con `distance_metric=cosine`: `CREATE VIRTUAL TABLE vec_memory USING vec0(id TEXT PRIMARY KEY, embedding float[N] distance_metric=cosine)`. Sin declarar `id`, la tabla solo tendría el `rowid` implícito y no se podría hacer el JOIN por id con `memory`. `memory` SHALL seguir siendo la fuente de verdad. La dimensión `N` SHALL coincidir con `EMBEDDING_DIMENSION`.

**Given** una base de datos migrada  
**When** se inspecciona `sqlite_master`  
**Then** `vec_memory` SHALL ser una tabla virtual `vec0`  
**And** SHALL exponer la columna `id` textual como clave primaria  
**And** SHALL declarar `embedding float[N]` con `distance_metric=cosine`  
**And** `memory` SHALL conservarse como fuente de verdad, con el JOIN por `id`

#### Scenario: Tabla virtual creada con columna id explícita
**Given** una base de datos vacía  
**When** se ejecuta `run_migrations()`  
**Then** `sqlite_master` contiene `vec_memory` de tipo `table` con SQL de `vec0`  
**And** la definición incluye `id TEXT PRIMARY KEY` y `embedding float[1024] distance_metric=cosine`

#### Scenario: El JOIN por id es posible
**Given** una fila en `memory` y su vector en `vec_memory` con el mismo `id`  
**When** se ejecuta `SELECT m.id, v.distance FROM memory m JOIN vec_memory v ON m.id = v.id WHERE v.embedding MATCH ? AND k = ? ORDER BY v.distance`  
**Then** la consulta devuelve la ficha con su distancia  
**And** NO falla con `table vec_items has no column named id`

#### Scenario: Migración idempotente
**Given** una base de datos ya migrada  
**When** se ejecuta `run_migrations()` de nuevo  
**Then** no se produce error  
**And** `vec_memory` sigue existiendo como tabla virtual

#### Scenario: La dimensión declarada coincide con EMBEDDING_DIMENSION
**Given** `EMBEDDING_DIMENSION = 1024`  
**When** se ejecuta `run_migrations()`  
**Then** `vec_memory` declara `embedding float[1024]`

### Requirement: db/schema SHALL define the reminders table

La tabla `reminders` SHALL tener las columnas `id` (PK), `profile_id` (FK a `profiles(id)`), `text`,
`datetime`, `status` (enum `pending`/`dismissed`/`snoozed`, por defecto `pending`) y `created_at`.

**Given** la migración inicial aplicada
**When** se inspecciona el esquema
**Then** DEBE existir la tabla `reminders` con esas columnas y el CHECK de `status`

#### Scenario: El CHECK de status restringe los valores
**Given** la tabla `reminders`
**When** se intenta insertar `status` distinto de `pending`/`dismissed`/`snoozed`
**Then** la inserción DEBE fallar por el CHECK

### Requirement: db/schema SHALL define the tasks table with GTD statuses

La tabla `tasks` SHALL tener `id` (PK), `profile_id` (FK), `content`, `status` (enum GTD
`inbox`/`todo`/`doing`/`waiting`/`someday`/`done`, por defecto `inbox`), `priority`
(`low`/`medium`/`high`, por defecto `medium`), `project`, `due_date`, `scope`
(`shared`/`personal`, por defecto `shared`), `created_at` y `updated_at`. El estado final proviene de
la migración `20260926000001_gtd_statuses`, que recrea la tabla y mapea los estados antiguos.

**Given** las migraciones aplicadas
**When** se inspecciona `tasks`
**Then** DEBE tener el CHECK GTD y `DEFAULT 'inbox'`

#### Scenario: El CHECK GTD admite los seis estados
**Given** la tabla `tasks`
**When** se inserta con `status` en `inbox`/`todo`/`doing`/`waiting`/`someday`/`done`
**Then** la inserción DEBE aceptarse

#### Scenario: La migración GTD mapea estados antiguos
**Given** una fila previa con `status = 'completed'`
**When** se aplica `20260926000001_gtd_statuses`
**Then** su `status` DEBE quedar como `done`

### Requirement: db/schema SHALL define the notes table

La tabla `notes` SHALL tener `id` (PK), `profile_id` (FK), `content`, `category`
(`idea`/`journal`/`fact`/`todo`, por defecto `idea`), `tags`, `created_at` y `updated_at`, y SHALL
mantener los triggers FTS (`notes_ai`/`notes_ad`/`notes_au`) sobre `notes_fts`.

**Given** las migraciones aplicadas
**When** se inspecciona `notes`
**Then** DEBE tener el CHECK de `category` y los triggers FTS

#### Scenario: El CHECK de category restringe los valores
**Given** la tabla `notes`
**When** se inserta una `category` fuera del enum
**Then** la inserción DEBE fallar

#### Scenario: Los triggers mantienen el índice FTS
**Given** la tabla `notes` y `notes_fts`
**When** se inserta, actualiza o borra una nota
**Then** los triggers DEBEN sincronizar `notes_fts`

### Requirement: settings SHALL store the timezone and location keys

La tabla `settings` (clave/valor con `updated_at`) SHALL almacenar, entre otras, las claves
`timezone` (zona IANA para `get_current_time`), `latitude` y `longitude` (coordenadas para
`get_current_location`).

**Given** la tabla `settings`
**When** se leen `timezone`, `latitude` y `longitude`
**Then** DEBEN existir como claves con valor textual cuando estén configuradas

#### Scenario: Lectura de las claves de hora y ubicación
**Given** `settings` con `timezone`, `latitude` y `longitude`
**When** se consultan esas claves
**Then** DEBE devolverse su valor textual

### Requirement: La guía de uso de widgets SHALL anexarse al `system_prompt` por una migración aditiva e idempotente

Una migración SHALL anexar al final del valor de `settings.system_prompt` la sección
`# Instrucciones de Interfaz y Widgets Interactivos`, con estas reglas: un widget solo se muestra si se
**ejecuta** la llamada a `render_widget` (nunca se afirma haberlo mostrado sin invocarla); criterios de
activación (decisión entre 2 o más opciones, recolección de más de un dato, flujos paso a paso, datos
complejos o geográficos); restricción de texto simple (no invocar para explicaciones o datos directos); y
procesamiento de la respuesta del usuario sin volver a renderizar el widget salvo modificación explícita.
La migración SHALL anexar solo si la sección aún no está presente, SHALL preservar íntegro el texto
existente del usuario y SHALL NOT fallar ni crear la clave si `system_prompt` no existe o está vacío.

#### Scenario: Instalación nueva recibe la sección de widgets

**Given** una base de datos vacía
**When** se ejecuta `run_migrations()`
**Then** `settings.system_prompt` contiene la cabecera `# Instrucciones de Interfaz y Widgets Interactivos`
**And** menciona `render_widget`
**And** indica que un widget solo se muestra si se ejecuta la llamada a la herramienta
**And** enumera los criterios de activación (decisiones, recolección de datos, flujos paso a paso, datos complejos o geográficos)
**And** indica que para una explicación o un dato directo NO se invoca la herramienta

#### Scenario: Personalización existente se conserva y se le anexa la sección

**Given** una base de datos con `settings.system_prompt = 'Mi prompt personalizado'`
**When** se ejecuta la migración de guía de widgets
**Then** `settings.system_prompt` contiene `'Mi prompt personalizado'`
**And** también contiene la cabecera `# Instrucciones de Interfaz y Widgets Interactivos`
**And** el texto del usuario aparece antes que la sección

#### Scenario: Idempotente: la sección no se duplica

**When** se ejecuta `run_migrations()` dos veces seguidas
**Then** `settings.system_prompt` contiene una sola aparición de `# Instrucciones de Interfaz y Widgets Interactivos`
