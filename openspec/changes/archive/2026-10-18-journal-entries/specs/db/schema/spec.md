## ADDED Requirements

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
