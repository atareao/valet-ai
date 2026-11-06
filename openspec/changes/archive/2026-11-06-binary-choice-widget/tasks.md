# Tasks — binary-choice-widget

## 1. Planificación (SDD)

- [x] 1.1 Change aprobado por el usuario antes de escribir código o tests (`just check-spec`).
- [x] 1.2 `plans/PLAN-017.md` creado y fila añadida al índice de `AGENTS.md` §VI.

## 2. RED — tests que fallan (Rust, función pura)

- [x] 2.1 Positivos: «¿Hacemos esto o no?», «¿Te parece bien?» y «¿Empezamos?» como última línea producen una presentación de elección binaria.
- [x] 2.2 La presentación lleva la pregunta y las opciones `Sí` y `No`, y su `span` delimita exactamente la línea de la pregunta.
- [x] 2.3 Negativos de precisión: pregunta abierta («¿Qué prefieres?», «¿Cuándo lo hacemos?») → sin forma.
- [x] 2.4 Negativo: alternativas nombradas con `o` («¿Lo hago ahora o mañana?») → sin forma.
- [x] 2.5 Negativo: pregunta de sí/no que **no** cierra el turno → sin forma.
- [x] 2.6 Negativo: pregunta dentro de un bloque de código con vallas → sin forma.
- [x] 2.7 Negativo: turno que ya proyectó opciones (lista cerrada por interrogativa) y además termina en pregunta de sí/no → una sola presentación, la de opciones.
- [x] 2.8 Negativo: turno sin pregunta al final → sin forma (comportamiento actual intacto).
- [x] 2.9 `cargo test --lib` en rojo solo por los tests nuevos.

## 3. GREEN — implementación mínima

- [x] 3.1 La forma nueva en `Presentation` y su detección en `detect` (solo la última línea con contenido, fuera de vallas, sin interrogativo abierto, sin alternativas nombradas).
- [x] 3.2 Regla de precedencia: no se añade si el turno produjo una presentación de opciones.
- [x] 3.3 Síntesis en `widget_for`: `QuickForm` con `radio`, opciones `Sí`/`No`, la pregunta en `label` y **sin** `title`.
- [x] 3.4 `cargo test --lib` en verde.

## 4. REFACTOR y auditoría

- [x] 4.1 Limpieza y comentarios del porqué (la exclusión de alternativas nombradas y de interrogativos abiertos).
- [x] 4.2 `cargo clippy --all-targets -- -D warnings` sin warnings y `cargo fmt --all -- --check` sin diffs.
- [x] 4.3 Auditoría de `@rust-reviewer` sobre el diff: 0 bloqueantes, 0 mayores.

## 5. Verificación

- [x] 5.1 `just check-all` → exit 0.
- [x] 5.2 `openspec validate --all --strict` sin errores.
- [x] 5.3 Comprobación de que el frontend no necesita cambios (contrato `QuickForm` sin tocar).

## 6. Release

- [x] 6.1 PR a `development` con CI verde. (PR #312)
- [x] 6.2 `release/vX.Y.Z` desde `development`, PR a `main` y tag. (PR #313, tag v0.28.0)
- [x] 6.3 Despliegue en `co1` y verificación de la versión en `/api/health`. (desplegado en co1, /api/health → version 0.28.0, verificado 2026-11-06)

## 7. Correcciones tras la auditoría de @rust-reviewer

- [x] 7.1 (M1) Interrogativos plurales/femeninos y fusionados (`cuáles`, `quiénes`, `cuántos`, `cuántas`, `cuánta`, `adónde`) en `OPEN_INTERROGATIVES`; sin añadir las formas sin acento.
- [x] 7.2 (M2) La elección binaria no se proyecta si su `span` solapa con otra presentación (`Location`); invariante «spans never overlap».
- [x] 7.3 (M3) `strip_markdown_emphasis` antes del gate `ends_with('?')`, de modo que la etiqueta no lleva marcadores.
- [x] 7.4 (M4) `ó` y `u` cuentan como conjunción (`¿Son 7 ó 8?`, `¿Son 7 u 8?`).
- [x] 7.5 (M5) Helper `quick_form` compartido por las ramas `Options` y `BinaryChoice`.
- [x] 7.6 (M6) Helper `last_content_line_outside_fences`, un solo recorrido (sin el re-escaneo de `binary_choice_block`).
- [x] 7.7 (M7) Cobertura: plurales/femeninos, `ó`/`u`, `o sí`, «¿Por correo o por nota?», pregunta enfatizada sin `**`, y aserción explícita de que el procedimiento no produce `Options`.
- [x] 7.8 (M8) Una sola construcción con `presentations.extend(...)` en vez del `if`/`if let` anidado.
- [x] 7.9 (M9/M10/M11/M12/M13) Matices de documentación (`is_interrogative`, `proposal.md`, `design.md`, acentos de `closed_yes_no_question`) y recorte del prefijo de cita/viñeta en la etiqueta.
- [x] 7.10 `cargo test --lib`, `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings` y `openspec validate` en verde.
- [x] 7.11 (N1–N4) Cierre de la reauditoría: comentario de `closed_yes_no_question` alineado con el array; `?`/`¿?` sueltos descartados (rama muerta eliminada); fijado por test y documentado en `design.md` §D1 que la pregunta puede preceder a una valla de cierre; y bordes de espacios finales cubiertos (con `?»`/`?)` declarado como limitación conocida).
