-- Migración de la integración con GitHub (`github-integration`).
--
-- Aditiva e idempotente, con el mismo idioma que las migraciones hermanas
-- (`20261027000001_opentodo.sql`, `20261107000001_oximeal.sql`): solo se rellena
-- un valor ausente, vacío o NULL, así que una personalización del usuario se
-- conserva.
--
-- Why each block:
--
--   1) `github_default_repo`: el repositorio de trabajo por defecto, en forma
--      `owner/name`. Es un ajuste normal (no sensible): viaja y se edita por
--      `GET`/`PUT /api/settings`. El parámetro `repo` de la herramienta lo
--      sobrescribe.
--
--   2) `github_default_labels`: los labels por defecto, una lista separada por
--      comas (o vacía). Es un ajuste normal (no sensible). El parámetro `labels`
--      de la herramienta lo sustituye.
--
--   **NO** se siembra `github_token`: es material sensible (`SENSITIVE_KEYS`) y su
--   única puerta de escritura es `PUT /api/github/token`, de modo que el token
--   nunca acaba en el repositorio ni en el historial de migraciones.
--
--   3) `SKILL_GITHUB_PROMPT`: fragmento de la skill `github`, con
--      `# SKILL ACTIVA: GITHUB` como primera línea (el `prompt_heading` que usa el
--      ensamblador de fragmentos para no duplicarlo). Solo se rellena cuando falta,
--      está vacío o es NULL.

-- 1) Repositorio por defecto. Solo se siembra cuando falta.
INSERT INTO settings (key, value, updated_at)
VALUES ('github_default_repo', 'atareao/valet-ai', datetime('now'))
ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')
WHERE settings.value = '' OR settings.value IS NULL;

-- 2) Labels por defecto: vacío a propósito; lo rellena el usuario en la UI.
INSERT INTO settings (key, value, updated_at)
VALUES ('github_default_labels', '', datetime('now'))
ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')
WHERE settings.value = '' OR settings.value IS NULL;

-- 3) Fragmento de la skill `github` (re-ejecutable; conserva una edición del usuario).
INSERT INTO settings (key, value, updated_at)
VALUES ('SKILL_GITHUB_PROMPT', '# SKILL ACTIVA: GITHUB
- Crear un issue se resuelve con la herramienta `github` y su operación `create_issue`; no mantengas una lista paralela en `notes`.
- El repositorio destino sale del parámetro `repo` (forma `owner/name`) o, si falta, del ajuste `github_default_repo`; sin ninguno de los dos, la llamada falla y no se toca la red.
- Los labels salen del parámetro `labels` o, si falta, del ajuste `github_default_labels`; sin ninguno, el issue se crea sin labels.
- `title` es obligatorio y no puede estar vacío; `body` es opcional y es el texto del issue.
- Crear un issue avisa pero no exige aprobación explícita: no es destructivo (un issue se cierra o se borra en GitHub).
- No inventes el repositorio ni los labels: si no los conoces, pregunta al usuario o deja que se apliquen los ajustes.
- Responde siempre en español.', datetime('now'))
ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')
WHERE settings.value = '' OR settings.value IS NULL;
