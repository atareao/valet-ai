# Tasks: Retirar la ruta muerta `process_message`

## Bloque 1 — Conservar y migrar la cobertura única
- [x] 1.1 Migrar el test `episodic_memory_block_precedes_history` a `process_message_stream` (no tiene gemelo en streaming).
- [x] 1.2 RED: migrar `test_process_message_records_stats_on_llm_error` a streaming. Falla porque el stream no registra la fila de error.
- [x] 1.3 GREEN: en `process_message_stream`, registrar `StatsRepo::record_request` con status `"error"` y `save_last_call` con `"error"` cuando falle `chat_stream()` o el stream.

## Bloque 2 — Borrar la ruta muerta
- [x] 2.1 Borrar `Orchestrator::process_message` (líneas 325–666) y el código que quede sin uso.
- [x] 2.2 Borrar los 5 tests duplicados: `test_process_message_uses_system_prompt_from_db`, `injects_episodic_memory_block_in_process_message`, `episodic_memory_block_is_identical_in_both_paths`, `test_orchestrator_injects_profile_id_into_tool_call`, `test_process_message_records_stats`.
- [x] 2.3 Ajustar o retirar helpers de test que solo servían a la ruta muerta (`build_orchestrator_with_full_capture` si queda huérfano).

## Bloque 3 — Specs
- [ ] 3.1 Aplicar el delta: los 3 requisitos reescritos y sin los escenarios de la ruta muerta (esto se consolida al archivar).

## Bloque 4 — Verificación
- [x] 4.1 `grep -rn 'process_message\b' src/` sin referencias fuera de `process_message_stream`.
- [x] 4.2 `cargo fmt --check` limpio.
- [x] 4.3 `cargo clippy --all-targets -- -D warnings` limpio.
- [x] 4.4 `cargo test` completo en verde, sin regresiones sobre el baseline.
- [x] 4.5 `openspec validate --strict` válido.
