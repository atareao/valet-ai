-- Migración de la integración con Mastodon (`mastodon-mentions`).
--
-- Aditiva e idempotente. Solo rellena lo que no tiene valor, así que una edición
-- del prompt hecha por el usuario se conserva byte a byte. No se siembra el
-- token: es sensible y su única puerta es `PUT /api/mastodon/token`.

-- 1) URL base: SIN valor por defecto. Enviar el token a una instancia
--    supuesta sería peligroso, así que el usuario debe escribir la suya.
INSERT INTO settings (key, value, updated_at)
VALUES ('mastodon_base_url', '', datetime('now'))
ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')
WHERE settings.value = '' OR settings.value IS NULL;

-- 2) Token: vacío a propósito (material sensible).
INSERT INTO settings (key, value, updated_at)
VALUES ('mastodon_token', '', datetime('now'))
ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')
WHERE settings.value = '' OR settings.value IS NULL;

-- 3) Fragmento de la skill `mastodon`.
INSERT INTO settings (key, value, updated_at)
VALUES ('SKILL_MASTODON_PROMPT', '# SKILL ACTIVA: MASTODON
- Leer las menciones de Mastodon se resuelve con la herramienta `mastodon` y su operación `list_mentions`, no inventes menciones ni las saques de `notes`.
- `list_mentions` admite `limit` (1 a 40, por defecto 20) y `max_id` opcional para paginar, devuelve el texto plano de cada mención y sus identificadores.
- Las menciones nuevas también llegan a la bandeja de avisos, no vuelvas a listarlas en bucle ni las marques como leídas sin que el usuario lo pida.
- Contestar se resuelve con la operación `reply`: pasa `status_id` y `text`, y opcionalmente `visibility` (`public`, `unlisted`, `private` o `direct`) y `spoiler_text`.
- `reply` exige aprobación explícita: pide confirmación antes de responder y nunca respondas por tu cuenta.
- Si el status original era `direct`, la respuesta se envía como `direct` aunque se pida otra visibilidad. No intentes eludirlo.
- No se publican estados nuevos, boosts, favourites ni multimedia: solo menciones y respuestas.
- Si Mastodon no está configurado, dilo y pide al usuario que pegue el token y la URL base en Ajustes → Integraciones → Mastodon.
- Responde siempre en español.', datetime('now'))
ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')
WHERE settings.value = '' OR settings.value IS NULL;

-- 4) Deduplicación de avisos y memoria de la visibilidad original. La
--    visibilidad se guarda para proteger las respuestas a estados `direct`.
CREATE TABLE IF NOT EXISTS mastodon_seen (
    notification_id TEXT PRIMARY KEY,
    status_id TEXT NOT NULL,
    visibility TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_mastodon_seen_status_id
    ON mastodon_seen(status_id);
