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

`EventsRepo::list_by_date_range` SHALL expandir los eventos recurrentes (con `rrule` no nulo)
generando las instancias que **se solapen** con el rango consultado. `expand_recurring` SHALL recibir
los extremos base y los límites del rango como **instantes ya parseados** (`DateTime<FixedOffset>`),
de modo que un límite de fecha suelta (`YYYY-MM-DD`) también funcione. Una instancia SHALL incluirse
cuando su intervalo solape el rango (`instancia_start < range_end AND instancia_end > range_start`).
Si un extremo base no es parseable, la expansión SHALL devolver una lista vacía. Las instancias SHALL
emitirse en UTC canónico (RFC 3339 con sufijo `Z`), convertidas desde el instante real del evento
base: una base almacenada con offset (`+02:00`) NO SHALL reetiquetarse con su hora de pared.

**Contracts:**

```rust
// Se modifica EventsRepo::list_by_date_range para expandir recurrencias.
// Por cada evento con rrule, se generan las instancias que solapan el rango.
// La expansion cubre: FREQ=DAILY, FREQ=WEEKLY, FREQ=MONTHLY
// Año y límite de expansión: máximo 365 instancias por evento.

fn expand_recurring(
    base_start: &DateTime<FixedOffset>,
    base_end: &DateTime<FixedOffset>,
    rrule: &str,
    range_start: &DateTime<FixedOffset>,
    range_end: &DateTime<FixedOffset>,
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

#### Scenario: Límites de fecha suelta
**Given** un evento "Standup" con rrule "FREQ=WEEKLY;BYDAY=MO,WE,FR" que empieza un lunes
**When** `list_by_date_range` se consulta con `start = "2026-09-28"` y `end = "2026-10-04"`
**Then** el evento SHALL aparecer 3 veces (lunes, miércoles y viernes)

#### Scenario: Instancia que solapa parcialmente el rango
**Given** una instancia recurrente que empieza antes del límite inferior del rango y termina dentro de él
**When** `list_by_date_range` se consulta con ese rango
**Then** la instancia SHALL incluirse

#### Scenario: Las instancias se emiten en UTC canónico
**Given** un evento con rrule "FREQ=DAILY" cuyo start_time es "2026-10-06T19:00:00+02:00" (17:00Z) y end_time "2026-10-06T20:00:00+02:00" (18:00Z)
**When** `expand_recurring` expande un rango que contiene el 2026-10-06
**Then** la instancia de ese día SHALL tener start_time="2026-10-06T17:00:00Z" y end_time="2026-10-06T18:00:00Z"

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

### Requirement: Migration SHALL add a kind column to llm_requests

La migración `20261009000001_llm_requests_kind.sql` SHALL añadir a `llm_requests` una columna `kind` de tipo `TEXT`, `NOT NULL`, con `DEFAULT 'chat'` y `CHECK (kind IN ('chat','router','archivist','consolidator','collapse'))`. Las filas existentes SHALL quedar con `kind = 'chat'`.

#### Scenario: La columna kind existe tras migrar
- **Given** una base de datos recién migrada
- **When** se inspeccionan las columnas de `llm_requests`
- **Then** existe `kind` de tipo `TEXT`, NOT NULL, con `CHECK (kind IN ('chat','router','archivist','consolidator','collapse'))` y DEFAULT `'chat'`

#### Scenario: Las filas previas quedan como chat
- **Given** filas de `llm_requests` insertadas antes de la migración
- **When** se ejecuta la migración
- **Then** todas ellas tienen `kind = 'chat'`

#### Scenario: La migración es idempotente
- **Given** las migraciones ya aplicadas
- **When** se vuelven a ejecutar
- **Then** no falla y la columna `kind` sigue presente

### Requirement: La migración SHALL crear la tabla `timeline_events` con sus índices

Una migración nueva SHALL crear, de forma idempotente, la tabla `timeline_events` con las columnas
`id` (clave primaria), `timestamp` (ISO 8601 UTC), `category` (con un conjunto cerrado garantizado por
`CHECK`), `fact`, `source_message_id` (clave ajena a `messages(id)` con `ON DELETE SET NULL`) y
`created_at`. SHALL crear además un índice por `timestamp` descendente, un índice por `category` y un
índice parcial por `source_message_id`.

**Given** una base de datos vacía
**When** se ejecuta la migración de `timeline_events`
**Then** existe la tabla `timeline_events` con las columnas indicadas
**And** existen los índices por fecha, por categoría y por mensaje de origen

#### Scenario: La tabla se crea con su CHECK de categoría
- **Given** la migración aplicada
- **When** se inspecciona el esquema de `timeline_events`
- **Then** la columna `category` admite `sport`, `lifestyle`, `work`, `shopping`, `health`, `social` y `system`
- **And** un valor fuera de ese conjunto lo rechaza la base de datos

#### Scenario: La clave ajena se anula al borrarse el mensaje
- **Given** un hecho cuyo mensaje de origen se borra
- **When** se consulta el hecho
- **Then** la fila sigue existiendo
- **And** su `source_message_id` queda a `NULL`

#### Scenario: La migración es idempotente
- **Given** la tabla `timeline_events` ya creada
- **When** se ejecuta la migración de nuevo
- **Then** no hay error
- **And** la tabla sigue existiendo con el mismo esquema

### Requirement: La migración SHALL ampliar los valores admitidos de `llm_requests.kind` sin perder filas

Una migración nueva SHALL ampliar el `CHECK` de `llm_requests.kind` para admitir `'journal'`, además
de `'timeline'`, `'chat'`, `'router'`, `'archivist'`, `'consolidator'` y `'collapse'`. Como SQLite no
permite alterar un `CHECK` en línea, la migración SHALL reconstruir la tabla y SHALL preservar
**todas** las filas existentes, sea cual sea su `kind`.

#### Scenario: La tabla acepta el nuevo origen
- **Given** la migración aplicada
- **When** se inserta una fila de `llm_requests` con `kind = 'timeline'`
- **Then** la inserción se acepta

#### Scenario: Las filas previas sobreviven
- **Given** una base de datos con filas de los cinco valores anteriores de `kind`
- **When** se ejecuta la migración
- **Then** todas las filas siguen ahí
- **And** sus valores de `kind` no cambian

#### Scenario: Un valor fuera del conjunto sigue rechazándose
- **Given** la migración aplicada
- **When** se intenta insertar una fila con un `kind` desconocido
- **Then** la base de datos lo rechaza

#### Scenario: La migración es idempotente
- **Given** la migración ya aplicada
- **When** se ejecuta de nuevo
- **Then** no hay error
- **And** no se duplica ninguna fila

#### Scenario: La tabla acepta el origen de la bitácora
- **Given** la migración aplicada
- **When** se inserta una fila de `llm_requests` con `kind = 'journal'`
- **Then** la inserción se acepta

#### Scenario: Las filas del timeline sobreviven a la segunda reconstrucción
- **Given** una base de datos con filas de `llm_requests` de los seis valores anteriores de `kind`,
  incluidas filas con `kind = 'timeline'`
- **When** se ejecuta la migración que amplía el `CHECK` para `'journal'`
- **Then** todas las filas siguen ahí
- **And** ninguna fila con `kind = 'timeline'` se pierde ni cambia de valor

### Requirement: La migración SHALL añadir `location` y un índice único por hecho y día a `timeline_events`

Una migración nueva SHALL añadir a `timeline_events` la columna `location` —TEXT, admite `NULL`— y SHALL
crear un índice **único** sobre la pareja formada por el hecho normalizado (`lower(trim(fact))`) y el día
de su `timestamp` (`substr(timestamp, 1, 10)`). Con ese índice en pie, dos hechos con el mismo texto
normalizado el mismo día no pueden coexistir: la base de datos rechaza el segundo. Tras la migración,
`timeline_events` tiene las columnas `id`, `timestamp`, `category`, `fact`, `location`,
`source_message_id` y `created_at`. La tabla queda sin filas: la migración la vacía.

#### Scenario: La columna `location` se añade y admite nulos
- **Given** una base de datos con `timeline_events` ya creada
- **When** se ejecuta la migración
- **Then** la tabla tiene la columna `location`
- **And** un hecho sin lugar la guarda a `NULL`

#### Scenario: El índice único rechaza un hecho repetido el mismo día
- **Given** un hecho registrado con `fact = 'Cené mochis'` y fecha 2026-10-11
- **When** se intenta insertar otro con el mismo `fact` y la misma fecha
- **Then** la base de datos rechaza la segunda fila
- **And** la primera sigue intacta

#### Scenario: El mismo hecho en días distintos es un hecho distinto
- **Given** un hecho registrado con `fact = 'Cené mochis'` y fecha 2026-10-11
- **When** se inserta otro con el mismo `fact` y fecha 2026-10-12
- **Then** las dos filas coexisten

#### Scenario: La normalización ignora espacios y mayúsculas
- **Given** un hecho registrado con `fact = 'Corrí 9,34 km'` y fecha 2026-10-11
- **When** se inserta otro con `fact = '  corrí 9,34 km  '` y la misma fecha
- **Then** la base de datos rechaza la segunda fila

### Requirement: La migración SHALL vaciar `timeline_events` antes de crear el índice único

La migración SHALL eliminar **todas** las filas de `timeline_events`. Los hechos guardados hasta ahora
son derivados —los escribió el extractor, no el usuario—, sus fechas son las de la mención y no las del
acontecimiento, ninguno tiene lugar y más de un tercio son duplicados: no hay nada que conservar. El
material crudo sigue en `messages`, así que la tabla es reconstruible. El vaciado SHALL ejecutarse
**antes** de crear el índice único, que de otro modo fallaría sobre los duplicados existentes, y SHALL
NOT tocar ninguna otra tabla.

#### Scenario: La tabla queda vacía
- **Given** una base de datos con filas en `timeline_events`
- **When** se ejecuta la migración
- **Then** `timeline_events` no tiene ninguna fila

#### Scenario: El vaciado no toca los mensajes
- **Given** una base de datos con mensajes y con hechos
- **When** se ejecuta la migración
- **Then** `messages` conserva todas sus filas

#### Scenario: El vaciado deja el índice único creable
- **Given** una base de datos con hechos duplicados que harían fallar el índice único
- **When** se ejecuta la migración
- **Then** la migración termina sin error
- **And** el índice único existe

### Requirement: La migración SHALL actualizar el prompt del extractor solo si nadie lo ha personalizado

La migración SHALL reemplazar el valor de `timeline_prompt` y de `SKILL_TIMELINE_PROMPT` **solo** cuando
el valor vigente sea exactamente el sembrado por la migración `20261010000004_timeline_prompts.sql`. Si
el valor es cualquier otro —una personalización del usuario— SHALL dejarlo intacto. Una clave ausente
SHALL rellenarse con el valor nuevo.

#### Scenario: El prompt sin tocar se actualiza
- **Given** `timeline_prompt` con el valor sembrado por la migración anterior
- **When** se ejecuta la migración
- **Then** `timeline_prompt` pasa a ser el prompt nuevo

#### Scenario: Una personalización se respeta
- **Given** `timeline_prompt` con un valor distinto del sembrado
- **When** se ejecuta la migración
- **Then** `timeline_prompt` conserva su valor

#### Scenario: Una clave ausente se rellena
- **Given** un `settings` sin la clave `timeline_prompt`
- **When** se ejecuta la migración
- **Then** `timeline_prompt` existe con el prompt nuevo

### Requirement: La migración SHALL crear la tabla `push_subscriptions` para las suscripciones WebPush

Una migración nueva SHALL crear, de forma idempotente, la tabla `push_subscriptions` con las columnas
exactas `id TEXT PRIMARY KEY`, `user_id TEXT NOT NULL DEFAULT 'default'`, `endpoint TEXT NOT NULL
UNIQUE`, `p256dh TEXT NOT NULL`, `auth TEXT NOT NULL`, `user_agent TEXT`, `created_at TEXT NOT NULL
DEFAULT (datetime('now'))` y `updated_at TEXT NOT NULL DEFAULT (datetime('now'))`. La restricción
`UNIQUE` sobre `endpoint` ya crea el índice, así que SHALL NO añadirse un índice redundante sobre
`endpoint`.

#### Scenario: La migración crea la tabla

- **Given** una base de datos vacía
- **When** se ejecuta `run_migrations()`
- **Then** existe la tabla `push_subscriptions` con las columnas indicadas
- **And** la migración es idempotente al ejecutarse de nuevo

#### Scenario: Un endpoint duplicado viola UNIQUE

- **Given** una fila existente en `push_subscriptions` con un `endpoint`
- **When** se intenta insertar otra fila con el mismo `endpoint`
- **Then** la base de datos rechaza la inserción por la restricción `UNIQUE`

#### Scenario: El user_id por defecto es 'default'

- **Given** una fila insertada sin especificar `user_id`
- **When** se consulta la fila
- **Then** su `user_id` es `'default'`

### Requirement: `reminders` SHALL registrar cuándo se notificó un recordatorio

La tabla `reminders` SHALL ganar una columna nueva `notified_at TEXT` nullable, donde `NULL` significa
«sin notificar todavía».

#### Scenario: La columna existe y es nullable

- **Given** una base de datos migrada
- **When** se inspeccionan las columnas de `reminders`
- **Then** existe `notified_at` de tipo `TEXT`
- **And** admite `NULL`

#### Scenario: Un recordatorio recién creado no está notificado

- **Given** un recordatorio recién creado sin `notified_at`
- **When** se consulta su `notified_at`
- **Then** su valor es `NULL`

### Requirement: La migración SHALL sembrar los ajustes de WebPush

La migración SHALL sembrar los ajustes `push_vapid_public_key` y `push_vapid_private_key` vacíos —se
generan al primer uso— y `push_reminder_grace_hours` con el valor `'24'`. La siembra SHALL ser
idempotente y SHALL NO sobrescribir un valor ya personalizado, con el mismo idioma que
`20261010000007_rss_skill.sql`.

#### Scenario: Tras migrar existen las tres claves con sus valores

- **Given** una base de datos vacía
- **When** se ejecuta `run_migrations()`
- **Then** `settings` contiene `push_vapid_public_key` y `push_vapid_private_key` vacías
- **And** contiene `push_reminder_grace_hours` con `'24'`

#### Scenario: Una segunda ejecución no sobrescribe un valor personalizado

- **Given** un `push_reminder_grace_hours` cambiado por el usuario
- **When** se vuelve a ejecutar la migración
- **Then** el valor del usuario se conserva
- **And** no se produce ningún error

### Requirement: La migración SHALL crear la tabla `journal_entries` con una entrada por perfil y día

Una migración nueva SHALL crear, de forma idempotente, la tabla `journal_entries` con las columnas
`id` (TEXT, clave primaria), `profile_id` (TEXT NOT NULL, clave ajena a `profiles(id)`), `date` (TEXT
NOT NULL, fecha ISO `YYYY-MM-DD` **en la zona horaria del usuario**), `mood` (TEXT, admite `NULL`),
`content` (TEXT NOT NULL, el relato), `achievements` (TEXT, JSON array o `NULL`), `struggles` (TEXT,
JSON array o `NULL`), `tags` (TEXT, JSON array o `NULL`), `created_at` (TEXT NOT NULL con
`DEFAULT (datetime('now'))`) y `updated_at` (TEXT NOT NULL con `DEFAULT (datetime('now'))`). La tabla
SHALL declarar `UNIQUE(profile_id, date)`: **una sola entrada por perfil y día**. SHALL crear además
un índice que sirva el listado por perfil y rango de fechas (perfil y `date` descendente). La
migración SHALL NOT añadir ninguna columna a una tabla existente y SHALL ser reejecutable tal cual.

El conjunto cerrado de `mood` (`great`, `good`, `neutral`, `bad`, `terrible`) NO SHALL imponerse con
un `CHECK`: se garantiza en el modelo (ver `db/repos`), de modo que añadir un valor futuro no exija
reconstruir la tabla.

#### Scenario: La tabla se crea con sus columnas

- **Given** una base de datos vacía
- **When** se ejecuta `run_migrations()`
- **Then** existe la tabla `journal_entries`
- **And** tiene las columnas `id`, `profile_id`, `date`, `mood`, `content`, `achievements`, `struggles`, `tags`, `created_at` y `updated_at`
- **And** `mood`, `achievements`, `struggles` y `tags` admiten `NULL`

#### Scenario: La unicidad por perfil y día rechaza la segunda entrada

- **Given** una entrada de la bitácora con `profile_id = 'p1'` y `date = '2026-10-18'`
- **When** se intenta insertar otra fila con el mismo `profile_id` y la misma `date`
- **Then** la base de datos rechaza la segunda fila por la restricción `UNIQUE`
- **And** la primera sigue intacta

#### Scenario: El mismo día de perfiles distintos son entradas distintas

- **Given** una entrada con `profile_id = 'p1'` y `date = '2026-10-18'`
- **When** se inserta otra con `profile_id = 'p2'` y la misma `date`
- **Then** las dos filas coexisten

#### Scenario: El índice sirve el listado por perfil y rango de fechas

- **Given** la migración aplicada
- **When** se inspecciona el esquema de `journal_entries`
- **Then** existe un índice sobre `(profile_id, date DESC)`

#### Scenario: La migración es idempotente

- **Given** la tabla `journal_entries` ya creada
- **When** se ejecuta la migración de nuevo
- **Then** no hay error
- **And** la tabla y el índice siguen existiendo con el mismo esquema

### Requirement: La migración SHALL sembrar los ajustes de la bitácora sin pisar una personalización

Una migración nueva SHALL sembrar en `settings`, de forma idempotente, los ajustes de la bitácora:
`journal_prompt` con el prompt del relato, `journal_enabled` con `'true'` y `journal_hour` con `'4'`.
La siembra SHALL seguir el idioma de `20261010000004_timeline_prompts.sql`: SHALL rellenar una clave
ausente o vacía y SHALL NOT sobrescribir **nunca** un valor ya presente —una personalización del
usuario, aunque el valor sea el sembrado por una migración anterior—.

La marca de progreso `journal_last_judged_day` SHALL NOT sembrarse: su ausencia es el estado «nunca se
ha juzgado nada» y el worker arranca por el último día cerrado (ver `workers`). Sembrarla con el valor
equivocado haría que el diario empezara por donde no toca o que no empezara nunca.

#### Scenario: Tras migrar existen las tres claves con sus valores

- **Given** una base de datos vacía
- **When** se ejecuta `run_migrations()`
- **Then** `settings` contiene `journal_prompt` con un valor no vacío
- **And** contiene `journal_enabled` con `'true'`
- **And** contiene `journal_hour` con `'4'`

#### Scenario: Una reejecución no sobrescribe un valor personalizado

- **Given** un `journal_prompt` editado por el usuario, un `journal_enabled` a `'false'` y un
  `journal_hour` a `'23'`
- **When** se vuelve a ejecutar la migración
- **Then** los tres valores se conservan tal cual
- **And** no se produce ningún error

#### Scenario: Una clave ausente se rellena

- **Given** un `settings` sin la clave `journal_prompt`
- **When** se ejecuta la migración
- **Then** `journal_prompt` existe con el prompt del relato

#### Scenario: La marca de progreso no se siembra

- **Given** una base de datos recién migrada
- **When** se consulta `settings`
- **Then** NO existe la clave `journal_last_judged_day`
- **And** el worker arranca por el último día cerrado

#### Scenario: El prompt sembrado queda listo para editarse

- **Given** una base de datos recién migrada
- **When** se consulta `journal_prompt` por `GET /api/settings`
- **Then** la clave aparece con su valor
- **And** se puede cambiar por `PUT /api/settings` como cualquier otra

### Requirement: EventsRepo::list_by_date_range SHALL resolver el rango por instante y por solape

`EventsRepo::list_by_date_range` SHALL recibir los límites del rango como instantes
(`DateTime<FixedOffset>`) y SHALL devolver los eventos cuyo intervalo **se solape** con el rango
consultado, entendido como el intervalo semiabierto `[start, end)`
(`start_time < range_end AND end_time > range_start`). El filtro SHALL resolverse comparando
**instantes**, nunca el orden lexicográfico de las cadenas almacenadas, de modo que dos filas que
representan el mismo instante con offsets distintos se traten igual. El resultado SHALL ordenarse por
instante de inicio; una fila cuyos timestamps no se puedan parsear SHALL incluirse y SHALL quedar al
final del orden (falla abierto). `EventsRepo::list_by_category` SHALL ordenar con el mismo criterio.
Un helper público SHALL traducir un límite en texto a un instante: RFC 3339 (con offset o sufijo `Z`)
o fecha suelta `YYYY-MM-DD`, que SHALL cubrir el día UTC completo (`T00:00:00Z` como límite inferior y
el inicio del día siguiente como límite superior).

**Contracts:**

```rust
/// Traduce un límite de rango en texto a un instante.
/// - RFC 3339 con offset o sufijo `Z` → el instante tal cual.
/// - `YYYY-MM-DD` → día UTC completo: `T00:00:00Z`, o el inicio del día
///   siguiente cuando `upper = true`.
/// - Cualquier otro valor → `None` (límite no parseable).
pub fn parse_range_bound(raw: &str, upper: bool) -> Option<DateTime<FixedOffset>>;

