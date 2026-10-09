# Tasks

## 1. RED — tests que fallan (grupo 1: fidelidad del historial)

- [x] 1.1 Test: un mensaje con `collapsed_content` viaja al clasificador con el **colapsado**, no con el crudo (la consulta del arnés selecciona el contenido efectivo).
- [x] 1.2 Test: la ventana la fija `max_window_tokens` contada hacia atrás desde el turno — con un presupuesto que excluye los mensajes antiguos, no entran; con presupuesto holgado, entran.
- [x] 1.3 Test: un corte de emparejamiento **no** vacía el historial si la ventana todavía alcanza a los mensajes anteriores.
- [x] 1.4 Test: el historial sigue llevando los **roles reales** (`user:` / `assistant:`) y el recorte a 400 caracteres.

## 2. RED — tests que fallan (grupo 2: instrumentación y repetición)

- [x] 2.1 Test: las herramientas por turno y los bytes/tokens estimados salen de las **definiciones expuestas** serializadas, y el referente es el conjunto habilitado completo (tabla con un caso construido a mano).
- [x] 2.2 Test: el ahorro es la proporción de bytes y de tokens estimados del bloque expuesto frente al completo, y es `0` (no un `NaN`) cuando el conjunto completo está vacío.
- [x] 2.3 Test: `--repeat 3` ejecuta tres barridos y el resumen publica mínimo, media y máximo; `--repeat 1` no añade el bloque de varianza.

## 3. GREEN — implementación

- [x] 3.1 `list_by_token_budget`-equivalente en el arnés: consulta con `CASE WHEN collapsed_content IS NOT NULL THEN collapsed_content ELSE content END` y el coste en tokens efectivo.
- [x] 3.2 Construcción del historial por turno: ventana hacia atrás con `max_window_tokens` (leído de `settings` con el mismo default 10000), sin reinicio por cortes, con el recorte de 400 caracteres.
- [x] 3.3 Instrumentación: media de herramientas por turno, bytes y tokens estimados expuestos y completos, y ahorro.
- [x] 3.4 `--repeat <N>` con el resumen de mínimo/media/máximo, y `--dry-run` declarando la ventana que habría usado.

## 4. REFACTOR

- [x] 4.1 `cargo fmt` y `cargo clippy --all-targets -- -D warnings` sin warnings.
- [x] 4.2 Eliminar del arnés lo que queda sin uso tras quitar la corrida contigua (helpers y tests asociados), sin dejar código muerto.

## 5. Verificación y re-medición de aceptación

- [x] 5.1 `just check-all` en verde.
- [x] 5.2 Re-medición real de los 66 turnos con el arnés alineado y `--repeat 3`: publicar cobertura por repetición y rango, herramientas por turno, bytes/tokens y ahorro.
- [x] 5.3 Contrastar con el 97,0 % del experimento y **documentar el hueco que quede** (D5): ni retocar valores a ojo ni presentar una cifra sin su rango.

## 6. Cierre

- [x] 6.1 Reviews con `@rust-reviewer` (solo lectura) y hallazgos aplicados.
  - La revisión **no encontró sesgos críticos** y confirmó la fidelidad de la ventana, del contenido efectivo y del instrumento de ahorro.
  - Sus **dos hallazgos mayores resultaron teóricos al medirlos contra la BD real**: 256 mensajes con 256 `created_at` distintos (**cero empates**, el desempate nunca se activa) y 66 asistentes con `tools_used` ↔ 66 mensajes de usuario distintos (**factor 1,00x**, sin sobreconteo). Ambos quedan documentados en el código con la medición.
  - Corregidos: `savings_pct` (bloque vacío = **100 %** de ahorro; se corrigió **mi propio test equivocado**, no una aserción retorcida), orden por `rowid`, presupuesto `0` alineado con producción, etiquetas acumuladas de `--repeat` y el test de forma del cable (antes tautológico).
  - Deuda menor asumida: la cobertura se mide sobre el bloque de herramientas, no sobre la petición completa (el encabezado lo dice).

- [ ] 6.2 Documentar la medición en el PR y la decisión razonada sobre encender (el enrutador sigue **apagado**).
- [ ] 6.3 PR a `development` y `openspec archive route-eval-fidelity`.

## Resultados de la verificación (2026-10-08)

- **RED**: 10 tests nuevos fallando por aserción; los 16 existentes intactos; lib 773 sin regresiones.
- **GREEN**: 26 passed en el bin; lib 773; clippy `-D warnings` limpio.
- **Aceptación 5.2/5.3** — 66 turnos, `--repeat 3`: **95,5 % · 98,5 % · 95,5 %** (mín. 95,5 · media **96,5** · máx. 98,5), frente a 92,4-95,5 % antes de alinear y al **97,0 %** del experimento, que cae **dentro** del rango. **El hueco era el contexto, no las criteria** (D5).
- **Palanca medida del cable**: **10,01 herramientas/turno** de 13; ahorro **25,4 % en bytes · 25,5 % en tokens estimados**. La fidelidad del instrumento se comprobó: `ToolDef` tiene un `Serialize` a medida que emite la forma exacta de OpenRouter y el proveedor hace `serde_json::to_value(tools)`, así que lo serializado es lo que viaja; además el bloque completo medido (8.972 B/turno) coincide con la medición de runtime previa.
- **Corrección pendiente**: `savings_pct(0, whole)` devuelve `0.0` por un guard que satisface mi test del RED, pero la semántica honesta de «bloque expuesto vacío» es 100 % de ahorro. Caso inalcanzable (el core siempre va expuesto); se corrige con la revisión.
- **El 28,1 % del diseño era optimista**: procedía de serializar cada herramienta por separado, no del formato que viaja.
- **Medición final, con el código congelado tras las correcciones** (66 turnos, `--repeat 3`): **95,5 % · 95,5 % · 93,9 %** (mín. 93,9 · media **94,9** · máx. 95,5); **9,37 herramientas/turno** de 13; ahorro **31,2 % en bytes · 31,4 % en tokens estimados**; latencia p50 404 ms / p95 728 ms; coste ~$0,00005/turno.
- **AVISO — la varianza es mayor que la estimada: entre las seis ejecuciones totales el rango va de 93,9 % a 98,5 % (~4,6 pp), y el ahorro entre 25 % y 31 %.** Las correcciones aplicadas no afectan al enrutado (cero empates, presupuesto 10000, etiquetas y `savings_pct` son presentación), así que la dispersión es de **Jev**. Refuerza la conclusión: **una sola muestra no decide nada** y el dintel de encendido debe ser un rango.
