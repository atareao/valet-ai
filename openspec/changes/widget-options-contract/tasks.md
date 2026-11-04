# Tasks — widget-options-contract

## 1. Planificación (SDD)

- [x] 1.1 Crear el change `openspec/changes/widget-options-contract/`.
- [x] 1.2 Escribir `proposal.md` con la evidencia medida del 2026-11-04 (20 de 23 formularios ilegibles, 2 falsos positivos, 0 llamadas a `render_widget` en 39 turnos).
- [x] 1.3 Escribir `design.md` (D1–D7) con el texto exacto de la guía y las guardas de la migración.
- [x] 1.4 Escribir los cuatro deltas (`orchestrator/agent`, `orchestrator/skill-router`, `db/schema`, `frontend`).
- [x] 1.5 Validar con `openspec validate widget-options-contract --strict`.
- [x] 1.6 **Aprobación explícita del usuario** antes de escribir tests o código.

## 2. RED — Rust, tests que fallan

- [x] 2.1 Test con la tabla **literal** de producción (`| Opción | Acción |` con `**A**`, `**B**`, `**C**`, `**D**` y sus descripciones): las opciones son el texto completo de cada fila, con la etiqueta incluida. (implementado como `test_3_1_tabla_real_opcion_es_la_fila_completa`)
- [x] 2.2 Test: la tabla con tres columnas (`| Opción | Sesión | Hora |`) da una opción con las tres celdas. (implementado como `test_3_2_tabla_de_opciones_de_tres_columnas_une_las_tres_celdas`)
- [x] 2.3 Test: la tabla `| Lap | Dist. | Ritmo | Pulso |` (1..8) NO produce ninguna presentación. (implementado como `test_3_3_tabla_de_vueltas_no_es_tabla_de_opciones`)
- [x] 2.4 Test: la tabla `| Split | Ritmo | Desnivel |` NO produce ninguna presentación. (implementado como `test_3_4_tabla_de_splits_no_es_tabla_de_opciones`)
- [x] 2.5 Test: regresión — `| Opción | Descripción |` con texto en la columna de opciones sigue proyectando, con la fila completa. (implementado como `test_3_5_cabecera_de_opciones_con_texto_en_la_columna`)
- [x] 2.6 Test: una fila sin ninguna celda con texto no aporta opción. (implementado como `test_3_6_fila_sin_texto_no_aporta_opcion`)
- [x] 2.7 Test: regresión — las listas de opciones en prosa y las cerradas por interrogativa siguen proyectando igual.
- [x] 2.8 Actualizar los tests existentes que fijan el texto de las opciones de tabla (2.5, 2.6, 2.10, 2.12 y los de `synthesize` afectados).
- [x] 2.9 Ejecutar `cargo test` y confirmar rojo en los nuevos y verde en los heredados.

## 3. GREEN — Rust

- [x] 3.1 `options_table_block`: la opción es la fila completa; la rama heurística exige letras; ninguna tabla con columna numérica produce opciones.
- [x] 3.2 `widget_for`: el widget de opciones emite `type: "radio"`.
- [x] 3.3 `cargo test` en verde y `cargo check` sin errores.

## 4. RED/GREEN — frontend

- [x] 4.1 Test de `QuickFormWidget`: un campo `radio` muestra las alternativas a la vista y envía el texto elegido.
- [x] 4.2 `types.ts` y `QuickFormWidget.tsx`: nuevo tipo `radio` (antd `Radio.Group`).
- [x] 4.3 `npx vitest run` y `npx tsc --noEmit` en verde.

## 5. Migración de la guía

- [x] 5.1 Extraer byte a byte el valor actual de `SKILL_WIDGETS_PROMPT` de la base de datos de producción (copia con `sudo python3` + `backup`; nunca `immutable=1`) a un fichero temporal.
- [x] 5.2 Escribir `migrations/20261104000001_widget_options_contract.sql` con una guarda byte a byte por cada texto conocido (el recordatorio de `20261102000001` y el imperativo de producción) y el texto nuevo de D4.
- [x] 5.3 Test de migración en `tests/db/migrations.rs`: sustituye el texto reconocido, no toca un valor distinto y es idempotente.
- [x] 5.4 Verificar la coincidencia carácter a carácter de la guarda contra el valor extraído en 5.1.

## 6. REFACTOR y auditoría

- [x] 6.1 `cargo fmt --check` y `cargo clippy --all-targets -- -D warnings` limpios.
- [x] 6.2 `just check-all` en exit 0 y `openspec validate --all --strict` sin errores.
- [x] 6.3 Auditoría con `@rust-reviewer` sobre el diff completo, comprobando que los tests nuevos fijan el **texto** de las opciones y no solo su longitud.
- [x] 6.4 Auditoría con @react-reviewer del diff del frontend (accesibilidad del campo `radio`).
- [x] 6.5 Correcciones de la auditoría: la etiqueta del campo se asocia al grupo (`role="radiogroup"` + `aria-labelledby`), `name` único por grupo, mensajes de aserción y nombres de test obsoletos corregidos, y frase del delta sincronizada con el código.

## 7. Release y verificación en producción

- [ ] 7.1 Hotfix `v0.27.1` (patch) desde `main`, con imagen a GHCR.
- [ ] 7.2 Redesplegar el stack en `co1` por Dockge.
- [ ] 7.3 Repetir los turnos que fallaron («noticias?», «que mas tenemos?») y comprobar en la BD: opciones con el texto de su fila, `span` sobre la tabla, ningún widget sobre tablas de vueltas o splits.
- [ ] 7.4 `openspec archive widget-options-contract` con la verificación aprobada.

## 8. Fase 2 — medición del disparador del modelo (en paralelo, no bloquea el release)

- [ ] 8.1 Medidor desechable fuera del repo (`/tmp/opencode/valet-widget-eval`).
- [ ] 8.2 Medir `guia-imperativa`, `contrato-tabla`, `herramientas-planas` y `directiva-en-banda` sobre turnos reales.
- [ ] 8.3 Documentar el resultado en `plans/PLAN-015.md` y decidir si el change siguiente devuelve la autoría al modelo (híbrido).
