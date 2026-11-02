# Change: El servidor proyecta los widgets, el modelo solo aporta contenido

## Why

El change `widgets-always-on` apostó por que el modelo **decidiera** renderizar un widget, y le puso
delante la herramienta y una guía prescriptiva para inclinar la decisión. Su batería de aceptación
(tarea 6.3) ha **fallado**, y la evidencia medida el 2-nov-2026 explica por qué. Toda la evidencia de
esta sección está verificada, no reinterpretada.

- **La guía está viva y no basta.** `settings.SKILL_WIDGETS_PROMPT` tiene 3.812 caracteres —la guía
  nueva, reescrita en prescriptivo— y `render_widget` figura en `exposed_tools` en el **100 %** de los
  turnos. Aun así, la batería de 3 prompts reales da **0/3 widgets**.
- **Experimento de aislamiento** (misma imagen `valet-ai:v0.25.0`, los mismos 3 prompts, contenedor
  aparte con una copia de la BD de producción):

  | Escenario | Resultado |
  |:---|:---|
  | Conversación vacía + guía vieja (2.704 car.) | **0/3** (tabla Markdown de 9 tareas) |
  | Conversación vacía + guía nueva perseverante (3.812 car.) | **1/3** — el único prompt cuya respuesta es genuinamente una lista renderizó un `Checklist` con las tareas **reales**; los otros dos no renderizaron widget **y era correcto** (un dato único y una explicación, los dos casos que la propia guía excluye) |
  | Conversación real de producción (995 mensajes; **21 de los últimos 30 mensajes de asistente son listas de opciones en prosa**) + guía nueva | **0/3**, los tres en **tablas Markdown** |

  Conclusión: **el modelo imita su propio precedente y una instrucción de prompt no le gana.** El
  modelo es `deepseek/deepseek-v4.1-flash`, la petición va con `tool_choice: auto`, y el prompt base
  (4.687 car.) *ordena* usar «listas y tablas», se autodeclara prioritario y trae **tres ejemplos, los
  tres del patrón «herramienta → prosa», y cero de widget**.
- **Encuesta de industria** (2-nov-2026). Ningún producto serio deja que el modelo decida «texto o
  interfaz»:
  - **ChatGPT Apps SDK / Vercel AI SDK «Generative UI» / mcp-ui**: la UI es **propiedad del resultado
    de la herramienta** («a widget is simply a React component»; «the tool call renders a React
    component»). Se llamó al tool ⇒ hay widget.
  - **MCP elicitation** (revisión de julio de 2026): el **servidor** manda un esquema y el **cliente**
    lo renderiza como formulario y devuelve valores tipados. Lo inicia el servidor: el modelo no tiene
    discreción.
  - **Slack Block Kit / Discord embeds / Telegram inline keyboards**: la app compone la interfaz; el
    modelo aporta texto como mucho.
  - Nadie resuelve esto con prompting. Valet es el único que pide al modelo *elegir presentar*. Y, a
    diferencia de los demás, Valet **sí controla el pipeline** que produce la respuesta: puede
    proyectar la presentación él mismo.

El giro: **el widget deja de ser algo que el modelo *decide renderizar* y pasa a ser algo que el
servidor *proyecta*.** El modelo aporta contenido; el servidor decide presentación. Es el principio
que ya siguen los cuatro ecosistemas de arriba.

## What Changes

1. **Módulo nuevo puro `src/orchestrator/widget_projection.rs`** (sin I/O, sin async), con tres
   funciones puras y testeables:
   - `detect(text: &str) -> Option<Presentation>` — detección determinista sobre el texto final del
     asistente. Reconoce `Options` (listas de opciones etiquetadas `A)`/`A.`/`A:`/`1)`/`1.`/`Opción A`,
     o alternativas en viñetas cerradas por una interrogativa del tipo «¿cuál…?»), `Checklist`
     (tablas Markdown cuya primera columna es un índice o una casilla, o listas de viñetas con forma
     de tarea) y `Location` (una latitud/longitud o una dirección). Todo lo demás es `None`.
   - `synthesize(presentation, tool_results) -> Vec<Widget>` — construye el payload del widget. **Orden
     de preferencia para los datos:** si el turno llamó a una herramienta que produjo la lista
     (`tasks::list_tasks`, `tasks::search_tasks`, `email_list_unread`, `rss_list_unread`,
     `notifications::list_unread`), los ítems salen del `data` de esa herramienta (precisión, sin
     parsear prosa); si no, salen del texto.
   - `strip(text, presentation) -> String` — retira del texto el bloque que el widget ya muestra,
     conservando la prosa que lo rodea.
