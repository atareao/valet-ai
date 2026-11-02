-- Widget projection (widget-projection, task 7).
--
-- Same idiom as the previous widget migrations: idempotent, and it never
-- overwrites a value the user has customised.
--
-- The widget guide no longer carries the presentation decision. The server now
-- projects widgets deterministically when the turn closes (see
-- `orchestrator/agent`), so the long prescriptive guide seeded by
-- `20261101000001_widgets_always_on.sql` is retired in favour of a short
-- reminder that just points at `render_widget` as the explicit override.
--
-- The replacement is guarded byte-for-byte: it only fires when
-- `SKILL_WIDGETS_PROMPT` still holds the exact text seeded by
-- `20261101000001`. A value the user edited is user content and is never
-- touched. An absent key is left absent -- `20261101000001` already seeds it,
-- and a present-but-blank key is deliberate user content.

UPDATE settings
SET value = 'Los widgets los proyecta el servidor a partir de tu respuesta: si presentas una lista de opciones, una lista de tareas o un lugar, aparecerá un widget. No lo anuncies ni repitas la lista en prosa; escribe solo la prosa que lo acompaña. Usa `render_widget` únicamente si necesitas un widget con datos más ricos de los que el servidor puede inferir.',
    updated_at = datetime('now')
WHERE key = 'SKILL_WIDGETS_PROMPT'
  AND value = '# Instrucciones de Interfaz y Widgets Interactivos

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

El usuario contesta con un mensaje como `[widget:Checklist#w3] submit {"checkedIds":["1"]}`. Procesa esa respuesta de inmediato y confirma el resultado en texto, sin volver a renderizar el mismo widget salvo que pida cambiarlo.';
