# Tasks — prose-choice-widget

## 1. Planificación (SDD)

- [x] 1.1 Change aprobado por el usuario antes de escribir código o tests (`just check-spec`).
- [x] 1.2 `plans/PLAN-018.md` creado y fila añadida al índice de `AGENTS.md` §VI. (creado)

## 2. RED — tests que fallan (Rust, función pura)

- [x] 2.1 **Corpus real primero**: positivos literales de producción (`…o va directo a algo concreto? ☕️`, `…o lo dejo así? ☕️`, `…o lo dejamos para mañana con luz? ☕️`, `¿Cerramos por hoy, Lorenzo?`, `¿Le saco el detalle?`, `### 📋 ¿Seguimos?`, `…o se lo deje más sobrio? Confirme y sale disparado. 🎩`) producen `ClosingChoice`.
- [x] 2.2 **Corpus real primero**: negativos literales de producción (`¿Qué necesita? 😒`, `¿Cuál, Lorenzo?`, `¿Prefieres A, B o C?`, `¿Por dónde empezamos, Lorenzo?`, `Dígame. ☕️`, `Todo cerrado, Lorenzo. **Ahora sí, a la calle** — … 😒☕️`) no producen `ClosingChoice`.
- [x] 2.3 F1: el recorte final deja la línea en `?` y el `span` sigue delimitando la línea original con emoji.
- [x] 2.4 F2: «¿X o Y?» produce dos opciones con los tramos limpios (recorte, guiones/rayas y comas, `;` y `:` finales, `¿`/`?`, capitalización).
- [x] 2.5 F2 binario: «¿Hacemos esto o no?» produce `Sí`/`No`; «o sí»/«o si» también.
- [x] 2.6 F2 guardas: más de una conjunción, tramo vacío o con <3 alfanuméricos, tramo >160 caracteres → sin forma.
- [x] 2.7 F3: pregunta no última del párrafo final (misma línea con prosa detrás, encabezado `###`) → sí; pregunta en párrafo anterior separada por línea en blanco → no.
- [x] 2.8 Negativos heredados: pregunta abierta, pregunta dentro de bloque de código, turno que ya proyectó opciones, procedimiento numerado.
- [x] 2.9 `cargo test --lib` en rojo solo por los tests nuevos.
- [x] 2.10 Medición contra 200 turnos reales de producción (arnés desechable, borrado): la elección de cierre pasó de **0** a **41 turnos (20,5 %)** — 9 binarios y 32 de dos caminos. Cero falsos positivos graves revisados a ojo. El resto de la cobertura (41 %) son las tablas de opciones que ya existían.

## 3. GREEN — implementación mínima

- [x] 3.1 Normalización de la línea en el orden `strip_line_prefix` → `strip_markdown_emphasis` → recorte final, antes del gate `ends_with('?')`.
- [x] 3.2 Búsqueda de la última línea con `?` del párrafo final, sin cruzar línea en blanco y fuera de vallas.
- [x] 3.3 Detección de dos caminos por la última conjunción `o`/`ó`/`u` con las guardas de F2; `o no`/`o sí`/`o si` se mantienen binarias.
- [x] 3.4 `Presentation::ClosingChoice { question, options, span }` sustituye a `BinaryChoice`; síntesis `QuickForm` con `radio`, pregunta en `label` y sin `title`.
- [x] 3.5 Precedencia intacta: no se añade si el turno produjo opciones; el `QuickForm` del modelo gana; `span` sin solapes.
- [x] 3.6 `cargo test --lib` en verde.

## 4. REFACTOR

- [x] 4.1 Limpieza y comentarios del porqué (recorte final, guardas de dos caminos, frontera de párrafo).
- [x] 4.2 `cargo fmt --all -- --check` sin diffs y `cargo clippy --all-targets -- -D warnings` sin warnings.
- [x] 4.3 Sin `unwrap()`/`expect()`/`panic!()` nuevos en producción.

## 5. Verificación

- [x] 5.1 `cargo fmt --check` → exit 0.
- [x] 5.2 `cargo clippy --all-targets -- -D warnings` → exit 0.
- [x] 5.3 `cargo test --lib` → verde.
- [x] 5.4 `just check-all` → exit 0. (exit 0; 1601 tests en lib)
- [x] 5.5 `openspec validate --all --strict` sin errores. (63 passed, 0 failed)
- [x] 5.6 Comprobación de que el frontend no necesita cambios (contrato `QuickForm` sin tocar).

## 6. Auditoría

- [x] 6.1 Auditoría de `@rust-reviewer` sobre el diff: 0 bloqueantes, 0 mayores. (@rust-reviewer: 0 bloqueantes tras una segunda pasada; en la primera detectó una regresión por línea en blanco final y un falso positivo con URL de query string, ambos corregidos)
- [x] 6.2 Correcciones de la auditoría aplicadas y re-verificadas.

## 7. Release

- [ ] 7.1 PR a `development` con CI verde.
- [ ] 7.2 `release/vX.Y.Z` desde `development`, PR a `main` y tag.
- [ ] 7.3 Despliegue en `co1` y verificación de la versión en `/api/health`.
- [ ] 7.4 Medición post-despliegue: contar widgets de elección en turnos reales y confirmar que ya no son 0.
