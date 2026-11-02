## MODIFIED Requirements

### Requirement: El orquestador SHALL proyectar widgets deterministas al cierre del turno

Al cerrar el turno, y **antes** de persistir el mensaje del asistente, el orquestador SHALL ejecutar una
proyección determinista sobre el texto final del asistente (`content_buffer`): detectar una forma de
presentación **y el tramo que ocupa**, sintetizar el widget correspondiente con ese tramo, fusionarlo
con los widgets ya renderizados por el modelo (`render_widget`), de modo que un widget del modelo gane a
la proyección de su mismo tipo y no se persista ningún duplicado, persistir el resultado con
`MessagesRepo::set_widgets` y emitir el/los evento(s) SSE `widget` **antes** del evento `done` del
turno.

La proyección SHALL NOT depender de que el modelo invoque `render_widget`: es el **servidor** quien
decide la presentación a partir de lo que el modelo está a punto de presentar. La proyección SHALL ser
una función **pura** —sin I/O ni async—, de modo que cada detección y, sobre todo, cada **no** detección
sea verificable con tests unitarios.

El **disparador SHALL ser el texto**: si el texto final no presenta una forma reconocible, no SHALL
proyectarse ningún widget y `widgets` SHALL quedar como `null`, sin cambio alguno respecto al
comportamiento previo. Las formas reconocibles SHALL ser, al menos:

- **opciones**: una lista de opciones etiquetadas (`A)`, `A.`, `A:`, `1)`, `1.`, `Opción A`); una lista de alternativas (en viñetas o numerada) cerrada por una interrogativa del tipo «¿cuál…?», aunque entre la lista y la interrogativa haya **líneas en blanco**; o una **tabla Markdown de alternativas** (primera columna con etiquetas consecutivas `A`, `B`, `C` / `1`, `2`, `3`, o cabecera que nombre opciones: `Opción`/`Opcion`/`Alternativa`/`Option`);
- **lista de tareas**: una tabla Markdown cuya primera columna es un índice o una casilla, o una lista
  de viñetas con forma de tarea (cosas que marcar);
- **lugar**: una latitud y una longitud.
  La forma «dirección» queda **fuera de la v1**: el `LocationWidget` del cliente exige coordenadas
  numéricas y, sin ellas, muestra un aviso en lugar del mapa, de modo que proyectar una dirección suelta
  sería un widget peor que la prosa. Geocodificar no es viable porque la proyección SHALL ser pura.

Los **marcadores de énfasis Markdown** (`**…**`, `__…__`, `*…*`, `_…_`, `` `…` ``) SHALL ignorarse al extraer la etiqueta y el texto de cada opción o ítem, de modo que el widget muestre el texto interior. El `span` SHALL calcularse siempre sobre el texto **original**, sin verse afectado por ese ignorado.

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

#### Scenario: Una lista de opciones separada de la interrogativa por líneas en blanco acaba en widget

**Given** un turno cuyo texto final presenta una lista numerada o con viñetas de dos o más alternativas, seguida de una o más líneas en blanco y de una interrogativa del tipo «¿cuál…?» (por ejemplo «¿Cuál prefiere?»), y que **no** llamó a `render_widget`
**When** el turno cierra
**Then** el mensaje del asistente SHALL persistirse con `widgets` no vacío
**And** el widget SHALL ser de opciones, con las alternativas del texto
**And** el `span` SHALL delimitar únicamente el bloque de la lista, dejando fuera las líneas en blanco y la interrogativa

#### Scenario: Los ítems con énfasis Markdown se muestran sin los marcadores

**Given** un turno cuyo texto final presenta alternativas cuyo texto está envuelto en marcadores de énfasis Markdown (`**…**`, `*…*`, `_…_`, `` `…` ``)
**When** el turno cierra
**Then** las opciones del widget SHALL llevar el texto interior, sin los marcadores Markdown
**And** el `span` SHALL seguir correspondiendo al tramo del texto original

#### Scenario: Una tabla de opciones acaba en widget

**Given** un turno cuyo texto final presenta una tabla Markdown cuyas filas son alternativas etiquetadas con etiquetas consecutivas (`A`, `B`, `C` / `1`, `2`, `3`) o cuya cabecera nombra opciones (`Opción`/`Alternativa`)
**When** el turno cierra
**Then** `widgets` SHALL contener un widget de opciones con las alternativas de la tabla
**And** el `span` SHALL delimitar la tabla

#### Scenario: Una tabla de tareas no se degrada a opciones

**Given** un turno cuyo texto final presenta una tabla Markdown cuya primera columna es un índice o una casilla (forma de lista de tareas)
**When** el turno cierra
**Then** SHALL proyectarse un widget de lista de tareas
**And** SHALL NOT proyectarse un widget de opciones para esa tabla
