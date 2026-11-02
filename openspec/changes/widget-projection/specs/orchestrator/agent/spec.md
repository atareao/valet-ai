## ADDED Requirements

### Requirement: El orquestador SHALL proyectar widgets deterministas al cierre del turno

Al cerrar el turno, y **antes** de persistir el mensaje del asistente, el orquestador SHALL ejecutar una
proyección determinista sobre el texto final del asistente (`content_buffer`): detectar una forma de
presentación **y el tramo que ocupa**, sintetizar el widget correspondiente con ese tramo, fusionarlo
con los widgets ya renderizados por el modelo (`render_widget`) deduplicando por **nombre de widget y
payload**, persistir el resultado con
`MessagesRepo::set_widgets` y emitir el/los evento(s) SSE `widget` **antes** del evento `done` del
turno.

La proyección SHALL NOT depender de que el modelo invoque `render_widget`: es el **servidor** quien
decide la presentación a partir de lo que el modelo está a punto de presentar. La proyección SHALL ser
una función **pura** —sin I/O ni async—, de modo que cada detección y, sobre todo, cada **no** detección
sea verificable con tests unitarios.

El **disparador SHALL ser el texto**: si el texto final no presenta una forma reconocible, no SHALL
proyectarse ningún widget y `widgets` SHALL quedar como `null`, sin cambio alguno respecto al
comportamiento previo. Las formas reconocibles SHALL ser, al menos:

- **opciones**: una lista de opciones etiquetadas (`A)`, `A.`, `A:`, `1)`, `1.`, `Opción A`) o una lista
  de alternativas en viñetas cerrada por una interrogativa del tipo «¿cuál…?»;
- **lista de tareas**: una tabla Markdown cuya primera columna es un índice o una casilla, o una lista
  de viñetas con forma de tarea (cosas que marcar);
- **lugar**: una latitud y una longitud, o una dirección.

Los **datos** del widget SHALL tomarse del `data` de la herramienta del turno cuando esa herramienta
haya producido la lista (por ejemplo `tasks::list_tasks`, `tasks::search_tasks`, `email_list_unread`,
`rss_list_unread` o `notifications::list_unread`); en su defecto, SHALL tomarse del texto.

La proyección SHALL declarar, para cada widget proyectado, el **tramo** del texto que ese widget ocupa
(`span`), de modo que el cliente pueda colocar el widget **en su sitio** en lugar de debajo del texto
(ver el requisito siguiente).

Si el modelo ya renderizó con éxito un widget de un tipo (`render_widget`), la proyección **para ese
tipo** SHALL omitirse: el widget del modelo gana, porque su payload es más rico.

#### Scenario: Una lista de opciones acaba en widget sin que el modelo lo pida

**Given** un turno cuyo texto final del asistente presenta dos o más opciones etiquetadas y que **no** llamó a `render_widget`
**When** el turno cierra
**Then** el mensaje del asistente SHALL persistirse con `widgets` no vacío
**And** el widget SHALL ser de opciones, con las opciones del texto
**And** el orquestador SHALL emitir un evento SSE `widget` antes del evento `done`

#### Scenario: Una lista de tareas acaba en widget

**Given** un turno cuyo texto final presenta una lista de cosas que marcar y que **no** llamó a `render_widget`
**When** el turno cierra
**Then** el mensaje del asistente SHALL persistirse con un widget de lista de tareas
**And** sus ítems SHALL conservar el texto de cada elemento

#### Scenario: El widget del modelo gana y no se duplica

**Given** un turno en el que el modelo invocó `render_widget` con un widget de lista de tareas
**And** el texto final presenta además una lista de tareas
**When** el turno cierra
**Then** `widgets` SHALL contener **un solo** widget de lista de tareas, el del modelo
**And** la proyección para ese tipo SHALL omitirse
**And** no SHALL persistirse ningún widget duplicado

#### Scenario: Sin forma reconocible no hay proyección

**Given** un turno cuyo texto final es un dato único o una explicación, sin opciones ni listas que marcar
**When** el turno cierra
**Then** `widgets` SHALL ser `null`
**And** no SHALL emitirse ningún evento SSE `widget`

