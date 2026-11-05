# Tasks — binary-choice-widget

## 1. Planificación (SDD)

- [x] 1.1 Change aprobado por el usuario antes de escribir código o tests (`just check-spec`).
- [x] 1.2 `plans/PLAN-017.md` creado y fila añadida al índice de `AGENTS.md` §VI.

## 2. RED — tests que fallan (Rust, función pura)

- [ ] 2.1 Positivos: «¿Hacemos esto o no?», «¿Te parece bien?» y «¿Empezamos?» como última línea producen una presentación de elección binaria.
- [ ] 2.2 La presentación lleva la pregunta y las opciones `Sí` y `No`, y su `span` delimita exactamente la línea de la pregunta.
- [ ] 2.3 Negativos de precisión: pregunta abierta («¿Qué prefieres?», «¿Cuándo lo hacemos?») → sin forma.
- [ ] 2.4 Negativo: alternativas nombradas con `o` («¿Lo hago ahora o mañana?») → sin forma.
- [ ] 2.5 Negativo: pregunta de sí/no que **no** cierra el turno → sin forma.
- [ ] 2.6 Negativo: pregunta dentro de un bloque de código con vallas → sin forma.
- [ ] 2.7 Negativo: turno que ya proyectó opciones (lista cerrada por interrogativa) y además termina en pregunta de sí/no → una sola presentación, la de opciones.
- [ ] 2.8 Negativo: turno sin pregunta al final → sin forma (comportamiento actual intacto).
- [ ] 2.9 `cargo test --lib` en rojo solo por los tests nuevos.

## 3. GREEN — implementación mínima

- [ ] 3.1 La forma nueva en `Presentation` y su detección en `detect` (solo la última línea con contenido, fuera de vallas, sin interrogativo abierto, sin alternativas nombradas).
- [ ] 3.2 Regla de precedencia: no se añade si el turno produjo una presentación de opciones.
- [ ] 3.3 Síntesis en `widget_for`: `QuickForm` con `radio`, opciones `Sí`/`No`, la pregunta en `label` y **sin** `title`.
- [ ] 3.4 `cargo test --lib` en verde.

## 4. REFACTOR y auditoría

- [ ] 4.1 Limpieza y comentarios del porqué (la exclusión de alternativas nombradas y de interrogativos abiertos).
- [ ] 4.2 `cargo clippy --all-targets -- -D warnings` sin warnings y `cargo fmt --all -- --check` sin diffs.
- [ ] 4.3 Auditoría de `@rust-reviewer` sobre el diff: 0 bloqueantes, 0 mayores.

## 5. Verificación

- [ ] 5.1 `just check-all` → exit 0.
- [ ] 5.2 `openspec validate --all --strict` sin errores.
- [ ] 5.3 Comprobación de que el frontend no necesita cambios (contrato `QuickForm` sin tocar).

## 6. Release

- [ ] 6.1 PR a `development` con CI verde.
- [ ] 6.2 `release/vX.Y.Z` desde `development`, PR a `main` y tag.
- [ ] 6.3 Despliegue en `co1` y verificación de la versión en `/api/health`.
