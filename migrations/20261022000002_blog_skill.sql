-- Siembra de la skill `blog`: la bandera de habilitación y el fragmento de guía
-- para la herramienta `blog_comment_reply`.
--
-- Mismo idioma que `20261010000007_rss_skill.sql`: idempotente, y solo rellena un
-- valor ausente o vacío, así que una personalización del usuario se conserva.
--
-- Why each block:
--
--   1) `ROUTER_SKILL_BLOG_ENABLED='true'`. El lector ya trata una clave ausente
--      como habilitada, pero sembrarla hace explícita la intención y da a la UI
--      una fila que editar. Una skill deshabilitada por el usuario nunca se pisa.
--
--   2) `SKILL_BLOG_PROMPT`. La clave coincide con el `prompt_key` de la skill y su
--      primera línea es el `prompt_heading` (`# SKILL ACTIVA: BLOG`), que es como
--      el catálogo detecta el fragmento y evita duplicarlo. Lleva la guía de
--      operación de `blog_comment_reply`.

-- 1) La skill blog nace habilitada; un valor del usuario no se pisa.
INSERT INTO settings (key, value, updated_at)
VALUES ('ROUTER_SKILL_BLOG_ENABLED', 'true', datetime('now'))
ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')
WHERE settings.value = '' OR settings.value IS NULL;

-- 2) Guía de operación de la skill blog. Solo se siembra cuando falta.
INSERT INTO settings (key, value, updated_at)
VALUES ('SKILL_BLOG_PROMPT', '# SKILL ACTIVA: BLOG
- «Responde a este comentario» se resuelve con `blog_comment_reply`, que publica UNA respuesta en el sitio.
- Publicar es público e irreversible: pídele confirmación al usuario y no publiques sin su visto bueno.
- Los identificadores salen del `metadata` de la notificación del comentario, que se lee con `notifications`. Pasa `notification_id` y la herramienta los resuelve, o pásale los `comment_id` y `post_id` explícitos; no los inventes nunca.
- `content` es obligatorio (no puede quedar vacío) y es el texto de la respuesta: redáctalo tú con lo que el usuario aprueba.
- No respondas dos veces el mismo comentario: si ya tiene registro, la herramienta lo dirá y no hay que insistir.
- Responde siempre en español.', datetime('now'))
ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')
WHERE settings.value = '' OR settings.value IS NULL;
