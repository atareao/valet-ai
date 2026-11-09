# Integración con GitHub

## Qué es

Integración de Valet con GitHub: el agente crea issues desde la conversación.
La herramienta `github` tiene una **única operación** (`create_issue`) y se
autentica con un **Personal Access Token (PAT) clásico** contra la API REST de
GitHub (`https://api.github.com`, URL **fija**, no configurable).

Va asociada al change `github-integration` y a la issue
[#333](https://github.com/atareao/valet-ai/issues/333).

## Ficheros implicados

| Fichero | Rol |
|---|---|
| `src/services/github.rs` | Cliente HTTP `Github`: `resolve_config` (lee de `settings`), `check_connection` (`GET /user`), `create_issue` (`POST /repos/{repo}/issues`) y `GithubError`. |
| `src/tools/github.rs` | Herramienta `GithubTool` (multioperación, molde `oximeal`): valida argumentos sin red, despacha `create_issue`, permiso `Notify`. |
| `src/handlers/github.rs` | Handlers HTTP: `status`, `check` y alta del token. |
| `src/routes/github.rs` | Rutas `/api/github/*`. |
| `src/db/repos/settings.rs` | `SENSITIVE_KEYS` incluye `github_token`. |
| `src/orchestrator/skills.rs` | Skill `github` (`tools: &["github"]`, `prompt_key: "SKILL_GITHUB_PROMPT"`, `prompt_heading: "# SKILL ACTIVA: GITHUB"`, umbral `0.10`). |
| `migrations/20261111000001_github.sql` | Siembra `github_default_repo`, `github_default_labels` y `SKILL_GITHUB_PROMPT` (aditiva e idempotente). |
| `src/lib.rs` | Registra la herramienta (`registry.register`) y monta las rutas. |
| `frontend/src/components/GithubSection.tsx` | Sub-pestaña «GitHub» dentro de Ajustes → Integraciones. |
| `frontend/src/hooks/useGithub.ts` | Hook con el estado de la integración y las acciones. |
| `frontend/src/api/github.ts` | Llamadas `getGithubStatus`/`putGithubToken`/`checkGithub`, fuera del cliente compartido `api/client.ts` para no engordar el grafo inicial del bundle. |
| `frontend/src/components/IntegrationsTab.tsx` | Añade la sub-pestaña «GitHub» (11 sub-pestañas en total). |
| `frontend/src/components/SettingsDialog.tsx` | Campos `github_default_repo` y `github_default_labels`. |
| `frontend/src/types/index.ts` | Tipos `GithubStatus`/`GithubCheckResult`. |

## Cómo funciona

Flujo de una llamada del agente:

1. **Selección de skill.** El router marca `github` como activa (umbral `0.10`)
   y expone la herramienta `github` al modelo.
2. **Validación de argumentos (sin red).** `src/tools/github.rs` valida antes de
   tocar la red: `title` obligatorio y no vacío; `repo` con forma `owner/name`
   (una sola barra); `labels`, si viene, array no vacío de cadenas no vacías. Un
   argumento inválido se rechaza con `InvalidArguments` y no envía petición.
3. **Permiso.** `create_issue` es `Notify` (escritura **aditiva**: un issue se
   cierra o se borra en GitHub). Una operación desconocida o ausente es
   `ExplicitApproval` (falla cerrado y `execute` la rechaza).
4. **Resolución de configuración.** `Github::resolve_config` lee
   `github_token`, `github_default_repo` (con `atareao/valet-ai` por defecto) y
   `github_default_labels` (lista separada por comas) **en cada llamada**, sin
   respaldo de variables de entorno. Sin token devuelve `NotConfigured` **sin**
   tocar la red.
5. **Resolución de repo y labels.** El repositorio sale del parámetro `repo` o,
   si falta, de `github_default_repo`; los labels del parámetro `labels` o, si
   falta, de `github_default_labels` (el parámetro **sustituye** al ajuste).
6. **Llamada a la API.** `POST {base}/repos/{repo}/issues` con
   `Authorization: Bearer <token>` y el cuerpo `{title, body?, labels?}`; `body`
   y `labels` se envían **solo** cuando están presentes. **Solo un `201` es
   éxito**: devuelve el número y la URL (`html_url`) del issue.
7. **Traducción de errores.** `401`/`403` → `Unauthorized`, `404` → `NotFound`,
   `422` → `InvalidInput` (con el `message` del servidor), `429` →
   `RateLimited`, `5xx` o fallo de red → `Upstream`. **Ningún** mensaje incluye
   el token y **no hay reintentos**: cada invocación envía una sola petición
   (reintentar una escritura sin idempotencia duplicaría issues).

### Endpoints de integración de Valet

| Método | Ruta | Qué hace |
|---|---|---|
| GET | `/api/github/status` | Estado sin el token: `{configured, default_repo}` |
| GET | `/api/github/check` | Sonda real (`GET /user`). Siempre `200` con `{ok, detail}` |
| PUT | `/api/github/token` | Guarda el PAT (`{"token": "…"}`): `400` si va vacío, `204` si se guarda |

`GET /api/github/check` distingue tres averías en `detail`:
`configuración incompleta` (falta el token), `el token fue rechazado`
(`401`/`403`) y `GitHub no responde` (cualquier otro desenlace).

## Configuración

| Ajuste | Por defecto | Sensible |
|---|---|---|
| `github_default_repo` | `atareao/valet-ai` | no |
| `github_default_labels` | `''` | no |
| `github_token` | `''` | **sí** |

- `github_token` figura en `SENSITIVE_KEYS` (`src/db/repos/settings.rs`):
  `GET /api/settings` lo **omite** y `PUT /api/settings` lo **ignora**. Su única
  puerta de escritura es `PUT /api/github/token`. El campo del frontend arranca
  **siempre vacío** y nunca relee el valor guardado.
- La **URL base del API no es un ajuste**: es la constante fija
  `https://api.github.com` (no hay instancia self-hosted que configurar).
- La migración es aditiva e idempotente: solo rellena un valor ausente, vacío o
  `NULL`, así que una personalización del usuario se conserva. **No** siembra
  `github_token`.

## Cómo habilitar / desplegar

1. **Crear un Personal Access Token (clásico)** en GitHub con permiso de
   escritura de issues sobre el repositorio destino (`repo`).
2. **Ajustes → Integraciones → GitHub**: pegar el token en el campo dedicado
   (arranca vacío) → «Guardar». Opcionalmente, fijar el repositorio por defecto
   (`github_default_repo`) y los labels por defecto (`github_default_labels`,
   lista separada por comas), que se guardan por `PUT /api/settings`.
3. **Comprobar la conexión** con «Probar conexión», que llama a
   `GET /api/github/check` y muestra el veredicto.
4. Publicar la imagen y recrear el stack en `co1` (Dockge).

## Decisiones

- **Una sola operación.** `github` es multioperación (molde `oximeal`) pero hoy
  solo admite `create_issue`; la validación ocurre antes de resolver la config y
  de tocar la red.
- **PAT clásico con `Authorization: Bearer`.** El cliente usa `bearer_auth`; la
  variante `NotConfigured` (falta el token) es **distinta** de `Unauthorized`
  (el token no vale).
- **URL fija.** No existe ajuste de URL base: `https://api.github.com` es una
  constante.
- **Sin reintentos.** Un `429` o un `5xx` se traduce a error sin volver a
  enviar la escritura (evita duplicar issues).
- **El token es sensible.** Nunca sale por `GET /api/settings`, no se guarda por
  `PUT /api/settings`, y ningún `Debug`, log o mensaje de error lo incluye
  (`GithubConfig` redacta el token como `***`).
- **`create_issue` avisa pero no exige aprobación** (`Notify`): es aditiva, no
  destructiva. Una operación desconocida falla cerrado.
- **Skill `github` dedicada.** Crear issues es un dominio propio, así que la
  herramienta vive en su propia skill (umbral `0.10`); el catálogo pasa a
  **14 skills / 35 herramientas**.

## Follow-ups

- **E2E pendiente contra GitHub real.** No se ha verificado el ciclo completo
  con un PAT clásico real (`check` → `{ok:true}` y `create_issue` contra
  `atareao/valet-ai` con comprobación del issue creado).
- **Rotación del token.** La actualización del PAT se hace por su endpoint
  dedicado; no hay invalidación de caché que gestionar porque el token no se
  cachea.
