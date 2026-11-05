-- Widgets always on (widgets-always-on D1-D4).
--
-- Same idiom as `20261008000001_skill_router_tuning.sql`: idempotent, and it
-- never overwrites a value the user has customised. The router knobs are read
-- on every turn, so editing them takes effect without a restart.
--
-- The `widgets` skill is no longer conditioned by the classifier: its compiled
-- threshold is now 0, so it is selected on every turn and its fragment travels
-- on every turn, router up or down. This migration retires the two artefacts
-- the previous tuning left behind, both guarded so a user value is never lost.
--
-- Why each block:
--
--   1) Retire the seeded widget threshold. `20261008000001` seeded
--      `ROUTER_THRESHOLD_WIDGETS = 0.20` to make the widget discretionary; the
--      skill now compiles to 0, so that override would re-condition it. The key
--      is deleted ONLY when it still holds the seeded value -- either `0.2` or
--      `0.20`, the seeder's and the UI's spellings of the same number. Any other
--      value is a deliberate user decision (raising the threshold is how one
--      re-conditions the skill) and is kept.
--
--   2) Replace the widget guide with the new one. `20261008000001` seeded the
--      old guide into `SKILL_WIDGETS_PROMPT`; the new guide must replace it, but
--      only when the fragment is still byte-for-byte the seeded text: a value
--      the user edited is never touched. When the key is absent it is seeded. A
--      key that is present but blank is deliberately left alone: an empty or
--      blank fragment is user content (and the spec wants it to leave no trace),
--      so it is never overwritten; only an absent key is seeded.

-- 1) Retire the seeded widget threshold, only when still at our default.
DELETE FROM settings
WHERE key = 'ROUTER_THRESHOLD_WIDGETS'
  AND trim(value) IN ('0.2', '0.20');

-- 2a) Replace the widget guide, only when it is still the seeded text.
UPDATE settings
SET value = '# Instrucciones de Interfaz y Widgets Interactivos

Dispones de la herramienta ejecutable `render_widget` para poner un widget interactivo en el chat.

## Regla de oro

Un widget **solo** aparece si ejecutas `render_widget` en esa misma respuesta. NUNCA digas que has mostrado un widget, un formulario, una lista o un mapa si no has ejecutado la herramienta en ese turno: afirmarlo es inventar información.

## La regla de las opciones

Mira lo que estás a punto de escribir. Si van a aparecer **dos o más opciones etiquetadas** (`A`/`B`/`C`, `1`/`2`/`3`) o **una lista de alternativas** para que el usuario elija —en tabla, en viñetas, en prosa o como pregunta—, NO las escribas: renderiza un widget con ellas.

Lo mismo si vas a pedir más de un dato a la vez, proponer un plan con pasos, listar cosas que marcar o situar un lugar en un mapa.

## Los tres widgets

`widget_name` admite exactamente estos tres nombres.

**`QuickForm`** — elegir entre opciones o pedir datos. `fields[].type` es `text`, `textarea`, `number`, `select`, `checkbox` o `slider`; `options` solo con `select`; `min` y `max` solo con `slider`.

```json
{"widget_name":"QuickForm","data":{"title":"Elige una opción","fields":[{"name":"opcion","label":"Opción","type":"select","options":["Actualizar ahora","Esperar a la próxima versión"]}],"submit_label":"Enviar"}}
```

**`Checklist`** — una lista de cosas que marcar. Cada ítem lleva `id` y `label`.

```json
{"widget_name":"Checklist","data":{"title":"Pendientes por despachar","items":[{"id":"1","label":"Responder el correo pendiente"},{"id":"2","label":"Pagar la luz"}]}}
```

**`LocationWidget`** — un mapa con un punto. Necesita `latitude` y `longitude` **reales** (de las herramientas disponibles o del contexto ya conocido); no las inventes.

```json
{"widget_name":"LocationWidget","data":{"title":"Parking del centro","latitude":43.3674,"longitude":-8.4012,"address":"Calle Mayor 1"}}
```

Puedes acompañar el widget con una frase breve de encabezado. Lo que no puedes es repetir en prosa la lista que el widget ya muestra.

## Los dos únicos casos sin widget

La regla de las opciones tiene prioridad sobre todo lo demás. Solo respondes con texto cuando:

1. Es una explicación conceptual que no pide elegir nada ni lleva lista que marcar.
2. Es un dato único y directo: una fecha, un número, un nombre.

Ante la duda, mira lo que ibas a escribir: si hay dos opciones o una lista, no es ninguno de esos dos casos.

## Después de un widget

El usuario contesta con un mensaje como `[widget:Checklist#w3] submit {"checkedIds":["1"]}`. Procesa esa respuesta de inmediato y confirma el resultado en texto, sin volver a renderizar el mismo widget salvo que pida cambiarlo.',
    updated_at = datetime('now')