2. **Integración en el turno** (`src/orchestrator/agent.rs`, al cierre y **antes** de persistir): el
   punto exacto ya existe —el turno ensambla `content_buffer`, recoge `rendered_widgets` y persiste
   con `MessagesRepo::set_widgets` justo antes de `publish_persisted_message`. Al cierre: `detect`
   sobre `content_buffer` → `synthesize` → fusionar con `rendered_widgets` (deduplicar por nombre de
   widget + payload) → `set_widgets` → emitir el/los evento(s) SSE `widget` **antes** de `done`.
3. **`render_widget` sigue funcionando y gana.** Si el modelo ya renderizó un widget de ese tipo, la
   proyección para ese tipo se omite (el payload del modelo es más rico). La herramienta queda como
   **vía explícita de override**.
4. **Fuente única de verdad: el widget, no la prosa.** El `content_buffer` que se persiste es el texto
   **sin** el bloque que el widget ya muestra, y el evento `done` lleva el contenido autoritativo para
   que el cliente reemplace lo que ya había streameado. **BREAKING** para el contrato del evento
   `done`: gana un campo `content`.
5. **`render_widget` y la guía se DEGRADAN, no se eliminan.** La herramienta se queda como vía
   explícita; la guía se recorta a un recordatorio breve, porque ya no carga el peso. Efecto lateral
   medido: la guía actual (3.812 car. ≈ 1.000 tokens) viaja en **todos** los turnos; recortarla ahorra
   ese coste por turno.
6. **Tests.** El proyector es puro ⇒ tests unitarios de **cada detección y, sobre todo, de cada NO
   detección** (los falsos positivos son el riesgo real: «widgets de más»), más un test end-to-end del
   camino del agente. Esto cierra el agujero que `PLAN-012` declaró explícitamente: «la guía no tiene
   test unitario».

## Impact

- **Specs afectadas**: `orchestrator/agent` (2 requisitos añadidos: la proyección al cierre del turno y el
  contenido íntegro con el tramo que cada widget declara), `orchestrator/skill-router` (el requisito de la guía
  pasa de mecanismo a recordatorio: `REMOVED` + `ADDED`), `db/schema` (1 requisito modificado: la referencia
  cruzada al encabezado nuevo del requisito de la guía), `stream-route` (1 requisito modificado: el evento `done`
  gana el campo `content`) y `frontend` (1 requisito añadido: el cliente compone el mensaje por tramos y coloca
  cada widget en su `span`). `tools/registry`, `message`, `messages-repo` y `db/schema` **no** cambian de
  contrato: el esquema de `render_widget`, la columna `messages.widgets` (JSON opaco con un campo opcional más) y
  su persistencia se quedan como están, **sin columna nueva, sin migración de datos y sin backfill**.
- **Código afectado**: `src/orchestrator/widget_projection.rs` (nuevo), `src/orchestrator/agent.rs`
  (el cierre del turno), `migrations/20261102000001_widget_projection.sql` (nuevo: recorta la guía a
  su recordatorio, solo si sigue prístina) y, en el frontend, `useSSE.ts` / `useMainChat.ts` (el
  `done` con contenido reemplaza el texto).
- **Fuera de alcance**: el esquema de `render_widget`, el registro de widgets del frontend (`WIDGET_REGISTRY`,
  `WidgetRenderer`, `QuickFormWidget`, `ChecklistWidget`, `LocationWidget`), el `data` de cada widget, la
  persistencia en `messages.widgets` y la forma general de los eventos SSE `widget` (que solo gana un campo
  opcional `span`). El **modelo de bloques** con `image`, `chart` y `citation` es el destino declarado en
  `design.md` → D9, pero va en un change posterior.
- **Riesgo principal — «widgets de más»**: el proyector decide sin el modelo, así que un falso positivo
  es peor que un falso negativo. Se acota con el **disparador en el texto** (solo se proyecta si el
  texto *presenta* una lista) y con la batería de tests de NO detección. El riesgo contrario (un
  falso negativo) es benigno: se queda la prosa de siempre.
- **Relación con `widgets-always-on`**: este change **supersede el papel de la guía**. Su código está
  mergeado y desplegado (v0.25.0) y sus spec deltas están verificados (exposición de la herramienta +
  inyección del fragmento); lo que falló es solo su batería de aceptación. Ver `design.md` → D4.
- **Coste por turno**: baja. La guía recortada pesa menos y el esquema de `render_widget` sigue
  viajando (la herramienta no se retira).
