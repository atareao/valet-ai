-- Capa C (memoria persistente): la tabla de la única fila lógica del estado
-- (`id = 'global_state'`) y las dos claves de `settings` que la gobiernan.
--
-- Same idiom as `20260929000001_prompts.sql` and
-- `20261001000002_memory_settings.sql`: the table is created with
-- `IF NOT EXISTS` and the upsert only overwrites empty/NULL values so user
-- customisations are preserved, and it is idempotent.

CREATE TABLE IF NOT EXISTS persistent_memory (
    id         TEXT PRIMARY KEY,
    payload    TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

INSERT INTO settings (key, value, updated_at)
VALUES ('consolidator_prompt', 'System: Eres el consolidador de memoria persistente de Valet.
Recibes el estado persistente actual y un bloque de mensajes nuevos, y devuelves el estado actualizado.

Devuelve EXCLUSIVAMENTE un objeto JSON con esta forma:
{
  "schema_version": 1,
  "user_profile": { },
  "system_rules": [ ]
}

Conserva la información ya presente y añade o corrige solo lo que los mensajes justifiquen.
No añadas claves de primer nivel distintas de las indicadas.

Estado persistente actual:
{{ ESTADO_ACTUAL }}

Bloque de mensajes:
{{ BLOQUE_DE_MENSAJES }}', datetime('now'))
ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')
WHERE settings.value = '' OR settings.value IS NULL;

INSERT INTO settings (key, value, updated_at)
VALUES ('PERSISTENT_MEMORY_BUDGET_TOKENS', '500', datetime('now'))
ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')
WHERE settings.value = '' OR settings.value IS NULL;
