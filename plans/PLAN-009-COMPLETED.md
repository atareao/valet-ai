# PLAN-009 — Integración ZenNotes: leer y escribir en el vault

> Estado: **pendiente de aprobación** (fase de planificación). No se escribe código hasta aprobar el change proposal OpenSpec.
> Convención de repo: al completarse se renombrará a `plans/PLAN-009-COMPLETED.md`. El `PLAN.md` de la raíz es el checkpoint que lee `@qa-plan-verifier`.

## 0. Objetivo
Que el agente **Valet** pueda **leer y escribir notas del vault de ZenNotes** del usuario, usando la API HTTP del servidor propio `https://zennotes.territoriolinux.es`, con una **carpeta dedicada `Valet`** para las notas que Valet cree por su cuenta.

## 1. Hechos verificados (evidencia, no supuesto)
- ZenNotes del usuario corre en **workspace remoto**: `~/.config/ZenNotes/zennotes.config.json` → `workspaceMode:"remote"`, `remoteWorkspace.baseUrl = https://zennotes.territoriolinux.es`.
- `GET /api/vault` → `{"root":"/workspace","name":"workspace"}`. `GET /api/capabilities` → `version 2.76.0`, `authRequired:true`.
- `GET /api/vault/settings` → `primaryNotesLocation:"root"`, `systemFolderPaths:{"inbox":"notes"}`.
- Auth: cabecera `Authorization: Bearer <token>`. Ya existe un token de MCP: `ZENNOTES_REMOTE_TOKEN` (config de OpenCode).
- Contratos reales capturados (solo lectura):
  - `GET /api/notes` → array de `{path,title,folder,siblingOrder,createdAt,updatedAt,size,tags[],wikilinks[],assetEmbeds[],hasAttachments,excerpt}`. **6.273 notas, 3,3 MB** ← la tool NO puede volcar esto al prompt.
  - `GET /api/notes/read?path=<rel>` → igual + `body` (markdown completo, con frontmatter).
  - `GET /api/search/text?q=<q>&backend=auto|text` → array de `{path,title,folder,lineNumber,offset,lineText}`.
  - `GET /api/folders` → array de `{folder,subpath,siblingOrder}`.
  - `GET /api/tasks?includeExcluded=1` → tareas.
  - `/api/tags` → **404** (no existe; las etiquetas salen de la metadata de las notas).
- Escrituras (rutas y cuerpos, tomadas del cliente oficial del MCP de ZenNotes):
  - `POST /api/notes/create` `{folder,title,subpath}` → devuelve meta con `path`. **La ruta create NO acepta body**: crear con contenido = create + write (dos viajes).
  - `POST /api/notes/write` `{path,body}` (sobrescribe el cuerpo completo).
  - `POST /api/notes/rename` `{path,title}`; `POST /api/notes/move` `{path,targetFolder,targetSubpath}`.
  - `POST /api/notes/archive|unarchive|trash|restore|delete` `{path}`; `POST /api/notes/empty-trash` `{}`; `POST /api/notes/duplicate` `{path}`.
  - `POST /api/folders/create` `{folder,subpath}`; `POST /api/folders/rename` `{folder,oldSubpath,newSubpath}`; `POST /api/folders/delete` `{folder,subpath}`.
  - **append/prepend/replace/insertAtLine NO son endpoints**: el cliente oficial los implementa con leer→modificar `body`→escribir.
- Arquitectura de Valet (verificada):
  - Herramientas en `src/tools/*.rs` (trait `Tool`, `src/tools/trait.rs`); registro en `build_tool_registry` (`src/lib.rs`).
  - Skills en `src/orchestrator/skills.rs` (**catálogo cerrado de 11**); los fragmentos de prompt se siembran por migración SQL y se leen por `prompt_key`.
  - Patrón de servicio externo en `src/services/*.rs` (molde: `miniflux.rs`/`apimail.rs`): resuelve la config desde `settings` **en cada llamada** y **nunca** filtra el token.
  - Tool de dominio con `operation` (molde: `calendar.rs`) y `permission(args)` que depende de la operación.
  - `PUT /api/settings` **ignora** las claves de `SENSITIVE_KEYS` (`src/routes/settings.rs`, `src/db/repos/settings.rs`). Las claves de API actuales (`brave_search_api_key`, `google_places_api_key`, `openweather_api_key`, `apimail_api_key`, `miniflux_api_key`) NO son sensibles: se ven en `GET` y se editan en Ajustes → API Keys.