WHERE key = 'SKILL_WIDGETS_PROMPT'
  AND value = '# Instrucciones de Interfaz y Widgets Interactivos

Dispones de la herramienta ejecutable `render_widget`.

## Reglas de Invocación
1. **Acción estricta:** Un widget SOLO se muestra en pantalla si ejecutas activamente la llamada a la función `render_widget`. NUNCA menciones que has mostrado, renderizado o dibujado un widget si no has ejecutado dicha herramienta en la misma respuesta.
2. **Criterios de Activación (Triggers):** DEBES invocar `render_widget` de forma explícita cuando la interacción cumpla cualquiera de estas condiciones:
   - **Toma de decisiones:** El usuario deba elegir entre 2 o más opciones.
   - **Recolección de datos:** Requieras más de un dato puntual (usa un formulario dinámico en lugar de repreguntar por texto).
   - **Flujos paso a paso:** Presentes un plan, guía o lista de tareas ejecutable.
   - **Datos complejos / Geográficos:** Muestres direcciones, rutas, mapas, tablas o estadísticas.
3. **Restricción de texto simple:** Si la respuesta se resuelve con una explicación conceptual o un dato directo, NO invoques la herramienta. Responde únicamente con texto plano.
4. **Procesamiento de respuestas:** Cuando el usuario interactúe con el widget, recibirás un mensaje de entrada con los datos seleccionados. Procesa la respuesta de inmediato y confirma el resultado sin volver a renderizar el widget, a menos que se requiera una modificación explícita.';

-- 2b) Seed the guide only when the key is absent (a blank key is user content).
INSERT INTO settings (key, value, updated_at)
SELECT 'SKILL_WIDGETS_PROMPT', '# Instrucciones de Interfaz y Widgets Interactivos

Dispones de la herramienta ejecutable `render_widget` para poner un widget interactivo en el chat.

## Regla de oro

Un widget **solo** aparece si ejecutas `render_widget` en esa misma respuesta. NUNCA digas que has mostrado un widget, un formulario, una lista o un mapa si no has ejecutado la herramienta en ese turno: afirmarlo es inventar información.

## La regla de las opciones

Mira lo que estás a punto de escribir. Si van a aparecer **dos o más opciones etiquetadas** (`A`/`B`/`C`, `1`/`2`/`3`) o **una lista de alternativas** para que el usuario elija —en tabla, en viñetas, en prosa o como pregunta—, NO las escribas: renderiza un widget con ellas.

Lo mismo si vas a pedir más de un dato a la vez, proponer un plan con pasos, listar cosas que marcar o situar un lugar en un mapa.

## Los tres widgets

`widget_name` admite exactamente estos tres nombres.

**`QuickForm`** — elegir entre opciones o pedir datos. `fields[].type` es `text`, `textarea`, `number`, `select`, `checkbox` o `slider`; `options` solo con `select`; `min` y `max` solo con `slider`.

```json
{"widget_name":"QuickForm","data":{"title":"Elige una opción","fields":[{"name":"opcion","label":"Opción","type":"select","options":["Actualizar ahora","Esperar a la próxima versión"]}],"submit_label":"Enviar"}}
```

**`Checklist`** — una lista de cosas que marcar. Cada ítem lleva `id` y `label`.

```json
{"widget_name":"Checklist","data":{"title":"Pendientes por despachar","items":[{"id":"1","label":"Responder el correo pendiente"},{"id":"2","label":"Pagar la luz"}]}}
```

**`LocationWidget`** — un mapa con un punto. Necesita `latitude` y `longitude` **reales** (de las herramientas disponibles o del contexto ya conocido); no las inventes.

```json
{"widget_name":"LocationWidget","data":{"title":"Parking del centro","latitude":43.3674,"longitude":-8.4012,"address":"Calle Mayor 1"}}
```

Puedes acompañar el widget con una frase breve de encabezado. Lo que no puedes es repetir en prosa la lista que el widget ya muestra.

## Los dos únicos casos sin widget

La regla de las opciones tiene prioridad sobre todo lo demás. Solo respondes con texto cuando:

1. Es una explicación conceptual que no pide elegir nada ni lleva lista que marcar.
2. Es un dato único y directo: una fecha, un número, un nombre.

Ante la duda, mira lo que ibas a escribir: si hay dos opciones o una lista, no es ninguno de esos dos casos.

## Después de un widget

El usuario contesta con un mensaje como `[widget:Checklist#w3] submit {"checkedIds":["1"]}`. Procesa esa respuesta de inmediato y confirma el resultado en texto, sin volver a renderizar el mismo widget salvo que pida cambiarlo.', datetime('now')
WHERE NOT EXISTS (SELECT 1 FROM settings WHERE key = 'SKILL_WIDGETS_PROMPT');
