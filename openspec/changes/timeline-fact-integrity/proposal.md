# Change: Timeline fact integrity — sin duplicados, con fecha real, lugar y hechos sustanciosos

## Why

El timeline se desplegó el 2026-10-10 y en dos días ha producido una tabla que no sirve para lo que
promete. Auditado sobre la base de datos de producción el 2026-10-11 (22 filas, 14 hechos distintos):

- **8 de las 22 filas (36 %) son duplicados exactos.** Siete salieron de una sola pasada, la de las
  13:55, cuyo mensaje de origen es la respuesta con la que el propio asistente **renderizó el timeline**
  a petición del usuario. El extractor leyó esa tabla, la tomó por conversación y volvió a extraer los
  hechos que ya estaban escritos: un bucle de realimentación que se repite cada vez que se pide ver el
  timeline.
- **Cinco de los catorce hechos no son hechos del usuario.** `Todavía no me fui a dormir` es un estado
  transitorio; `Dió por cumplido el recordatorio`, `Marcó como leídos los correos de tududi`,
  `Envió un correo a …` y `Borré los correos de phishing` son órdenes dadas al asistente o acciones
  suyas.
- **La fecha miente.** El `timestamp` se deriva del `created_at` del mensaje que menciona el hecho, no
  de cuándo ocurrió: `Fui a correr 9,19 km` quedó fechado a las 15:25 cuando la carrera fue a las 10:21,
  y `Corrí 9,34 km a 6:44` a las 17:23 del día anterior.
- **No hay lugar.** El esquema no tiene columna: `Pulpería Terra Meiga`, `vía verde` o `casa` se
  pierden.
- **Y ni siquiera los hechos distintos son acontecimientos distintos.** `Salí a correr 10 km`,
  `Fui a correr 10 km por la vía verde`, `Terminé el entrenamiento` y `Fui a correr 9,19 km por la vía
  verde` son la misma carrera del domingo: cuatro filas para una tirada. Dos días de vida real —correr,
  cenar, comer con los Montaña— caben en cuatro hechos, y hay veintidós filas.

La causa está en tres decisiones del diseño original: el extractor recibe **todos** los mensajes sin
indexar, sin filtrar por rol (`src/workers/episodic_memory.rs:542`); la única deduplicación es **dentro
de un mismo lote** (`src/workers/episodic_memory.rs:236-262`) y el esquema no tiene ninguna restricción
de unicidad; y el prompt prohíbe expresamente pedirle fechas al modelo, de modo que la fecha es siempre
la del mensaje que lo menciona.

## What Changes

- **La fuente es el usuario.** Un hecho SHALL citar (`source`) una línea de rol `user`; un hecho que
  cite una línea del asistente SHALL descartarse. Es una comprobación **en código**, no una instrucción
  al modelo: el bucle muere por construcción, porque el render del timeline es un mensaje del asistente.
  El asistente sigue en el bloque, pero como contexto: nunca como origen de un hecho.
- **Sin duplicados, en tres capas.** Se mantiene la deduplicación dentro del lote; el esquema gana un
  **índice único** sobre el hecho normalizado y el día (`lower(trim(fact))`, `substr(timestamp,1,10)`)
  y la inserción pasa a `INSERT OR IGNORE`; y el extractor recibe los **hechos ya registrados** de los
  últimos días con la orden de no repetirlos, que es lo único que caza los casi-duplicados
  (`Corrí 9,34 km` frente a `Corrí 9,34 km a 6:44`), invisibles para una comparación de texto.
- **La fecha la propone el modelo y se valida.** El extractor SHALL devolver un `timestamp` ISO 8601
  opcional. Si falta, no parsea, no lleva desfase horario, está en el futuro o es anterior a la ventana
  admitida, SHALL caer al `created_at` del mensaje citado —el comportamiento actual—. **Esto invierte un
  requisito vigente** (`workers`: «el extractor SHALL NOT pedirle fechas al modelo»).
