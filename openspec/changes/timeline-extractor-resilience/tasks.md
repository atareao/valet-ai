# Tasks

Todas las casillas empiezan sin marcar. **No se escribe ni una línea de código ni un solo test antes de que el usuario apruebe este change.**

## 0. Aprobación (punto de parada del protocolo)

- [x] 0.1 Presentar este change (proposal + delta de `workers` + design + tasks) y **esperar aprobación explícita** antes de tocar código.

## 1. RED — parser y reintento

- [x] 1.1 RED — test unitario en `src/workers/episodic_memory.rs` (`#[cfg(test)]`, junto a los de `parse_timeline_events`): un extractor que responde `[]` produce **cero hechos** y la llamada queda `status = 'success'` (hoy: `NotJson` → `status = 'error'`). Comando: `cargo test --lib timeline` (ajusta el filtro si no selecciona nada). **Pegar la salida en rojo en el informe.**
- [x] 1.2 RED — test unitario: un extractor que falla el primer intento (sin el envoltorio `events`) y acierta el segundo persiste los hechos del segundo intento y la pasada no falla; y un extractor que falla **todos** los intentos deja la llamada en `status = 'error'` y la pasada sigue (ficha y `is_indexed` escritos).

## 2. GREEN — implementación mínima

- [x] 2.1 GREEN — tolerar un **array JSON vacío** como cero hechos en el parser del envoltorio, sin registrarlo como error.
- [x] 2.2 GREEN — reintentar la extracción del timeline un número acotado de veces (alineado con el patrón del consolidador) ante un fallo de forma, antes de registrar el error y continuar. Confirmar `cargo test --lib timeline` y `cargo test` en verde.

## 3. Configuración y medición (sin código)

- [ ] 3.1 Poner `TIMELINE_MODEL=deepseek/deepseek-v4.1-flash` en `.env` (gitignorado) y **confirmar** que `GENERATION_TIMELINE_REASONING = off` en `settings`. **Revisar antes** `COLLAPSE_MODEL` y `SEMANTIC_MODEL` del `.env`, que hoy no están aplicados en producción, porque recrear el contenedor los aplicará.
- [ ] 3.2 Recrear el contenedor y medir: `SELECT status, error_message, completion_tokens FROM llm_requests WHERE kind='timeline'` y el número de filas nuevas en `timeline_events`. Dejar constancia del antes y el después.

## 4. Verificación y cierre

- [x] 4.1 REFACTOR — `cargo fmt`, `cargo clippy --all-targets -- -D warnings` y `cargo test` (suite completa) en verde.
- [x] 4.2 `openspec validate --all` en verde.
- [ ] 4.3 PR a `development` con merge commit, revisado por `rust-reviewer`. Archive del change en PR aparte tras el merge.