#### Scenario: Los datos salen del resultado de la herramienta cuando existe

**Given** un turno que llamó a una herramienta que produjo la lista
**And** el texto final presenta esa lista
**When** el turno cierra
**Then** los ítems del widget SHALL salir del `data` de esa herramienta
**And** SHALL NOT depender de lo que el modelo haya escrito en prosa

#### Scenario: Los datos salen del texto cuando no hay herramienta

**Given** un turno que no llamó a ninguna herramienta que produjera la lista
**And** el texto final presenta una lista de opciones
**When** el turno cierra
**Then** las opciones del widget SHALL salir del texto

#### Scenario: La proyección declara el tramo que el widget ocupa

**Given** un turno cuyo texto final presenta una lista de opciones y que **no** llamó a `render_widget`
**When** el turno cierra
**Then** el widget proyectado SHALL llevar un `span` con el tramo del texto que ocupa
**And** el tramo SHALL corresponder exactamente al bloque que el proyector reconoció
**And** SHALL NOT solaparse con el `span` de otro widget del mismo mensaje

### Requirement: El contenido del mensaje del asistente SHALL persistirse íntegro y el widget SHALL declarar el tramo que ocupa

El `content_buffer` que se persiste SHALL ser el texto del asistente **íntegro**: la proyección SHALL NOT
recortar, resumir, reordenar ni reescribir el contenido del modelo, de modo que el índice FTS, el contexto
que ve el modelo (`effective_content`) y la auditoría sigan viendo exactamente lo que el modelo dijo.

Cada widget **proyectado** SHALL llevar un campo `span` con el rango **semiabierto** `[start, end)` —en
**bytes UTF-8** sobre ese contenido— del tramo que el widget ocupa, que SHALL corresponder exactamente al
bloque que `detect` reconoció. Los tramos de un mismo mensaje SHALL NOT solaparse. Los widgets del modelo
(`render_widget`) SHALL NOT llevar `span`: no sustituyen ningún tramo del texto.

El campo `span` SHALL ser **opcional** en todo el recorrido del widget —el evento SSE `widget` y la columna
`messages.widgets`—, de modo que un widget sin `span`, o una fila persistida antes de este change, se
comporte como hoy: el widget se pinta **debajo** del texto. No SHALL escribirse ninguna migración de datos
para añadir el campo a las filas existentes.

El evento `done` SHALL llevar, en el campo `content`, ese mismo contenido íntegro, que es la **base
autoritativa** contra la que los `span` están calculados (ver `stream-route`).

#### Scenario: El contenido persistido conserva el texto íntegro del modelo

**Given** un turno cuyo texto final presenta una lista de opciones
**When** el turno cierra y se persiste el mensaje del asistente
**Then** el contenido persistido SHALL contener el bloque de opciones tal y como lo escribió el modelo
**And** `widgets` SHALL contener el widget de esas opciones
**And** SHALL NOT haberse recortado ninguna parte del texto

#### Scenario: El widget declara el tramo exacto que ocupa

**Given** un turno cuyo texto final rodea la lista de opciones con una frase de encabezado y una de cierre
**When** el turno cierra y se persiste el mensaje del asistente
**Then** el widget SHALL llevar un `span`
**And** el tramo del contenido que el `span` delimita SHALL ser exactamente el bloque de opciones
**And** la frase de encabezado y la de cierre SHALL quedar fuera del tramo

#### Scenario: Los widgets del modelo no declaran tramo

**Given** un turno en el que el modelo invocó `render_widget`
**When** el turno cierra y se persiste el mensaje del asistente
**Then** el widget persistido SHALL NOT llevar `span`

#### Scenario: Un turno sin proyección persiste el texto íntegro

**Given** un turno cuyo texto final no presenta ninguna forma reconocible
**When** el turno cierra y se persiste el mensaje del asistente
**Then** el contenido persistido SHALL ser el texto íntegro del asistente
**And** `widgets` SHALL ser `null`