## 2. Decisiones del usuario
- **D1 Alcance:** vault completo + carpeta Valet (leer y escribir en cualquier nota, y carpeta propia para sus notas).
- **D2 Carpeta dedicada:** `Valet`.
- **D3 Operaciones:** todas (crear, leer, buscar, añadir/editar parcial, mover/archivar, papelera/borrar).
- **D4 UI:** solo herramientas del agente (sin vista de notas). *Sí* hay cambio de frontend para el **token** (Ajustes → API Keys).
- **D5 Token:** nuevo ajuste **oculto en GET** (sensible), editable desde Ajustes → API Keys.

## 3. Diseño técnico
### 3.1 Servicio `src/services/zennotes.rs`
- Molde de `miniflux.rs`: `ZennotesConfig { base_url, token, folder }`; resolución en cada ejecución (settings → entorno → default solo de URL); `ZennotesError` (`MissingCredentials`/`Unauthorized`/`NotFound`/`Unavailable`/`Http`) con mensajes en español y `Debug` que **redacta el token** (nunca aparece en errores/logs).
- Cliente `Zennotes` con `reqwest` (timeout ~30 s, `User-Agent: valet/1.0`), cabecera `Authorization: Bearer <token>`.
- Operaciones del cliente: `vault`, `list_notes` (con **resumen**), `read_note`, `search_text`, `search_by_title` (filtro local sobre `list_notes`), `search_by_tag` (filtro local), `create_note` (create+write), `append` (read+write), `replace` (read+write), `write`, `move`, `archive`/`unarchive`, `rename`, `trash`/`restore`, `delete`.

### 3.2 Tool `src/tools/zennotes.rs`
- **Una sola tool `zennotes`** con parámetro `operation` (molde `calendar`).
- Operaciones: `list`, `search`, `read`, `create`, `append`, `replace`, `write`, `move`, `archive`, `unarchive`, `rename`, `trash`, `restore`, `delete`.
- **Permisos:** `list`/`search`/`read` → `NoConfirm`; `create`/`append`/`replace`/`move`/`archive`/`unarchive`/`rename` → `Notify`; `write`/`trash`/`restore`/`delete` → `ExplicitApproval`. **`empty-trash` NO se expone.**
- **Recorte obligatorio del listado** (R2): nunca devolver las 6.273 notas; `limit` por defecto pequeño (20–50) y campos mínimos (`path`/`title`/`folder`/`tags`/`updatedAt`). `search` limitado. `read` con tope de caracteres (p. ej. 20.000, como `rss_get_entry`).
- **Carpeta por defecto** de las notas creadas sin destino: `Valet` (ajuste `zennotes_folder`); la API crea las carpetas que falten.
- Nombre `zennotes` para no colisionar con la tool interna `notes` (notas SQLite del dominio `recuerdos`).

### 3.3 Skill `zennotes`
- Nueva variante `Skill::Zennotes`, id `zennotes`, `prompt_key: "SKILL_ZENNOTES_PROMPT"`, `prompt_heading: "# SKILL ACTIVA: ZENNOTES"`, `tools: ["zennotes"]`, threshold `0.10`.
- Criterios `criteria_true`/`criteria_false` que distingan del `recuerdos` interno: el **vault de ZenNotes** (artículos, podcasts, notas del vault, «mis notas del vault») vs. el apunte interno de Valet.

### 3.4 Ajustes, migración y endpoints
- Claves nuevas: `zennotes_base_url` (default `https://zennotes.territoriolinux.es`), `zennotes_folder` (default `Valet`), `zennotes_token` (**sensible**).
- `zennotes_token` → añadir a `SENSITIVE_KEYS`. Como `PUT /api/settings` ignora los sensibles, se añade **endpoint dedicado** (molde Strava): `GET /api/zennotes/status` → `{configured, base_url, folder}` (sin token) y `PUT /api/zennotes/token` → `{token}` no vacío (guarda; responde sin eco). 
- Migración SQL: siembra `zennotes_base_url`, `zennotes_folder`, `zennotes_token=''`, el fragmento `SKILL_ZENNOTES_PROMPT` y los defaults del router (`ROUTER_SKILL_ZENNOTES_ENABLED`, umbral).

