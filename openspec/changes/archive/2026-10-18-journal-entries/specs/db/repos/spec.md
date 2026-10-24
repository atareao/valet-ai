## ADDED Requirements

### Requirement: El modelo `JournalEntry` y `Mood` SHALL definir el contrato de la bitácora

El modelo SHALL exponer `JournalEntry { id, profile_id, date, mood: Option<Mood>, content,
achievements: Vec<String>, struggles: Vec<String>, tags: Vec<String>, created_at, updated_at }` y
`enum Mood { Great, Good, Neutral, Bad, Terrible }`. `Mood` SHALL serializarse y deserializarse en
`snake_case` (`great`, `good`, `neutral`, `bad`, `terrible`) y SHALL ser **estricto**: un valor fuera
del conjunto NO SHALL ser válido, ni se inventa un ánimo ni se traga el dato en silencio. El ánimo
SHALL ser opcional: una entrada sin `mood` SHALL ser válida. Los campos `achievements`, `struggles` y
`tags` SHALL exponerse como listas de cadenas; un valor `NULL` en la base de datos SHALL leerse como
lista vacía `[]` y una lista vacía SHALL persistirse sin ambigüedad (`[]` o `NULL`).

**Contracts:**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mood { Great, Good, Neutral, Bad, Terrible }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JournalEntry {
    pub id: String,
    pub profile_id: String,
    pub date: String,               // ISO YYYY-MM-DD, en la zona horaria del usuario
    pub mood: Option<Mood>,
    pub content: String,
    pub achievements: Vec<String>,  // JSON array o NULL en la base
    pub struggles: Vec<String>,     // JSON array o NULL en la base
    pub tags: Vec<String>,          // JSON array o NULL en la base
    pub created_at: String,
    pub updated_at: String,
}
```

#### Scenario: El ánimo se serializa en snake_case

- **Given** un `Mood::Great`
- **When** se serializa a JSON
- **Then** el valor es `"great"`
- **And** los cinco ánimos se serializan como `great`, `good`, `neutral`, `bad` y `terrible`

#### Scenario: Un ánimo fuera del conjunto no es válido

- **Given** el valor `"genial"` para el campo `mood`
- **When** se deserializa una `JournalEntry`
- **Then** la deserialización falla
- **And** no se sustituye por ningún ánimo por defecto

#### Scenario: Una entrada sin ánimo es válida

- **Given** una entrada con `mood` ausente (`NULL`)
- **When** se lee la entrada
- **Then** su `mood` es `None`
- **And** la entrada se conserva entera

#### Scenario: Los campos JSON nulos se leen como listas vacías

- **Given** una entrada con `achievements`, `struggles` y `tags` a `NULL`
- **When** se lee la entrada
- **Then** sus tres campos son listas vacías `[]`

### Requirement: `JournalRepo` SHALL exponer upsert, get, list y delete por perfil y fecha

`JournalRepo` SHALL operar sobre `journal_entries` y exponer `upsert(pool, entry) -> JournalEntry`,
`get(pool, profile_id, date) -> Option<JournalEntry>`, `list(pool, profile_id, from, to, limit) ->
Vec<JournalEntry>` y `delete(pool, profile_id, date) -> bool`. El `upsert` SHALL crear la entrada si
no existe y **reemplazar** la del mismo día si existe, sin duplicar fila y conservando una sola;
`updated_at` SHALL cambiar en el reemplazo. `get` y `delete` SHALL direccionarse por
`(profile_id, date)` —la fecha es la clave pública; el `id` es interno— y `delete` SHALL devolver
`true` si borró una fila y `false` si no había ninguna. `list` SHALL devolver las entradas del rango
**inclusivo** `[from, to]` ordenadas de más reciente a más antigua y acotadas por `limit`.

**Contracts:**

```rust
impl JournalRepo {
    pub async fn upsert(pool: &SqlitePool, entry: &JournalEntry) -> Result<JournalEntry, sqlx::Error>;
    pub async fn get(pool: &SqlitePool, profile_id: &str, date: &str) -> Result<Option<JournalEntry>, sqlx::Error>;
    pub async fn list(pool: &SqlitePool, profile_id: &str, from: &str, to: &str, limit: i64) -> Result<Vec<JournalEntry>, sqlx::Error>;
    pub async fn delete(pool: &SqlitePool, profile_id: &str, date: &str) -> Result<bool, sqlx::Error>;
}
```

#### Scenario: `upsert` crea la entrada del día

- **Given** un perfil sin entrada para `'2026-10-18'`
- **When** se invoca `upsert` con esa fecha y un relato
- **Then** queda una fila nueva en `journal_entries` con ese relato

#### Scenario: `upsert` reemplaza la entrada del mismo día sin duplicar

- **Given** una entrada existente para `'2026-10-18'` con un relato
- **When** se invoca `upsert` con la misma fecha y un relato distinto
- **Then** sigue habiendo **una sola** fila para ese perfil y día
- **And** su `content` es el nuevo

#### Scenario: El reemplazo actualiza `updated_at`

- **Given** una entrada existente con su `created_at` y `updated_at`
- **When** se invoca `upsert` con la misma fecha
- **Then** `created_at` se conserva
- **And** `updated_at` es posterior o distinto del anterior

#### Scenario: `get` devuelve la entrada del día o nada

- **Given** una entrada guardada para `'2026-10-18'`
- **When** se invoca `get` con esa fecha
- **Then** devuelve la entrada
- **When** se invoca `get` con `'2026-10-19'`
- **Then** devuelve `None`

#### Scenario: `list` devuelve el rango de más reciente a más antiguo

- **Given** entradas en `'2026-10-15'`, `'2026-10-16'` y `'2026-10-20'`
- **When** se invoca `list` con `from = '2026-10-15'` y `to = '2026-10-16'`
- **Then** devuelve las dos entradas del rango
- **And** `'2026-10-16'` aparece antes que `'2026-10-15'`
- **And** la del `'2026-10-20'` no aparece

#### Scenario: `list` respeta el tope quedándose con lo más reciente

- **Given** más entradas que el tope pedido dentro del rango
- **When** se invoca `list` con ese `limit`
- **Then** devuelve a lo sumo `limit` entradas
- **And** son las más recientes del rango

#### Scenario: `delete` borra la del día y avisa de lo que no existía

- **Given** una entrada existente para `'2026-10-18'`
- **When** se invoca `delete` con esa fecha
- **Then** devuelve `true` y la fila desaparece
- **When** se invoca `delete` con `'2026-10-19'`
- **Then** devuelve `false`
