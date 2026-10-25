# Tasks

## 0. Aprobación

- [x] 0.1 Presentar este change (proposal + design + specs + tasks) y OK explícito dado del usuario antes de escribir código o tests.

## 1. RED — ajustes

- [x] 1.1 Test que falla: existe una migración que siembra `REACT_TOOL_FINGERPRINT_LIMIT=2`, `REACT_TOOL_FAILURE_LIMIT=3`, `REACT_TOOL_SUCCESS_LIMIT=50` y `REACT_TOOL_TOTAL_LIMIT=100` (guardián de contenido de migración, mismo idioma que el de los `GENERATION_*`).
- [x] 1.2 Test que falla: tras migrar, `SettingsRepo::get` devuelve esos cuatro valores.
- [x] 1.3 Test que falla: `read_tool_limits` devuelve los defaults cuando las claves faltan.
- [x] 1.4 Test que falla: `read_tool_limits` cae al default con warning si un valor no es parseable.

## 2. RED — guardas del bucle

- [x] 2.1 Test que falla: N llamadas con **argumentos distintos** al mismo `tool::operation` (todas exitosas) NO se bloquean mientras N ≤ `REACT_TOOL_SUCCESS_LIMIT`.
- [x] 2.2 Test que falla: alcanzado `REACT_TOOL_SUCCESS_LIMIT` éxitos, la siguiente llamada al mismo `tool::operation` se bloquea y el mensaje indica el motivo «éxito».
- [x] 2.3 Test que falla: `REACT_TOOL_FAILURE_LIMIT` fallos **consecutivos** bloquean; un éxito intermedio **reinicia** el contador de fallos.
- [x] 2.4 Test que falla: la **misma** llamada (mismo tool+operation+args) se bloquea tras `REACT_TOOL_FINGERPRINT_LIMIT` ejecuciones, aunque cambie el resultado entre medias.
- [x] 2.5 Test que falla: dos llamadas con argumentos distintos al mismo tool NO comparten huella (solo comparten la capa 2).
- [x] 2.6 Test que falla: alcanzado `REACT_TOOL_TOTAL_LIMIT` llamadas en el turno, se bloquea cualquier herramienta.
- [x] 2.7 Test que falla: el mensaje al modelo es **exacto** y nombra el motivo (huella/fallo/éxito/global), no «3 times».
- [x] 2.8 Test que falla: los contadores se **reinician** en el turno siguiente.
- [x] 2.9 Test que falla: al bloquear se emite `SSEEvent::ToolResult { success: false }` (comportamiento actual conservado) y se registra un `warn!` con nombre y motivo.

## 3. GREEN

- [x] 3.1 `migrations/2026…_react_tool_guards.sql`: sembrado idempotente (upsert que solo sobrescribe valores vacíos/NULL) de las cuatro claves.
- [x] 3.2 `src/orchestrator/tool_limits.rs`: `ToolLimits` + `read_tool_limits` (defaults + `warn!`).
- [x] 3.3 `src/orchestrator/agent.rs`: sustituir el contador único por las tres guardas; mensaje exacto por motivo; eliminar `const MAX_TOOL_RETRIES`.
- [x] 3.4 Actualizar los tests existentes que usan `MAX_TOOL_RETRIES` (p. ej. el bloque en `agent.rs` ~4671–4817) al nuevo contrato.

## 4. Interfaz

- [x] 4.1 `SettingsDialog.tsx`: sección «Límites del agente» con los cuatro campos numéricos (min 1); integrarla en el formulario y en el estado (`useSettings` / tipos).
- [x] 4.3 (react-reviewer #2) Validación «entero ≥ 1» en los cuatro campos (`precision={0}` + `rules`), con test de valor inválido.
- [x] 4.2 Test de componente: los cuatro campos se renderizan con los valores de `settings` y se envían al guardar.

## 5. Verificación y cierre

- [x] 5.1 `cargo test` en verde.
- [x] 5.2 `cargo clippy --all-targets -- -D warnings` (0 warnings).
- [x] 5.3 `cargo fmt --check` en verde.
- [x] 5.4 `npx tsc --noEmit` y `npx vitest run` en verde.
- [x] 5.5 `openspec validate react-tool-guards --strict` en verde.
- [x] 5.6 Revisión con `@rust-reviewer` (backend) y `@react-reviewer` (UI); hallazgos aplicados.
- [x] 5.7 `openspec archive react-tool-guards` y registrar el cierre en `AGENTS.md` §V.

## 6. Hallazgos de auditoría (@rust-reviewer)

- [x] 6.1 (M2) Test que **ate** los defaults de Rust (`tool_limits.rs`) con los valores sembrados por la migración: tras migrar, `SettingsRepo::get` de las cuatro claves SHALL igualar los `DEFAULT_*`. *(M1 resuelto alineando `design.md` y el delta de spec con la semántica real, sin cambio de código.)*
- [x] 6.2 (N1) Endurecer la aserción de `guard_block_emits_failure_event_and_warns`: afirmar la línea concreta (`guard=<motivo>` y `ReAct tool guard tripped`), no una subcadena laxa como `"success"`.
- [x] 6.3 (N3) `src/lib.rs`: `pub mod test_support` → `pub(crate) mod test_support`.