### 3.5 Registro y router
- Registrar la tool `zennotes` en `build_tool_registry` (`src/lib.rs`).
- Añadir la skill al catálogo + al enum + a `ALL_SKILLS` (los tests de integridad exigen que toda tool registrada esté cubierta por core o skill).

### 3.6 Frontend
- Ajustes → API Keys: campo **password** «ZenNotes · Token» que **nunca** muestra el valor guardado (estado «Configurado / No configurado» vía `GET /api/zennotes/status`); al guardar, `PUT /api/zennotes/token`. (Opcional: botón «Probar conexión».)

## 4. Tareas (checklist)
### Fase 1 — OpenSpec (SDD)
- [ ] 1.1 Crear `openspec/changes/zennotes-notes/` con `proposal.md`, deltas de spec (`tools/zennotes`, `zennotes-api`, ajustes/UI) y `tasks.md`.
- [ ] 1.2 Definir contratos (tipos Rust/TS, endpoints, firmas) y escenarios BDD (Given/When/Then) para happy path, errores y casos límite.
- [ ] 1.3 **STOP**: presentar y esperar aprobación explícita antes de escribir código.
### Fase 2 — TDD backend
- [ ] 2.1 (RED) Tests de `services/zennotes.rs` (config, errores, wiremock, **no filtración del token**).
- [ ] 2.2 (GREEN) Implementar `services/zennotes.rs`.
- [ ] 2.3 (RED/GREEN) Migración SQL (settings + prompt + router) + tests.
- [ ] 2.4 (RED/GREEN) Tool `zennotes` (operaciones, permisos, **recorte del listado**) + tests.
- [ ] 2.5 (RED/GREEN) Skill `zennotes` en el catálogo + registro en el registry + tests de integridad.
- [ ] 2.6 (RED/GREEN) Endpoints `GET /api/zennotes/status` + `PUT /api/zennotes/token` + `SENSITIVE_KEYS` + tests.
- [ ] 2.7 (REFACTOR) `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`.
### Fase 2 — TDD frontend
- [ ] 2.8 (RED/GREEN) Campo de token + estado en Ajustes → API Keys + tests (`vitest run`, `tsc --noEmit`, `eslint`).
### Fase 3 — Auditoría
- [ ] 3.1 `@rust-reviewer` sobre el backend.
- [ ] 3.2 `@react-reviewer` sobre el frontend.
- [ ] 3.3 Corregir hallazgos y re-auditar.
### Fase 4 — Verificación
- [ ] 4.1 `@qa-plan-verifier` contra este `PLAN.md`.
- [ ] 4.2 E2E con el servidor real: guardar token, pedir a Valet «apunta en mi vault…» y comprobar que **llama** a `zennotes` y crea en `Valet/`.
- [ ] 4.3 Medir enrutado (skill `zennotes` vs `recuerdos`) con `valet-route-eval`.
- [ ] 4.4 Archivar el change (`openspec archive`) y cerrar el plan (`plans/PLAN-009-COMPLETED.md`).

## 5. Riesgos y dependencias
- **R1 (alto) Fiabilidad del agente al llamar herramientas.** Contexto reciente: el agente responde/promete y **no** llama a la tool (y llega a alucinar). Esta feature es inservible si eso no se arregla. Dependencia del arreglo del `system_prompt` (pendiente de decisión).
- **R2 (alto) Volumen.** `GET /api/notes` = 3,3 MB / 6.273 notas → recorte y límites obligatorios; nunca exponer el listado completo.
- **R3 (medio) Solapamiento de router** `zennotes` ↔ `recuerdos`. Mitigar con criterios explícitos y medir con `valet-route-eval`.
- **R4 (medio) Token.** Contradicción con el modelo actual (los sensibles no se escriben por `PUT /api/settings`) → se resuelve con endpoint dedicado.
- **R5 (bajo) Destructivas.** `delete`/`empty-trash`: se expone `delete` con `ExplicitApproval`; `empty-trash` queda fuera.
- **R6 (bajo) Consistencia con §V.9 (MCP descartado).** Esta integración es **nativa por HTTP**, alineada con la alternativa vigente (integración nativa por herramienta).

## 6. Fuera de alcance
- Vista de Notas en la app (D4: solo herramientas).
- Integración por MCP (§V.9).
- Assets/imágenes del vault (solo notas).
- Escritura de *tasks* de ZenNotes (`/api/tasks`): no es objetivo.
