# Change: Integración ZenNotes — leer y escribir notas del vault

## Why

Valet no tiene hoy ninguna vía para tocar el **vault real** de notas del usuario. Lo que existe es una
store **interna** (la tool `notes`, tabla SQLite con categorías `idea`/`journal`/`fact`) que solo el
propio Valet ve. El usuario gestiona su conocimiento en **ZenNotes**, un servidor autoalojado
(`https://zennotes.territoriolinux.es`) con una API HTTP propia y **6.273 notas** (verificado: 3,3 MB
en `GET /api/notes`). El deseo es que el agente lea y escriba ese vault —artículos, podcasts, notas—
y que, además, tenga una **carpeta dedicada `Valet`** para las notas que cree por su cuenta.

La API de ZenNotes está autenticada con `Authorization: Bearer <token>` y expone exactamente las
operaciones necesarias (contratos verificados por lectura directa):

- `GET /api/vault` → `{"root":"/workspace","name":"workspace"}`
- `GET /api/notes` → `[{path,title,folder,siblingOrder,createdAt,updatedAt,size,tags[],wikilinks[],assetEmbeds[],hasAttachments,excerpt}]`
- `GET /api/notes/read?path=<rel>` → igual + `body` (markdown completo, con frontmatter)
- `GET /api/search/text?q=<q>&backend=auto|text` → `[{path,title,folder,lineNumber,offset,lineText}]`
- `GET /api/folders` → `[{folder,subpath,siblingOrder}]`
- Escrituras: `POST /api/notes/create {folder,title,subpath}` (**sin body**), `/api/notes/write {path,body}`,
  `/api/notes/rename {path,title}`, `/api/notes/move {path,targetFolder,targetSubpath}`,
  `/api/notes/archive|unarchive|trash|restore|delete {path}`, `/api/notes/duplicate {path}`,
  `/api/folders/create {folder,subpath}`.

Dos hechos condicionan el diseño: (1) la ruta `create` **no acepta contenido**, así que crear una nota
con cuerpo son **dos viajes** (create + write); y (2) `append`/`replace`/`prepend` **no son endpoints**,
sino leer→modificar el `body`→escribir, como hace el cliente oficial.

## What Changes

- **Nueva skill `zennotes`** en el catálogo cerrado (`Skill::Zennotes`), la **duodécima**, con su
  pregunta, sus dos criterios, su umbral `0.10`, su clave de fragmento `SKILL_ZENNOTES_PROMPT` y su
  encabezado `# SKILL ACTIVA: ZENNOTES`. Habilitada por defecto. Cubre la herramienta `zennotes`, y
  solo ella. Sus criterios distinguen el **vault de ZenNotes** del apunte interno de `recuerdos`.
- **Nueva herramienta `zennotes`** (una sola, con parámetro `operation`, molde `calendar`), registrada
  en `build_tool_registry` y cubierta por la skill. Operaciones: `list`, `search`, `read`, `create`,
  `append`, `replace`, `write`, `move`, `archive`, `unarchive`, `rename`, `trash`, `restore`, `delete`.
  Lecturas (`list`/`search`/`read`) → `NoConfirm`; escrituras aditivas (`create`/`append`/`replace`/
  `move`/`archive`/`unarchive`/`rename`/`restore`) → `Notify`; irreversibles (`write`/`trash`/`delete`)
  → `ExplicitApproval`. `empty-trash` **no se expone**.
- **Recorte obligatorio**: `GET /api/notes` devuelve 6.273 notas / 3,3 MB, así que `list` SHALL
  devolver un resumen acotado (por defecto 20, máximo 200) con campos mínimos, y `read` SHALL recortar
  el cuerpo a un tope (20.000 caracteres), como `rss_get_entry`.
- **Carpeta dedicada `Valet`**: las notas creadas sin destino explícito SHALL ir a la **subcarpeta**
  configurada (por defecto `Valet`) dentro de `inbox`, es decir a `inbox/Valet/`; `folder` es un enum
  restringido a `inbox`/`quick`/`archive` en el servidor.
