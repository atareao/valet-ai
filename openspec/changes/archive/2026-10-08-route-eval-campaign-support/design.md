# Design

## D1 — Por qué publicar la probabilidad, y no solo «qué faltó»

El informe actual ya señala la skill que habría cubierto una herramienta faltante, pero no su puntuación. Esa puntuación es **la que decide la táctica**:

- Si la skill quedó a un pelo del umbral (**near-miss**), el arreglo es bajar el umbral — barato y sin tocar texto.
- Si puntuó bajo de verdad (**miss semántico**), hay que reescribir las `criteria` — trabajo editorial.

Publicar además la **proximidad al umbral** en bandas (p. ej. a menos de 0,02 / 0,05 / 0,10) convierte «3 fallos» en «2 near-miss y 1 miss», que es la información con la que se decide. Se publica la probabilidad que el propio router compara contra el umbral (`Selection.probabilities`), sin recalcular nada: el diagnóstico no puede ser una segunda verdad distinta de la que gobierna la selección.

## D2 — Por qué un fichero de overrides y no la base de datos

El flag `--threshold` de hoy solo pisa el umbral **global**; los overrides por skill viven en `settings` y solo se cambian mutando la BD. Eso hace que cada variante de la campaña: (a) ensucie la base de desarrollo, (b) no quede versionada, y (c) no sea reproducible.

Un fichero `--overrides` resuelve las tres: la variante es un artefacto que se guarda, se revisa y se re-ejecuta tal cual. El formato SHALL ser **JSON** (no se añade dependencia: `serde_json` ya está en uso), con umbrales (global y por skill) y criterios por skill —las mismas tres claves que ya entiende el router: pregunta, criterio de «sí» y criterio de «no»—.

## D3 — Precedencia de overrides

`CLI > fichero > settings (que cae al catálogo)`. Los flags `--threshold` y `--model` existentes se conservan y mandan sobre el fichero, para no romper la interfaz ya usada. El informe declara **qué** overrides estaban activos y de **dónde** vino cada uno, para que cada cifra siga siendo atribuible (mismo principio que la «configuración efectiva» del requisito vigente).

Un fichero indicado y **ausente o ilegible** SHALL fallar de forma ruidosa, no medir en silencio la configuración equivocada.

## D4 — Qué NO cambia

- El comportamiento de producción: catálogo, umbrales, criterios, `agent.rs`, API, UI. El enrutador sigue apagado.
- La semántica de la cobertura (un turno sin `tools_used` parseable nunca cuenta como cubierto) y la fórmula `cubiertos / total`.
- `--repeat`: sigue midiendo la **varianza de la misma** configuración, no barriendo variantes.
- La fidelidad ya ganada (contenido efectivo, ventana por `max_window_tokens`, instrumento de ahorro sobre las definiciones del cable).

## D5 — Qué habilita este change (y qué no)

Este change entrega **el instrumento**, no la decisión. La campaña que lo usa —clasificar los tres fallos, proponer variantes (umbral de `widgets`, criterios para peticiones compuestas), medirlas con `--repeat` y decidir el encendido— queda **fuera de alcance** y se apoyará en este arnés. Encender el enrutador sigue siendo una decisión aparte.
