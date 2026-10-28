# Tasks — opentodo-integration

## 0. Aprobación

- [x] 0.1 Presentar el change (proposal + design + specs + tasks) y obtener OK explícito del usuario antes de escribir código o tests.
- [x] 0.2 `openspec validate opentodo-integration --strict`.

## 1. RED — servicio

- [x] 1.1 Test: `OpentodoConfig` se resuelve desde `settings` (`opentodo_base_url`, `opentodo_token`) y, si falta, del entorno (`OPENTODO_BASE_URL`, `OPENTODO_TOKEN`).
- [x] 1.2 Test: sin token se obtiene `NotConfigured` (distinto de `Unauthorized`).
- [x] 1.3 Test: `401`→`Unauthorized`, `404`→`NotFound`, red/`5xx`→`Upstream`, `400`→`InvalidInput`.
- [x] 1.4 Test: el `Debug` de la config y todo `Display` de `OpentodoError` **nunca** contienen el token.
- [x] 1.5 Test (wiremock): una tarea con campos ausentes (`start`, `deadline`, `evening`, `labels`) deserializa sin error y con `Option`.
- [x] 1.6 Test (wiremock): `priority` se envía siempre numérica 1..4 y se lee de vuelta como `critical|high|medium|low`.
- [x] 1.7 Test (wiremock): un `409` de `Idempotency-Key` se trata como «ya aplicado» y no se reintenta.
- [x] 1.8 Test: las vistas `inbox|today|upcoming|anytime|someday|doing|waiting` clasifican vectores fijos; una tarea puede salir en varias y `all` excluye las completadas.
- [x] 1.9 Test: `set_label` **no borra** etiquetas ajenas, es excluyente dentro del grupo y con `value: null` solo quita las del grupo.
- [x] 1.10 Test: una etiqueta homónima ya existente se adopta (no se crea un duplicado) y una ausente se crea una vez.
- [x] 1.11 Ejecutar `cargo test` y confirmar el fallo de estos tests con el resto en verde.

## 2. GREEN — servicio

- [x] 2.1 `src/services/opentodo.rs`: config resuelta por llamada, cliente, modelos tolerantes, vistas, resolución de proyectos/etiquetas y `set_label`.
- [x] 2.2 `src/services/mod.rs`: exportar `opentodo`.
- [x] 2.3 `cargo test` en verde.

## 3. RED/GREEN — ajustes y endpoints

- [x] 3.1 Test: `opentodo_token` figura en `SENSITIVE_KEYS` (no aparece en `GET /api/settings` y se ignora en `PUT /api/settings`); `opentodo_base_url` **sí** viaja en los ajustes.
- [x] 3.2 Test: `GET /api/opentodo/status` devuelve `{configured, base_url}` sin el token.
- [x] 3.3 Test: `PUT /api/opentodo/token` vacío → `400`; válido → `204` y guardado recortado.
- [x] 3.4 `src/handlers/opentodo.rs`, `src/routes/opentodo.rs`, `src/routes/mod.rs`, `SENSITIVE_KEYS`.
- [x] 3.5 Test: `GET /api/opentodo/check` sondea de verdad (`GET /api/v1/me`), responde siempre `200` con `{ok, detail}` en sus cuatro salidas y **nunca** el token.

## 4. RED/GREEN — migración

- [x] 4.1 Test guardián: la migración siembra `opentodo_base_url` (`https://todo.territoriolinux.es`) y `opentodo_token` (vacío).
- [x] 4.2 Test: la migración promueve `SKILL_PENDIENTES_PROMPT` solo si su valor es exactamente el sembrado por `20261012000003`, conserva la primera línea `# SKILL ACTIVA: PENDIENTES` y **no** pisa una edición del usuario.
- [x] 4.3 Test: el fragmento nuevo menciona OpenTodo, las vistas, la prioridad 1–4, `someday` y que los recordatorios son otra cosa.
- [x] 4.4 `migrations/20261027000001_opentodo.sql`.

## 5. RED/GREEN — herramienta `tasks`

- [x] 5.1 Test: `parameters()` declara `operation` con el enum de siete valores y `required: ["operation"]`, más `view`, `limit`, `project`, `priority`, `due`, `start`, `group`, `value`, `purge`, `query`.
- [x] 5.2 Test: `permission()` → `NoConfirm` en `list_tasks`/`search_tasks`, `Notify` en `add_task`/`update_task`/`complete_task`/`set_label`, `ExplicitApproval` en `delete_task` (con y sin `purge`).
- [x] 5.3 Test: con `opentodo_token` configurado, `list_tasks` golpea la API de OpenTodo y devuelve como mucho `limit` tareas con campos mínimos más el `summary`.
- [x] 5.4 Test: `limit` fuera de `1..=50` → `InvalidArguments` **sin** salir a la red.
- [x] 5.5 Test: sin token, la herramienta conserva el comportamiento local actual (tabla `tasks`) y `profile_id` sigue funcionando.
- [x] 5.6 Test: `add_task` sin prioridad envía `4`; con `priority: "critical"` envía `1`.
- [x] 5.7 Test: `delete_task` con `purge: true` llama al endpoint de purga.
- [x] 5.8 Test: un fallo de OpenTodo devuelve un error legible del turno, no un pánico ni un `unwrap`.
- [x] 5.9 `src/tools/tasks.rs` y `src/tools/mod.rs`.

## 6. REFACTOR — backend

- [x] 6.1 `cargo fmt --check`
- [x] 6.2 `cargo clippy -- -D warnings`
- [x] 6.3 `cargo test` (todo en verde).

## 7. RED/GREEN — frontend

- [x] 7.1 Test: «Integraciones» muestra **ocho** sub-pestañas, la octava «OpenTodo», y el panel tiene campo de token, indicador y botón «Probar conexión».
- [x] 7.2 Test: el campo del token arranca **vacío** aunque haya token guardado, y guardar llama a `PUT /api/opentodo/token` (no a `PUT /api/settings`).
- [x] 7.3 Test: el indicador «Configurado / Sin configurar» sale de `GET /api/opentodo/status` y es visible desde el primer render.
- [x] 7.4 `keyIntegrations.ts`, `OpentodoSection.tsx`, `useOpentodo.ts`, `IntegrationsTab.tsx`, `types.ts`.
- [x] 7.5 `npx vitest run`, `npx tsc --noEmit`, `npm run lint`.
- [x] 7.6 Test: «Probar conexión» usa `GET /api/opentodo/check` (no `/status`) y muestra el `detail` como texto en la línea de estado.
- [x] 7.7 Test: cuando `GET /api/opentodo/status` falla, el panel y el indicador muestran la misma etiqueta («Sin datos») y no «Sin configurar».

## 8. Auditoría

- [x] 8.1 `@rust-reviewer` sobre el backend.
- [x] 8.2 `@react-reviewer` sobre el frontend.
- [x] 8.3 Corregir hallazgos y re-auditar.

## 9. Verificación y cierre

- [x] 9.1 `@qa-plan-verifier` contra `PLAN.md`.
- [x] 9.2 `openspec archive opentodo-integration`.
- [x] 9.3 E2E con el servidor real: guardar el token, crear/editar/completar/borrar una tarea por la herramienta, y dejar la cuenta como estaba (borrar + purgar + etiquetas). — Ejecutada por el usuario contra producción el 2026-10-28, con el token configurado en la UI de Ajustes → Integraciones → OpenTodo sobre la v0.23.0 desplegada en `co1`.