- **Nueva capa de servicio `Zennotes`** (`src/services/zennotes.rs`) que resuelve la URL base, el token
  y la carpeta —de `settings` primero (`zennotes_base_url`, `zennotes_token`, `zennotes_folder`), del
  entorno después (`ZENNOTES_BASE_URL`, `ZENNOTES_TOKEN`) y, solo para la URL, de un valor por defecto
  `https://zennotes.territoriolinux.es`— y traduce los errores a mensajes accionables **sin exponer
  nunca el token**.
- **Token como ajuste sensible**: `zennotes_token` SHALL entrar en `SENSITIVE_KEYS` (oculto en
  `GET /api/settings`). Como `PUT /api/settings` **ignora** las claves sensibles, se añaden **endpoints
  dedicados** `GET /api/zennotes/status` y `PUT /api/zennotes/token` (molde Strava).
- **Migración aditiva e idempotente** que siembra `zennotes_base_url`, `zennotes_folder`,
  `zennotes_token` (vacío), `SKILL_ZENNOTES_PROMPT` y `ROUTER_SKILL_ZENNOTES_ENABLED`, sin pisar
  valores existentes.
- **Pestaña «API Keys»** con un campo de token de ZenNotes que **nunca muestra** el valor guardado
  (estado «Configurado / No configurado»).

## Impact

### Specs modificadas
- `orchestrator/skill-router`: el catálogo cerrado pasa de once a **doce** skills.
- `tools/registry`: el registro de producción pasa de treinta y dos a **treinta y tres** herramientas,
  y el requisito de operaciones destructivas nombra `zennotes`.
- `frontend`: la pestaña «API Keys» gana el campo del token de ZenNotes.

### Specs nuevas
- `tools/zennotes`: la herramienta `zennotes` y su contrato con la API del servidor.
- `zennotes-api`: las claves de ajuste, su sensibilidad, la migración y los endpoints de token.

### Código
- `src/services/zennotes.rs` (nuevo), `src/services/mod.rs`, `src/tools/zennotes.rs` (nuevo),
  `src/tools/mod.rs`, `src/lib.rs` (registro), `src/orchestrator/skills.rs` (variante + catálogo +
  tests), `src/db/repos/settings.rs` (`SENSITIVE_KEYS` + siembra), `src/routes/settings.rs` o ruta
  nueva para los endpoints de token, y `frontend/src/components/SettingsDialog.tsx` (+ tipos y hook).
  Una migración nueva en `migrations/` y su target de test.

### Datos
- Migración nueva que siembra cinco claves. No se reconstruye ninguna tabla. No se migran datos del
  vault de ZenNotes.

## Non-goals

- **No hay vista de Notas en la app**: solo herramientas del agente (decisión del usuario). El único
  cambio de interfaz es el campo del token en Ajustes.
- **No se implanta MCP** (decisión §V.9 de `AGENTS.md`): esto es integración **nativa** por HTTP.
- **No se tocan los assets/imágenes del vault** (solo notas): `list_assets` queda fuera.
- **No se escriben las *tasks* de ZenNotes** (`/api/tasks`): no es objetivo.
- **No se expone `empty-trash`**: vaciar la papelera entera es demasiado destructivo.

## Riesgos y dependencia

- **R1 (alto) Fiabilidad del agente al llamar herramientas.** Diagnóstico reciente en producción: el
  agente responde/promete y **no** llama a la herramienta (llega a alucinar resultados). Esta feature
  es inservible si eso no se arregla. Dependencia con el arreglo del `system_prompt`.
- **R2 (alto) Volumen.** Recorte y límites obligatorios (ver arriba).
- **R3 (medio) Solapamiento de router** `zennotes` ↔ `recuerdos`. Mitigar con criterios explícitos y
  medir con `valet-route-eval`.
