# Tasks

## 0. Aprobación

- [x] 0.1 Presentar el change (proposal + design + specs + tasks) y obtener OK explícito del usuario antes de escribir código o tests.

## 1. RED — servicio

- [x] 1.1 Test que falla: `ZennotesConfig` se resuelve desde `settings` (`zennotes_base_url`, `zennotes_token`, `zennotes_folder`).
- [x] 1.2 Test que falla: sin token en settings ni entorno, se obtiene `MissingCredentials` (distinto de `Unauthorized`).
- [x] 1.3 Test que falla: `GET /api/notes/read` mapea `401`→`Unauthorized`, `404`→`NotFound`, red/`5xx`→`Unavailable`.
- [x] 1.4 Test que falla: el `Debug` de la config y el mensaje de todo `ZennotesError` **nunca** contienen el token.
- [x] 1.5 Test que falla (wiremock): `create` con cuerpo hace dos viajes (create + write); `append`/`replace` hacen read + write.

## 2. GREEN — servicio

- [x] 2.1 `src/services/zennotes.rs`: cliente + `ZennotesConfig` + `ZennotesError` + operaciones (vault, list, read, search_text, create, append, replace, write, move, archive/unarchive, rename, trash/restore, delete).
- [x] 2.2 `src/services/mod.rs`: exportar `zennotes`.

## 3. RED/GREEN — migración

- [x] 3.1 Test guardián de contenido: la migración siembra `zennotes_base_url`, `zennotes_folder`, `zennotes_token` (vacío), `SKILL_ZENNOTES_PROMPT` y `ROUTER_SKILL_ZENNOTES_ENABLED`.
- [x] 3.2 Test que falla: tras migrar, `SettingsRepo::get` devuelve los valores por defecto; y la migración **respeta** un valor ya fijado (upsert idempotente).
- [x] 3.3 `migrations/*.sql`: siembra idempotente.

## 4. RED/GREEN — herramienta `zennotes`

- [x] 4.1 Test que falla: `parameters()` declara `operation` con el enum completo y `required: ["operation"]`.
- [x] 4.2 Test que falla: `permission(args)` devuelve `NoConfirm`/`Notify`/`ExplicitApproval` según la tabla del diseño.
- [x] 4.3 Test que falla: `list` devuelve como mucho `limit` notas (def 20, máx 200) y **solo** los campos recortados; un `limit` fuera de rango → `InvalidArguments` sin salir a la red.
- [x] 4.4 Test que falla: `read` recorta el `body` a 20.000 caracteres e indica el recorte.
- [x] 4.5 Test que falla: `create` sin destino usa la carpeta `Valet`; `search` de texto devuelve coincidencias con `lineText`.
- [x] 4.6 Test que falla: operación desconocida y argumentos obligatorios ausentes → `InvalidArguments`.
- [x] 4.7 `src/tools/zennotes.rs` + `src/tools/mod.rs`.

## 5. RED/GREEN — skill y registro

- [x] 5.1 Test que falla: `Skill::Zennotes` está en el catálogo (id `zennotes`, umbral `0.10`, `tools: ["zennotes"]`, prompt `SKILL_ZENNOTES_PROMPT`).
- [x] 5.2 Test que falla: toda herramienta registrada (incluida `zennotes`) está cubierta por core o skill.
- [x] 5.3 `src/orchestrator/skills.rs`: variante + entrada de catálogo + `ALL_SKILLS`; `src/lib.rs`: registro de `zennotes`.

## 6. RED/GREEN — ajustes y endpoints de token

- [x] 6.1 Test que falla: `zennotes_token` está en `SENSITIVE_KEYS` (no aparece en `GET /api/settings` y se ignora en `PUT /api/settings`).
- [x] 6.2 Test que falla: `GET /api/zennotes/status` devuelve `{configured,base_url,folder}` sin el token.
- [x] 6.3 Test que falla: `PUT /api/zennotes/token` con token vacío → `400`; con token válido guarda y devuelve `204`.

## 7. REFACTOR — backend

- [x] 7.1 `cargo fmt --check`
- [x] 7.2 `cargo clippy -- -D warnings`
- [x] 7.3 `cargo test` (todo en verde).

## 8. RED/GREEN — frontend

- [x] 8.1 Test que falla: la pestaña «API Keys» renderiza un campo de token de ZenNotes que **nunca** muestra el valor guardado y muestra «Configurado / No configurado».
- [x] 8.2 Test que falla: al guardar, se llama al endpoint `PUT /api/zennotes/token` (no a `PUT /api/settings` con `zennotes_token`).
- [x] 8.3 `SettingsDialog.tsx` (+ tipos y hook de ajustes).
- [x] 8.4 `npx vitest run`, `npx tsc --noEmit`, `npm run lint`.

## 9. Auditoría

- [x] 9.1 `@rust-reviewer` sobre el backend.
- [x] 9.2 `@react-reviewer` sobre el frontend.
- [x] 9.3 Corregir hallazgos y re-auditar. — Ambas auditorías APROBADAS sin bloqueantes. Hallazgos menores (opcionales): expect en Zennotes::new (replicado del molde miniflux), cobertura opcional de tests de ciclo de vida a nivel de tool, y un test de frontend que mockea funciones en vez de la URL. Deriva doc de `create` ya corregida en design.md y frontend/spec.md.

## 10. Verificación y cierre

- [x] 10.1 `@qa-plan-verifier` contra `PLAN.md`. — Todas las fases 1–8 verificadas por CLI, en verde.
- [x] 10.2 E2E con el servidor real: guardar token y comprobar que el agente **llama** a `zennotes` y crea en `Valet/`. — Verificado contra ZenNotes real vía la propia tool: list recorta (count=6273, 5 devueltas), create sin destino nace en inbox/Valet/, y trash+delete deja la nota en NotFound.
- [x] 10.3 Medir el enrutado (skill `zennotes` vs `recuerdos`) con `valet-route-eval`. — Cobertura 100% (R3 grave descartado). Sobre-activación en turnos internos corregida endureciendo criteria (0.78 → 0.07) sin perder cobertura.
- [x] 10.4 `openspec archive zennotes-notes`. — Archivado como 2026-10-26-zennotes-notes.
- [x] 10.5 E2E real: corregido el default de `create`. En la API del servidor `folder` es un enum restringido a `inbox`/`quick`/`archive` (un `folder` desconocido, como `Valet`, devuelve 500) y `Valet` es en realidad el **subpath**; el servidor **sí autocrea** el subpath (`os.MkdirAll`, verificado con `inbox/ValetSondeoNovel`). Ahora `create` usa `folder=inbox` y `subpath=zennotes_folder` (`Valet`) por defecto, de modo que la nota nace en `inbox/Valet/`. Sonda real contra `https://zennotes.territoriolinux.es`: `GET /api/folders` 200, `create {folder:"inbox",subpath:"Valet"}` 200 → `Valet/<título>.md`, limpieza con `trash`+`delete` (`POST /api/folders/delete` para las carpetas).
