-- Migración de la integración con el blog (`wordpress-credentials-settings`).
--
-- Aditiva e idempotente, con el mismo idioma que las migraciones hermanas
-- (`20261027000001_opentodo.sql`, `20261107000001_oximeal.sql`): solo se rellena
-- un valor ausente, vacío o NULL, así que una personalización del usuario se
-- conserva.
--
-- Why each block:
--
--   1) `blog_base_url`: la URL base del sitio de WordPress, con su valor por
--      defecto. Es un ajuste normal (no sensible): viaja y se edita por
--      `GET`/`PUT /api/settings`.
--
--   2) `blog_service_user`: se siembra **vacío** a propósito; el usuario lo pega
--      en Ajustes → Integraciones → Blog.
--
--   3) `blog_app_password`: se siembra **vacío** a propósito y **nunca** con un
--      valor por defecto. Es material sensible (`SENSITIVE_KEYS`) y su única
--      puerta de escritura es `PUT /api/blog/password`, de modo que el secreto
--      jamás acaba en el repositorio ni en el historial de migraciones.

-- 1) URL base del blog. Solo se siembra cuando falta.
INSERT INTO settings (key, value, updated_at)
VALUES ('blog_base_url', 'https://atareao.es', datetime('now'))
ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')
WHERE settings.value = '' OR settings.value IS NULL;

-- 2) Usuario de servicio: vacío a propósito; lo rellena el usuario en la UI.
INSERT INTO settings (key, value, updated_at)
VALUES ('blog_service_user', '', datetime('now'))
ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')
WHERE settings.value = '' OR settings.value IS NULL;

-- 3) Application Password: vacía a propósito; solo se escribe por
--    `PUT /api/blog/password`.
INSERT INTO settings (key, value, updated_at)
VALUES ('blog_app_password', '', datetime('now'))
ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')
WHERE settings.value = '' OR settings.value IS NULL;
