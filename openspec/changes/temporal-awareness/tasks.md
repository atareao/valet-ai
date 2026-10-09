# Tasks

## 0. Change proposal y aprobación

- [ ] 0.1 Presentar este change (proposal + specs + tasks) y **esperar aprobación explícita** antes de escribir código.

## 1. RED: fijar el contrato

- [ ] 1.1 Tests que fallan en `time_format`: `format_inline_timestamp` (`[YYYY-MM-DD HH:MM]` en la zona pedida) y `format_prompt_now` (`YYYY-MM-DD HH:MM:SS (weekday)`), con zona inválida → UTC y fecha inválida → `None`.
- [ ] 1.2 Tests que fallan en `agent`: el mensaje de sistema contiene la sección temporal **siempre** (con y sin `BrowserContext`) y **cierra** el mensaje.
- [ ] 1.3 Tests que fallan en `agent`: la sección de ubicación lleva solo ubicación/coordenadas y se omite sin coordenadas; ya no contiene fecha.
- [ ] 1.4 Tests que fallan en `agent`: los mensajes `user`/`assistant` del historial van prefijados con `[YYYY-MM-DD HH:MM]`; los `tool` y los de contenido vacío, no; una fecha ilegible no rompe.
- [ ] 1.5 Tests que fallan en `agent`: el mensaje del turno va prefijado con el instante efectivo.

## 2. GREEN — formateadores

- [ ] 2.1 `format_inline_timestamp(iso_utc, tz) -> Option<String>` en `src/tools/time_format.rs`.
- [ ] 2.2 `format_prompt_now(iso_utc, tz) -> Option<String>` y una variante para «ahora» del reloj del sistema.

## 3. GREEN — ensamblado en `agent.rs`

- [ ] 3.1 Resolver instante y zona efectivos del turno (navegador → `settings.timezone` → `Europe/Madrid`; UTC si es inválida).
- [ ] 3.2 Componer la sección temporal siempre presente y cerrar con ella el mensaje de sistema.
- [ ] 3.3 Reducir la sección de ubicación a la ubicación (sin fecha).
- [ ] 3.4 Prefijar el historial (`user`/`assistant`, contenido no vacío) y el mensaje del turno.

## 4. REFACTOR / limpieza

- [ ] 4.1 `cargo fmt` + `clippy --all-targets -D warnings` (0 warnings).
- [ ] 4.2 Código muerto del formato anterior de la sección «browser» eliminado.

## 5. VERIFY / cierre

- [ ] 5.1 `cargo test` y `openspec validate --all --strict` en verde.
- [ ] 5.2 Review `@rust-reviewer`; hallazgos aplicados.
- [ ] 5.3 Archivar el change.
