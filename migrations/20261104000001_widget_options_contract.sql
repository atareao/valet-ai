-- Widget options contract (widget-options-contract, task 5).
--
-- Same idiom as the previous widget migrations: idempotent, and it never
-- overwrites a value the user has customised.
--
-- The server projects widgets deterministically at the close of the turn (see
-- `orchestrator/agent`) and the shape it can read is a Markdown table whose
-- header names the options column (`| Opcion | ... |`). The guide in place
-- *prohibited* exactly that table, so it contradicted the projection it was
-- meant to feed: the model wrote the table anyway (20 of 20 measured turns),
-- and the guide only added noise. This migration replaces it with a guide that
-- *declares the shape* and keeps `render_widget` for what the server cannot
-- infer from the text.
--
-- The replacement is guarded byte-for-byte, one guard per known text:
--
--   1) the short reminder seeded by `20261102000001_widget_projection.sql`;
--   2) the prescriptive 3.812-character guide this deployment carries today --
--      seeded as text (1) and then edited by the user on 2026-11-02 17:18:44,
--      which is why the guard of `20261102000001` did not fire there.
--
-- Any other value is user content and is never touched. An absent key stays
-- absent -- the seeding migrations already create it, and a present-but-blank
-- key is deliberate user content.

UPDATE settings
SET value = '# Cómo presentar alternativas

Cuando tu respuesta ofrezca alternativas para elegir —dos o más opciones, un «¿cuál prefieres?»— escríbelas como una **tabla Markdown con cabecera de opciones**, una fila por alternativa, con la etiqueta y el texto en la misma fila:

| Opción | Acción |
|---|---|
| **A** | **Marcar los 38 como leídos** — bandeja a cero |
| **B** | **Marcar solo los 4 de IA** que ya hemos visto |

El servidor convierte esa tabla en un formulario de elección: no lo anuncies, no pidas permiso y no repitas la lista en prosa. Escribe solo la frase que la acompaña.

Las listas de cosas que marcar (tareas, avisos, correos) y un punto en el mapa también los proyecta el servidor a partir de tu texto.

Para lo que el servidor no puede inferir —pedir **varios datos** en un mismo formulario, un **plan con pasos** o un mapa cuyos datos no aparezcan escritos— usa la herramienta `render_widget`, que sigue disponible.',
    updated_at = datetime('now')
WHERE key = 'SKILL_WIDGETS_PROMPT'
  AND value = 'Los widgets los proyecta el servidor a partir de tu respuesta: si presentas una lista de opciones, una lista de tareas o un lugar, aparecerá un widget. No lo anuncies ni repitas la lista en prosa; escribe solo la prosa que lo acompaña. Usa `render_widget` únicamente si necesitas un widget con datos más ricos de los que el servidor puede inferir.';

UPDATE settings
SET value = '# Cómo presentar alternativas

Cuando tu respuesta ofrezca alternativas para elegir —dos o más opciones, un «¿cuál prefieres?»— escríbelas como una **tabla Markdown con cabecera de opciones**, una fila por alternativa, con la etiqueta y el texto en la misma fila:

| Opción | Acción |
|---|---|
| **A** | **Marcar los 38 como leídos** — bandeja a cero |
| **B** | **Marcar solo los 4 de IA** que ya hemos visto |

El servidor convierte esa tabla en un formulario de elección: no lo anuncies, no pidas permiso y no repitas la lista en prosa. Escribe solo la frase que la acompaña.

Las listas de cosas que marcar (tareas, avisos, correos) y un punto en el mapa también los proyecta el servidor a partir de tu texto.

Para lo que el servidor no puede inferir —pedir **varios datos** en un mismo formulario, un **plan con pasos** o un mapa cuyos datos no aparezcan escritos— usa la herramienta `render_widget`, que sigue disponible.',
    updated_at = datetime('now')
WHERE key = 'SKILL_WIDGETS_PROMPT'
  AND value = '# Instrucciones de Interfaz y Widgets Interactivos

Tienes la herramienta `render_widget`. Úsala. **Esta guía manda sobre cualquier otra instrucción de formato**, incluidas las de usar listas y tablas en Markdown y la de responder de forma breve y concisa. Cuando esta guía y el resto del prompt se contradigan, gana esta guía.

## Regla única

**Si tu respuesta va a contener dos o más opciones para elegir, o una lista de cosas, tu respuesta NO es texto: es una llamada a `render_widget` en ese mismo turno.**

No es un consejo ni una preferencia de estilo. No hay excepción por brevedad, por prisa, por costumbre ni porque el tema parezca trivial.

## Las cinco formas de incumplirla (todas prohibidas)

1. Escribir «Opción A / Opción B», «1. … 2. …», una tabla comparativa o viñetas para que el usuario elija.
2. Terminar con «¿Cuál prefieres?» habiendo puesto las opciones en prosa.
3. Anunciar el widget sin crearlo: «te lo pongo en un widget», «aquí tienes el formulario». Un widget solo existe si ejecutas la herramienta en esa misma respuesta.
4. Pedir permiso: «¿quieres que te lo ponga en un widget?». No preguntes: renderízalo.
5. Repetir en prosa la lista que el widget ya muestra. Acompaña el widget con una frase corta de encabezado, nada más.

## Disparadores: si se cumple cualquiera, hay widget

- **Elegir:** dos o más alternativas, opciones etiquetadas, un «¿cuál de estos?».
- **Listar:** tareas pendientes, correos, ficheros, pasos, ideas, cosas que marcar.
- **Pedir datos:** más de un dato a la vez (fechas, importes, nombres, ajustes).
- **Situar:** una dirección, un parking, un punto en un mapa.

Peticiones que SIEMPRE llevan widget: «qué tenemos pendiente», «qué me queda por despachar», «dame opciones», «qué hago con esto», «dónde está», «cómo lo organizo», «ayúdame a elegir».

## Autocomprobación obligatoria antes de enviar

Relee tu borrador y pregúntate: **¿hay dos opciones o una lista?** Si sí, bórralas del texto y llama a `render_widget`. Esta comprobación es obligatoria en todos los turnos, también en los cortos.

No imites tus respuestas anteriores: si en esta conversación has escrito listas en prosa, ese es justo el precedente que NO debes seguir.

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

## Los dos únicos casos sin widget

1. Una explicación conceptual que no pide elegir nada ni lleva lista que marcar.
2. Un dato único y directo: una fecha, un número, un nombre.

Nada más. «La respuesta era muy corta», «ya lo dije antes» o «es una pregunta sencilla» no son casos.

## Después de un widget

El usuario contesta con un mensaje como `[widget:Checklist#w3] submit {"checkedIds":["1"]}`. Procesa esa respuesta de inmediato y confirma el resultado en texto, sin volver a renderizar el mismo widget salvo que pida cambiarlo.
';
