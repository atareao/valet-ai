-- Bitácora: una entrada por perfil y día.
--
-- Aditiva e idempotente: `CREATE TABLE IF NOT EXISTS` más un índice con
-- `IF NOT EXISTS`, así que reejecutar el fichero tal cual es un no-op y ninguna
-- tabla existente se reconstruye ni se ensancha. La tabla nace vacía; no hay
-- datos que migrar.
--
-- `UNIQUE(profile_id, date)` es lo que hace del día la unidad: la segunda
-- escritura de la misma fecha reemplaza la fila en vez de añadir otra. `date` se
-- guarda como ISO `YYYY-MM-DD` **en la zona horaria del usuario**; convertir de
-- UTC a local es cosa de quien escribe, no del repositorio.
--
-- `mood` no lleva `CHECK` a propósito: el conjunto cerrado (`great`, `good`,
-- `neutral`, `bad`, `terrible`) se garantiza en el modelo (`Mood`), de modo que
-- añadir un ánimo futuro no exija reconstruir la tabla. Lo mismo vale para los
-- tres campos con estructura: son JSON array o `NULL`.
CREATE TABLE IF NOT EXISTS journal_entries (
    id TEXT PRIMARY KEY,
    profile_id TEXT NOT NULL REFERENCES profiles(id),
    date TEXT NOT NULL,
    mood TEXT,
    content TEXT NOT NULL,
    achievements TEXT,
    struggles TEXT,
    tags TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(profile_id, date)
);

-- El listado de la bitácora es «por perfil, de más reciente a más antigua»: este
-- índice lo sirve sin ordenar la tabla entera.
CREATE INDEX IF NOT EXISTS idx_journal_entries_profile_date
    ON journal_entries(profile_id, date DESC);
