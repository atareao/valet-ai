## ADDED Requirements

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
