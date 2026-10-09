# Tasks: harden-consolidator

## Bloque 0 — Baseline (antes de tocar nada)
- [x] 0.1 Registrar el baseline por CLI: `cargo test --no-fail-fast` (passed/failed), `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`.
- [x] 0.2 Inventario: `grep -rn "Effort(Low)\|semantic_model\|GENERATION_SEMANTIC_REASONING" src/` y localizar `consolidate_state`/`call_semantic_chat` y la plantilla de respaldo.

## Bloque 1 — RED
- [x] 1.1 `src/generation.rs`: test que verifica que el default del rol `Semantic` es `reasoning = Some(ReasoningSpec::Off)`. Hoy es `Effort(Low)` → rojo.
- [x] 1.2 `src/workers/episodic_memory.rs`: con un `LLMProvider` de prueba que devuelve contenido vacío y luego un JSON válido, `consolidate_state`/la consolidación inicial DEBE tener éxito (reintento). Hoy aborta con el primer vacío → rojo.
- [x] 1.3 `src/workers/episodic_memory.rs`: si ambos intentos devuelven vacío/no-JSON, el error DEBE incluir `content_len`. Hoy el mensaje es genérico → rojo.
- [x] 1.4 Migración (test de settings): tras `run_migrations` en una BD con `GENERATION_SEMANTIC_REASONING = low`, el valor queda en `off`, y un valor distinto (p. ej. `medium`) se respeta. Rojo sin la migración nueva.
- [x] 1.5 Prompt sembrado: tras `run_migrations`, `consolidator_prompt` contiene la taxonomía (`preferences_and_tastes`, `dislikes_and_dealbreakers`) y ambos placeholders. Rojo con el prompt viejo.
- [x] 1.6 Presupuesto: tras `run_migrations`, `PERSISTENT_MEMORY_BUDGET_TOKENS` vale `800`, y `read_budget` cae a `800` cuando falta la clave. Rojo con el valor viejo.
- [x] 1.7 Verificar por CLI: 1.1–1.6 en rojo por aserción y el resto en verde.

## Bloque 2 — GREEN
- [x] 2.1 `src/generation.rs`: default `Semantic => (0.1, "off", 2048)`.
- [x] 2.2 Nueva migración `migrations/20261003000003_consolidator_reliability.sql`: corrige `low→off` (solo si `low`), sube `PERSISTENT_MEMORY_BUDGET_TOKENS` `500→800` (solo si `500`) y actualiza `consolidator_prompt` solo si es el default anterior exacto. No editar migraciones existentes.
- [x] 2.3 `src/persistent_memory.rs`: `PERSISTENT_MEMORY_BUDGET_TOKENS_DEFAULT = 800` y tests de `read_budget` actualizados.
- [x] 2.4 `src/workers/episodic_memory.rs`: plantilla de respaldo = V8; reintento único de la consolidación inicial (vacío / no-JSON / esquema inválido); error con `content_len` y preview.
- [x] 2.5 Tests GREEN de 1.1–1.6.
- [x] 2.6 Verificar por CLI: `cargo test --no-fail-fast` (0 failed), `cargo check`.

## Bloque 3 — REFACTOR y cierre
- [x] 3.1 `cargo fmt --check` y `cargo clippy --all-targets -- -D warnings` limpios.
- [x] 3.2 Revisión con `rust-reviewer` y `sqlite-expert` (migración).
- [x] 3.3 PR a `development`; tras el merge, PR de archivado (`openspec archive harden-consolidator`).