impl EventsRepo {
    /// Intervalo semiabierto [start, end).
    /// Solape: start_time < end && end_time > start, comparando instantes.
    pub async fn list_by_date_range(
        pool: &SqlitePool,
        profile_id: &str,
        start: DateTime<FixedOffset>,
        end: DateTime<FixedOffset>,
    ) -> Result<Vec<Event>, sqlx::Error>;

    pub async fn list_by_category(
        pool: &SqlitePool,
        profile_id: &str,
        category: &str,
    ) -> Result<Vec<Event>, sqlx::Error>;
}
```

**Scenarios:**

#### Scenario: Un límite de fecha suelta incluye el último día
**Given** un evento con start_time="2026-10-31T08:00:00Z" y end_time="2026-10-31T09:00:00Z"
**When** `list_by_date_range` se consulta con `start = "2026-09-27"` y `end = "2026-10-31"`
**Then** el evento SHALL aparecer en el resultado

#### Scenario: Solape por delante
**Given** un evento con start_time="2026-09-26T23:00:00Z" y end_time="2026-09-27T01:00:00Z"
**When** `list_by_date_range` se consulta con `start = "2026-09-27T00:00:00Z"` y `end = "2026-09-28T00:00:00Z"`
**Then** el evento SHALL aparecer en el resultado

#### Scenario: Un evento que termina en el límite inferior queda fuera
**Given** un evento cuyo end_time es exactamente el límite inferior del rango consultado
**When** `list_by_date_range` se consulta con ese rango
**Then** el evento NO SHALL aparecer (el intervalo es semiabierto)

#### Scenario: Los formatos mezclados se comparan por instante
**Given** un evento A con start_time="2026-10-06T19:00:00+02:00" y end_time="2026-10-06T20:00:00+02:00" (17:00–18:00Z)
**And** un evento B con start_time="2026-10-06T18:30:00Z" y end_time="2026-10-06T19:30:00Z"
**When** `list_by_date_range` se consulta con `start = "2026-10-06T18:15:00Z"` y `end = "2026-10-06T18:45:00Z"`
**Then** B SHALL aparecer y A NO SHALL aparecer

#### Scenario: Orden por instante de inicio
**Given** un evento A con start_time="2026-10-06T19:00:00+02:00" (17:00Z)
**And** un evento B con start_time="2026-10-06T18:30:00Z"
**When** `list_by_date_range` se consulta con un rango que abarca los dos
**Then** A SHALL aparecer antes que B

#### Scenario: Una fila con timestamps ilegibles se incluye
**Given** una fila cuyo start_time no es parseable
**When** `list_by_date_range` se consulta con cualquier rango
**Then** la fila SHALL incluirse y SHALL quedar al final del orden

#### Scenario: Un límite no parseable no se convierte en instante
**When** `parse_range_bound("no-es-una-fecha", false)` se evalúa
**Then** SHALL devolver `None`

### Requirement: EventsRepo::find_free_slots SHALL comparar por instante

`EventsRepo::find_free_slots` SHALL calcular los huecos libres de un día comparando **instantes**
parseados, nunca el orden lexicográfico de las cadenas almacenadas, de modo que los eventos guardados
con offset local y los guardados con sufijo `Z` se ordenen y se recorten correctamente. El día SHALL
abarcar desde las `T00:00:00Z` del día indicado hasta el **inicio del día siguiente** (intervalo
semiabierto `[start, end)`, igual que el rango de consulta) y los huecos SHALL devolverse en RFC 3339
UTC canónico.

**Contracts:**

```rust
impl EventsRepo {
    /// `date` en `YYYY-MM-DD`. Los huecos se calculan por instante y se
    /// devuelven en RFC 3339 UTC canónico.
    pub async fn find_free_slots(
        pool: &SqlitePool,
        profile_id: &str,
        date: &str,
        duration_min: i64,
    ) -> Result<Vec<(String, String)>, sqlx::Error>;
}
```

**Scenarios:**

#### Scenario: Huecos con formatos mezclados
**Given** un evento con start_time="2026-10-06T19:00:00+02:00" y end_time="2026-10-06T20:00:00+02:00" (17:00–18:00Z)
**And** un evento con start_time="2026-10-06T18:30:00Z" y end_time="2026-10-06T19:30:00Z"
**When** se piden huecos de 30 minutos para `2026-10-06`
**Then** SHALL devolverse un hueco que termina en `2026-10-06T17:00:00Z`
**And** ningún hueco SHALL solaparse con los dos eventos

### Requirement: La migración SHALL retirar la tabla `tasks` y su índice FTS

La migración `20261028000001_remove_local_tasks.sql` SHALL ejecutar `DROP TABLE IF EXISTS tasks_fts`
**antes** de `DROP TABLE IF EXISTS tasks`, porque `tasks_fts` es una tabla FTS5 de contenido externo
que apunta a `tasks`. SHALL ser idempotente (`IF EXISTS`) y NO SHALL tocar ninguna otra tabla.

**Given** la base de datos con la migración aplicada
**When** se consulta `sqlite_master`
**Then** no existen `tasks` ni `tasks_fts`
**And** `events`, `notes`, `messages`, `reminders`, `memory`, `journal_entries` y `timeline_events`
siguen intactas

#### Scenario: Un despliegue repetido no falla

- **Given** una base de datos donde la migración ya se aplicó
- **When** se vuelve a ejecutar el arranque
- **Then** no devuelve error, porque ambos `DROP` llevan `IF EXISTS`
