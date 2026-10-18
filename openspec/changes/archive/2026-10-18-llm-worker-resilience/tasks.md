# Tasks

## 0. Aprobación

- [x] 0.1 Presentar este change (proposal + design + specs + tasks) y **esperar el OK explícito** del usuario antes de escribir código o tests.

## 1. RED — proveedor

- [x] 1.1 Test que falla: `ChatResponse` expone `finish_reason` cuando el proveedor lo envía (`stop`).
- [x] 1.2 Test que falla: `finish_reason` es `None` cuando el proveedor no lo envía (p. ej. Ollama).
- [x] 1.3 Test que falla: `finish_reason = length` se parsea como `FinishReason::Length`.
- [x] 1.4 Test que falla: un cuerpo HTTP 200 **sin `choices`** (p. ej. `{"error": …}`) es un error, no una respuesta vacía.

## 2. RED — workers

- [x] 2.1 Test que falla: al truncar por presupuesto (`finish_reason = length`), el reintento de la consolidación usa el **doble** de presupuesto.
- [x] 2.2 Test que falla: un fallo de la consolidación **degrada** la pasada —escribe ficha y marca, conserva el estado anterior— en vez de abortar.
- [x] 2.3 Test que falla: la fila `status='error'` del consolidador incluye el **motivo de finalización** en el diagnóstico. (La fila `status='error'` ya existe: la cubrió `episodic-call-integrity`, archivado el 2026-10-12; lo que falta es el motivo, que depende del grupo 1.)
- [x] 2.4 Test que falla: un resumen vacío o solo-espacios **no se escribe** en `messages.collapsed_content` y el mensaje queda sin colapsar.

## 3. GREEN

- [x] 3.1 `src/llm/provider.rs`: `FinishReason` y `finish_reason: Option<FinishReason>` en `ChatResponse`.
- [x] 3.2 `src/llm/openrouter.rs`: parseo de `finish_reason`; un cuerpo 200 sin `choices` es un error.
- [x] 3.3 `src/workers/episodic_memory.rs`: reintento con presupuesto doblado al truncar, degradación de la pasada y fila de error en `llm_requests`.
- [x] 3.4 `src/workers/collapse.rs`: guarda que impide escribir un resumen vacío o degenerado; lo registra como error.

## 4. Plantillas

- [x] 4.1 `.env.j2`: `SEMANTIC_MODEL=qwen/qwen3-235b-a22b-2507`, `COLLAPSE_MODEL=mistralai/mistral-small-24b-instruct-2501`, `MEMORY_MODEL=mistralai/mistral-small-24b-instruct-2501`.
- [x] 4.2 `.env.example`: los mismos modelos.

## 5. Verificación y cierre

- [x] 5.1 `cargo test` en verde (baseline: 1423 passed).
- [x] 5.2 `cargo clippy --all-targets -- -D warnings` (0 warnings).
- [x] 5.3 `cargo fmt --check` en verde.
- [x] 5.4 `openspec validate llm-worker-resilience --strict` en verde.
- [x] 5.5 Revisión con `@rust-reviewer`; hallazgos aplicados.
- [x] 5.6 `openspec archive llm-worker-resilience` y registrar el cierre en `AGENTS.md` §V. El **Tema 5 de `plans/PLAN-003-COMPLETED.md` NO se toca**: el plan está completado y congelado, y §VI de `AGENTS.md` dice que el cuerpo de un plan completado no se reescribe porque es registro histórico.
