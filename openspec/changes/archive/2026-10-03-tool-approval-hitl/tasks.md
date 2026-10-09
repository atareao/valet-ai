# Tasks: tool-approval-hitl

## Bloque 0 — Baseline (antes de tocar nada)
- [x] 0.1 Registrar el baseline por CLI: `cargo test --no-fail-fast` (passed/failed), `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` y `cd frontend && npx vitest run`.
- [x] 0.2 Inventario: `grep -rn "fn permission" src/` para localizar los 12 tools y los mocks de test.

## Bloque 1 — RED (comportamiento observable con la API actual)
- [x] 1.1 `src/orchestrator/guardrails.rs` (tests): `check("calendar", {"operation":"delete_event","id":"x"})` devuelve `RequiresApproval`. Hoy devuelve `Allowed` porque `check` ignora los args → rojo. Construir `CalendarTool` con un pool perezoso (nunca se toca la BD en `check`).
- [x] 1.2 `src/orchestrator/guardrails.rs` (tests): `check("tasks", {"operation":"delete_task","id":"x"})` devuelve `RequiresApproval`. Rojo.
- [x] 1.3 `tests/api/chat.rs`: `test_approval_endpoint_with_deny` debe aseverar `approved: false` (el stub debe reflejar el cuerpo). Hoy devuelve `true` → rojo.
- [x] 1.4 Verificar por CLI: 1.1–1.3 en rojo y el resto en verde. Los tests que requieren API nueva (`await_approval`, `approval_result`, UI) van en GREEN para no romper la compilación de la suite; se documenta en el informe de RED.

## Bloque 2 — GREEN (implementar)
- [x] 2.1 `src/tools/trait.rs`: `fn permission(&self, args: &Value) -> Permission`; actualizar el mock y su test.
- [x] 2.2 `src/tools/registry.rs`: `permission(name, args)`; actualizar el mock y los tests.
- [x] 2.3 Los 12 tools: firma nueva; sin operaciones devuelven su permiso actual ignorando `args`.
- [x] 2.4 `src/tools/calendar.rs`: permiso por `operation` — lectura `NoConfirm`, `create_event`/`update_event` `Notify`, `delete_event` `ExplicitApproval`.
- [x] 2.5 `src/tools/tasks.rs`: `delete_task` `ExplicitApproval`, resto `NoConfirm`.
- [x] 2.6 `src/orchestrator/guardrails.rs`: `check` usa `registry.permission(name, args)`; la solicitud guarda un `oneshot::Sender`; `await_approval(id, timeout) -> ApprovalOutcome`; `resolve_approval` envía y respeta 404/409; limpieza tras consumir/expirar. Mocks de test actualizados.
- [x] 2.7 `src/orchestrator/agent.rs`: en `RequiresApproval`, emitir `ApprovalRequired`, `await_approval` y continuar (aprobado → ejecutar; denegado/expira/cancelado → `role:"tool"` de rechazo y seguir). Emitir `ApprovalResult`. Eliminar el `return Err`.
- [x] 2.8 `src/routes/stream.rs`: el stub refleja `approved`; documentar 200/404/409.
- [x] 2.9 Tests GREEN: guardrails (aprobar/denegar/expirar/cancelar), agent end-to-end (aprueba → ejecuta y `Done`; deniega → no ejecuta y `Done`), endpoint (deny).
- [x] 2.10 Frontend: `onApprovalRequired` cableado en `useMainChat`; estado `pendingApproval`; diálogo en `ChatView`; llamada a `api.approveAction`; limpieza al `approval_result`/`done`/`error`. Test de vitest.
- [x] 2.11 Verificar por CLI: `cargo test --no-fail-fast` (0 failed), `cargo check`, `npx vitest run`, `npx tsc --noEmit`.

## Bloque 3 — Documentación
- [x] 3.1 `README.md`/`README.es.md`: documentar el permiso por operación y la aprobación (endpoint y eventos SSE) si aplica.
- [x] 3.2 Revisar que las 7 spec deltas describen lo implementado.

## Bloque 4 — REFACTOR y cierre
- [x] 4.1 `cargo fmt --check` y `cargo clippy --all-targets -- -D warnings` limpios; `npm run lint`.
- [x] 4.2 `grep -rn "fn permission" src/` sin impls con la firma vieja.
- [x] 4.3 Revisión con `rust-reviewer` (backend) y `react-reviewer` (frontend).
- [x] 4.4 PR a `development`; tras el merge, PR de archivado (`openspec archive tool-approval-hitl`).
