# Tasks: Reagrupar el prompt en un único mensaje de sistema

## Bloque 1 — Caracterización de lo actual
- [x] 1.1 Test de caracterización: la petición abre con 3 mensajes `role:"system"` (prompt, episódica, fecha) y son mensajes separados. Documenta el estado actual.
- [x] 1.2 `cargo test` en verde; anotar el baseline. (baseline: 660 passed, 0 failed)

## Bloque 2 — Un único mensaje de sistema (prompt + episódica)
- [x] 2.1 RED: test que exige exactamente un mensaje `role:"system"` antes del historial, con el prompt y, si hay fichas, la sección episódica.
- [x] 2.2 GREEN: extraer el constructor del mensaje de sistema y hacer que la ruta lo use.
- [x] 2.3 Verificar que la sección episódica se omite por completo sin fichas (sin etiquetas ni relleno).

## Bloque 3 — Fecha, hora y ubicación al final
- [x] 3.1 RED: test que exige la sección de fecha como última del mensaje de sistema.
- [x] 3.2 GREEN: mover el bloque de fecha al constructor compartido.
- [x] 3.3 Verificar la omisión total de la sección cuando no hay contexto de navegador.

## Bloque 4 — Hueco reservado de memoria persistente (sin texto)
- [x] 4.1 RED: test que exige que la petición NO contenga ninguna sección de memoria persistente, ni título, marcador, comentario ni línea suelta.
- [x] 4.2 GREEN: añadir el punto de inserción (parámetro opcional, hoy ausente) sin emitir texto.
- [x] 4.3 Test del orden completo: prompt → (hueco) → episódica → fecha.

## Bloque 5 — Verificación y limpieza
- [x] 5.1 `cargo fmt --check` limpio.
- [x] 5.2 `cargo clippy --all-targets -- -D warnings` limpio.
- [x] 5.3 `cargo test` completo en verde, sin regresiones sobre el baseline. (668 passed, 0 failed; baseline 660)
- [x] 5.4 `openspec validate --strict` válido.
- [x] 5.5 Cerrar la nota del plan si el código revela algo no previsto. (Nota: la tarea 2.2 decía «sustituye los tres push por uno solo»; se implementó unificando primero prompt+episódica en el compositor y dejando el contexto de navegador como mensaje aparte hasta el bloque 3, para que el RED de 3.1 fuese real. El contrato final es un único mensaje con las tres secciones.)
