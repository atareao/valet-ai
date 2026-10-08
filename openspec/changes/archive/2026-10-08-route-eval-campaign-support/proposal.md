# Proposal

## Why

La campaña para decidir si se enciende el enrutador arranca hoy con dos carencias del arnés que la hacen **ciega y no reproducible**:

1. **El arnés descarta las probabilidades.** Llama a `select` y recibe el `Selection` completo, pero solo usa `selection.skills`; `selection.probabilities` se tira. Ante un turno no cubierto, el informe dice qué herramienta faltó y qué skill la habría cubierto, pero **no a qué distancia del umbral quedó esa skill**. Sin esa distancia no se puede saber si el fallo se arregla bajando un umbral (near-miss) o reescribiendo las `criteria` (miss semántico) — y con una varianza medida de ~4,6 pp, un resultado sin su puntuación no es atribuible.

2. **Las variantes no son reproducibles.** Las `criteria` y los umbrales solo se pueden variar mutando `settings` (el flag `--threshold` pisa el global, pero **no** los overrides por skill como `ROUTER_THRESHOLD_WIDGETS=0,20`). Cada experimento exige ensuciar la base de datos de desarrollo, no queda versionado y no se puede repetir tal cual.

El diagnóstico de partida sobre los 66 turnos da **95,5 % (63/66)** con tres turnos no cubiertos —`calendar`+`tasks`; `render_widget`; y un turno compuesto que pidió `calendar`, `reminders`, `render_widget` y `weather`— y **ninguna herramienta faltante ajena al catálogo**: la campaña de `criteria` es viable, pero necesita este instrumento para no ajustar a ciegas.

## What Changes

- **Diagnóstico de los turnos no cubiertos**: el arnés SHALL publicar, por cada herramienta usada que no se expuso, la skill que la habría cubierto, la **probabilidad** que el clasificador le asignó, el **umbral efectivo** con el que se comparó y la **fuente** de la selección; y SHALL agregar la **proximidad al umbral** (cuántos fallos quedaron a menos de una distancia dada), para distinguir near-miss de miss semántico.
- **Overrides desde fichero**: el arnés SHALL aceptar `--overrides <ruta>` con umbrales (global y por skill) y criterios por skill, de modo que una variante sea un **artefacto versionado y reproducible** que SHALL NOT requerir mutar `settings`. La precedencia SHALL ser CLI > fichero > `settings` (que cae al catálogo) y el informe SHALL declarar qué overrides estaban activos.
- **Sin cambios** en el comportamiento de producción: ni el enrutador (`ROUTER_ENABLED=false`), ni el catálogo, ni los umbrales, ni las herramientas, ni la API, ni la UI.

## Capabilities

### New Capabilities
- (ninguna)

### Modified Capabilities
- `orchestrator/skill-router`: el requisito del arnés añade el **diagnóstico con probabilidades, umbral efectivo y fuente** y los **overrides desde fichero**.

## Impact

- `src/bin/route-eval.rs` y sus tests.
- **No** toca: el catálogo, el enrutador, `orchestrator::agent`, la API, la UI, `settings` ni ninguna migración.
- **Criterio de aceptación de este change**: (a) una ejecución explica **cada** turno no cubierto con la probabilidad de la skill que lo habría cubierto y el umbral que no alcanzó; y (b) dos ficheros de overrides distintos producen dos mediciones distintas y reproducibles sin tocar la base de datos.