- **Lugar.** Columna `location` (TEXT, admite `NULL`) en `timeline_events`; `location` opcional en la
  respuesta del extractor y en el argumento de `timeline_add_event`; y `location` en lo que devuelve
  `timeline_get_events`.
- **Hechos sustanciosos.** El prompt sembrado SHALL definir qué es un hecho —una acción o
  acontecimiento del pasado, completo y comprensible sin la conversación—, SHALL excluir expresamente
  las órdenes dirigidas al asistente, los estados transitorios, las opiniones, las preguntas y los
  planes futuros, y SHALL pedir el lugar y la fecha cuando el mensaje los permita. El código pone el
  tope duro: un `fact` de más de 280 caracteres se descarta por no ser atómico.
- **Un hecho es un acontecimiento, no sus fases.** El prompt sembrado SHALL decir que salir a correr,
  correr diez kilómetros y terminar el entrenamiento son **un solo hecho**, y SHALL ordenar no repetir
  ningún hecho ya registrado ni con las mismas palabras ni con otras. Es la parte que ningún índice puede
  resolver —una comparación de texto no une `Salí a correr 10 km` con `Terminé el entrenamiento`— y por
  eso vive en el prompt, con la lista de hechos registrados como herramienta.
- **Instante de referencia.** El bloque SHALL llevar el `created_at` del último mensaje del lote, para
  que el modelo pueda resolver fechas relativas («ayer», «esta mañana»).
- **Vaciado, no reparación.** La migración SHALL **vaciar** `timeline_events` en vez de intentar
  arreglarla. Los hechos guardados son derivados —los escribió el extractor, no el usuario—, sus fechas
  son las de la mención y no las del acontecimiento, ninguno tiene lugar y más de un tercio son
  duplicados: no hay nada que conservar, y el material crudo sigue en `messages`. Vaciarla elimina además
  el paso frágil de deduplicar antes de crear el índice único, que era el único trozo de SQL delicado del
  change.
- **El prompt se actualiza solo si nadie lo ha tocado.** La migración SHALL reemplazar
  `timeline_prompt` y `SKILL_TIMELINE_PROMPT` **solo** si su valor es todavía el sembrado por la
  migración `20261010000004`; una personalización se respeta (en producción el valor es byte a byte el
  sembrado, así que allí sí se actualizará).

## Impact

### Specs modificadas

- `db/schema`: el vaciado de la tabla, la columna `location`, el índice único por hecho y día y la
  actualización del prompt sembrado.
- `workers`: la Capa D —fuente restringida al rol `user`, fecha propuesta y validada, lugar, hechos
  sustanciosos, deduplicación entre pasadas y lista de hechos ya conocidos—, y el contrato del prompt
  sembrado (exclusiones, grano de acontecimiento, fecha y lugar).
- `tools/timeline`: `location` en la lectura y en el alta, y el alta que informa cuando el hecho ya
  existía.

### Código

- `migrations/` (una migración nueva), `src/db/repos/timeline.rs`, `src/models/timeline.rs`,
  `src/workers/episodic_memory.rs` y `src/tools/timeline.rs`.

### Datos

- `timeline_events` queda **vacía** y gana la columna `location`. No se pierde nada irrecuperable: los
  hechos eran derivados y el material crudo sigue en `messages`.

### No-objetivos

- **No se reconstruyen los hechos actuales**: se vacían y ahí queda. Recuperar esos dos días exigiría
  volver a pasar el extractor sobre `messages`, y no hay tal función.
- **No hay merge ni enriquecimiento posterior de un hecho**: un hecho se registra una vez y se borra si
  procede; no se actualiza con datos que aparezcan después.
- **No se leen herramientas para completar el hecho** (Strava, calendario): el contexto del lote son los
  mensajes de la conversación y nada más. Por eso el lugar y las métricas solo entran cuando el usuario
  los dice.
- **La sustancia no se comprueba con una heurística**: no hay definición mecánica de «hecho
  sustancioso». El contrato vive en el prompt sembrado y en el tope de 280 caracteres; el resto es
  criterio del modelo.
- No hay interfaz nueva, ni exportador, ni cambios en el consolidador o en la memoria episódica.
