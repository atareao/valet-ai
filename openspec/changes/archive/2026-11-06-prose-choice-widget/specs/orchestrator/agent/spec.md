## MODIFIED Requirements

### Requirement: El orquestador SHALL proyectar widgets deterministas al cierre del turno

Al cerrar el turno, y **antes** de persistir el mensaje del asistente, el orquestador SHALL ejecutar una
proyección determinista sobre el texto final del asistente (`content_buffer`): detectar una forma de
presentación **y el tramo que ocupa**, sintetizar el widget correspondiente con ese tramo, fusionarlo con los
widgets ya renderizados por el modelo (`render_widget`), de modo que un widget del modelo gane a la proyección
de su mismo tipo y no se persista ningún duplicado, persistir el resultado con `MessagesRepo::set_widgets` y
emitir el/los evento(s) SSE `widget` **antes** del evento `done` del turno.

La proyección SHALL NOT depender de que el modelo invoque `render_widget`: es el **servidor** quien decide la
presentación a partir de lo que el modelo está a punto de presentar. La proyección SHALL ser una función
**pura** —sin I/O ni async—, de modo que cada detección y, sobre todo, cada **no** detección sea verificable
con tests unitarios.

El **disparador SHALL ser el texto**: si el texto final no presenta una forma reconocible, no SHALL proyectarse
ningún widget y `widgets` SHALL quedar como `null`, sin cambio alguno respecto al comportamiento previo. Las
formas reconocibles SHALL ser, al menos:

- **opciones**: una lista de opciones etiquetadas (`A)`, `A.`, `A:`, `1)`, `1.`, `Opción A`); una lista de
  alternativas (en viñetas o numerada) cerrada por una interrogativa del tipo «¿cuál…?», aunque entre la lista
  y la interrogativa haya **líneas en blanco**; o una **tabla Markdown que declare alternativas**: con una
  cabecera que **nombre** la columna de opciones (`Opción`/`Opcion`/`Alternativa`/`Alternativas`/`Option`/
  `Options`), o con una primera columna cuyas celdas lleven **letras** consecutivas (`A`, `B`, `C`) y ninguna
  celda de la cabecera nombre la columna de opciones;
- **lista de tareas**: una tabla Markdown cuya primera columna es un índice o una casilla, o una lista de
  viñetas con forma de tarea (cosas que marcar);
- **lugar**: una latitud y una longitud.
  La forma «dirección» queda **fuera de la v1**: el `LocationWidget` del cliente exige coordenadas numéricas y,
  sin ellas, muestra un aviso en lugar del mapa, de modo que proyectar una dirección suelta sería un widget
  peor que la prosa. Geocodificar no es viable porque la proyección SHALL ser pura.

- **elección de cierre**: la **última línea con pregunta** del **párrafo final** del turno es una pregunta
  cerrada de sí/no o una pregunta de dos caminos «¿X o Y?»;

Una columna cuyas celdas sean **números** consecutivos SHALL leerse como un **índice** de la tabla (vueltas,
divisiones, enumeraciones de datos) y SHALL NOT producir opciones: la rama heurística sin cabecera que nombre
opciones SHALL exigir etiquetas **alfabéticas**.

Una pregunta de cierre SHALL reconocerse **solo** en el **párrafo final** del turno: SHALL buscarse hacia atrás
la última línea con contenido que contenga `?`, **sin cruzar una línea en blanco** (frontera de párrafo) y sin
salir de un bloque de código delimitado por vallas. La línea SHALL NOT llevar ningún interrogativo abierto
(`qué`, `cuál`, `quién`, `cómo`, `cuándo`, `dónde`, `cuánto`, `por qué`, y sus formas plurales o femeninas
acentuadas `cuáles`, `quiénes`, `cuántos`, `cuántas`, `cuánta`, `adónde`).

Antes de extraer la pregunta, la línea SHALL normalizarse con `strip_line_prefix` y
`strip_markdown_emphasis`. Sobre la línea normalizada, la pregunta SHALL extraerse entre el **último** `¿` de la
línea y el **primer** `?` a partir de ahí. Cuando la línea SHALL NOT llevar `¿`, la pregunta SHALL empezar en su
primer carácter con contenido y SHALL terminar en el primer `?`, que SHALL considerarse de cierre **solo** si
tras él no queda ningún carácter alfanumérico (espacios, emoji, puntuación o marcadores): un `?` con texto
alfanumérico detrás SHALL NOT cerrar la pregunta, de modo que una línea como `Más info en
https://ejemplo.com/x?from=2026-11-01` SHALL NOT ser una pregunta. El `span` SHALL calcularse siempre sobre el
texto **original** y SHALL NOT verse alterado por la normalización.

Si la pregunta contiene una conjunción `o`/`ó`/`u` a nivel de palabra y la palabra siguiente **no** es
`no`/`sí`/`si`, la pregunta SHALL ser de **dos caminos**: SHALL partirse por **la** conjunción y SHALL
emitirse **dos opciones** con los dos tramos. Si la palabra siguiente a la conjunción **sí** es `no`/`sí`/`si`,
la pregunta SHALL ser **binaria** y SHALL emitirse con las dos opciones fijas `Sí` y `No`.

Cada tramo de una pregunta de dos caminos SHALL recortarse, quitar sus guiones o rayas y comas, `;` y `:` finales, quitar
el `¿` inicial y el `?` final, y capitalizar la primera letra. La elección de dos caminos SHALL NOT proyectarse
si: hay **más de una** conjunción `o`/`ó`/`u` (una elección de tres o más caminos), algún tramo queda vacío o
con menos de 3 caracteres alfanuméricos, o algún tramo supera los 160 caracteres.

La elección de cierre SHALL emitirse como `Presentation::ClosingChoice { question, options, span }`, donde
`options` es `["Sí","No"]` en el caso binario o los dos caminos en el caso «¿X o Y?». SHALL NOT proyectarse si
el turno ya produjo un widget de **opciones**: SHALL proyectarse **una sola** cuando ambas formas compitan. La
**pregunta SHALL ser la etiqueta del grupo de opciones** (`label`) y el widget SHALL NOT llevar `title`: el
`span` sustituye el texto que ocupa, de modo que sin la pregunta dentro del widget esta desaparecería de la
pantalla. El `span` SHALL delimitar exactamente la línea de la pregunta, y la etiqueta SHALL llevar el texto de
esa línea sin los marcadores de énfasis Markdown.

El **texto de cada opción de una tabla SHALL ser el de su fila completa**: todas las celdas con contenido de la
fila, en el orden de la tabla, unidas por ` — `, con la etiqueta incluida cuando la haya. La proyección SHALL
NOT quedarse con una sola celda de la fila. Una fila sin ninguna celda con texto SHALL NOT aportar opción, y si
quedan menos de dos opciones no SHALL proyectarse widget.

Los **marcadores de énfasis Markdown** (`**…**`, `__…__`, `*…*`, `_…_`, `` `…` ``) SHALL ignorarse al extraer
la etiqueta y el texto de cada opción o ítem, de modo que el widget muestre el texto interior. El `span` SHALL
calcularse siempre sobre el texto **original**, sin verse afectado por ese ignorado.

Los **datos** del widget SHALL tomarse del `data` de la herramienta del turno cuando esa herramienta haya
producido la lista (por ejemplo `tasks::list_tasks`, `tasks::search_tasks`, `email_list_unread`,
`rss_list_unread` o `notifications::list_unread`); en su defecto, SHALL tomarse del texto.

La proyección SHALL declarar, para cada widget proyectado, el **tramo** del texto que ese widget ocupa
(`span`), de modo que el cliente pueda colocar el widget **en su sitio** en lugar de debajo del texto (ver el
requisito siguiente).

Si el modelo ya renderizó con éxito un widget de un tipo (`render_widget`), la proyección **para ese tipo**
SHALL omitirse: el widget del modelo gana, porque su payload es más rico.

La verificación de la elección de cierre SHALL incluir un **corpus de regresión con cadenas literales extraídas
de producción**, además de los casos sintéticos, de modo que un cambio en la normalización o en las guardas no
pueda volver a pasar en verde sin cubrir la prosa real del modelo.

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

**Given** un turno cuyo texto final presenta una tabla Markdown cuya cabecera nombra la columna de opciones (`| Opción | Acción |`) y cuyas filas llevan la etiqueta en esa columna y el texto de la alternativa en la siguiente
**When** el turno cierra
**Then** `widgets` SHALL contener un widget de opciones
**And** cada opción SHALL ser el texto completo de su fila, con la etiqueta incluida
**And** el `span` SHALL delimitar la tabla

#### Scenario: Una tabla con una columna de números no es una tabla de opciones

**Given** un turno cuyo texto final presenta una tabla Markdown de datos cuya primera columna es un índice numérico consecutivo (`| Lap | Dist. | Ritmo | Pulso |`, `| Split | Ritmo | Desnivel |`)
**When** el turno cierra
**Then** NO SHALL proyectarse ningún widget de opciones para esa tabla

#### Scenario: Una tabla de tareas no se degrada a opciones

**Given** un turno cuyo texto final presenta una tabla Markdown cuya primera columna es un índice o una casilla (forma de lista de tareas)
**When** el turno cierra
**Then** SHALL proyectarse un widget de lista de tareas
**And** SHALL NOT proyectarse un widget de opciones para esa tabla

#### Scenario: Una fila sin contenido no aporta opción

**Given** una tabla Markdown de opciones en la que una fila no tiene ninguna celda con texto
**When** el turno cierra
**Then** esa fila SHALL NOT aportar ninguna opción al widget

#### Scenario: Una pregunta de sí/no que cierra el turno acaba en widget de elección

**Given** un turno cuyo texto final es `> ¿Le saco el detalle?` o `> ¿Cerramos por hoy, Lorenzo?` y que **no** llamó a `render_widget`
**When** el turno cierra
**Then** `widgets` SHALL contener un widget de elección con las opciones `Sí` y `No`
**And** la pregunta SHALL ser la etiqueta del grupo de opciones
**And** el widget SHALL NOT llevar `title`
**And** el `span` SHALL delimitar la línea de la pregunta

#### Scenario: El emoji tras el signo de interrogación no impide la elección

**Given** un turno cuyo texto final es `> ¿Le doy el parte del día — tiempo, agenda y entrenamiento — o va directo a algo concreto? ☕️` o `> ¿Le pongo un recordatorio un rato antes, o lo dejo así? ☕️`
**When** el turno cierra
**Then** SHALL proyectarse un widget de elección
**And** el emoji SHALL NOT impedir la elección **porque la pregunta termina en el `?`**: lo que sigue al `?` SHALL ser solo no alfanumérico
**And** el `span` SHALL seguir delimitando la línea original, emoji incluido

#### Scenario: Una pregunta con alternativas nombradas no es una elección binaria

**Given** un turno cuyo texto final es `> ¿Le busco el repositorio y las instrucciones de instalación, o lo dejamos para mañana con luz? ☕️`
**When** el turno cierra
**Then** `widgets` SHALL contener un widget de elección con **dos** opciones, los dos caminos
**And** la primera opción SHALL ser `Le busco el repositorio y las instrucciones de instalación`
**And** la segunda opción SHALL ser `Lo dejamos para mañana con luz`
**And** SHALL NOT emitirse las opciones fijas `Sí` y `No`
**And** la pregunta completa SHALL ser la etiqueta del grupo de opciones
**And** el widget SHALL NOT llevar `title`

#### Scenario: Una pregunta de dos caminos con la conjunción en medio de la frase se parte por la última conjunción

**Given** un turno cuyo texto final es `> ¿Quiere que le suba el tono de guasa o se lo deje más sobrio? Confirme y sale disparado. 🎩`
**When** el turno cierra
**Then** SHALL proyectarse un widget de elección con las opciones `Quiere que le suba el tono de guasa` y `Se lo deje más sobrio`
**And** el `span` SHALL delimitar la línea de la pregunta

#### Scenario: La forma binaria «o no» se mantiene como Sí/No

**Given** un turno cuyo texto final es `> ¿Hacemos esto o no?`
**When** el turno cierra
**Then** `widgets` SHALL contener un widget de elección con las opciones `Sí` y `No`
**And** SHALL NOT emitirse dos opciones con los tramos de la pregunta

#### Scenario: Una pregunta de tres caminos no se proyecta

**Given** un turno cuyo texto final es `> ¿Prefieres A, B o C?`
**When** el turno cierra
**Then** NO SHALL proyectarse ningún widget de elección

#### Scenario: Una pregunta abierta no es una elección binaria

**Given** un turno cuyo texto final es `> ¿Qué necesita? 😒`, `> ¿Cuál, Lorenzo?` o `> ¿Por dónde empezamos, Lorenzo?`
**When** el turno cierra
**Then** NO SHALL proyectarse ningún widget de elección

#### Scenario: Un cierre sin pregunta no proyecta elección

**Given** un turno cuyo texto final es `> Dígame. ☕️` o `> Todo cerrado, Lorenzo. **Ahora sí, a la calle** — el progresivo no se corre desde el sofá. 😒☕️`
**When** el turno cierra
**Then** NO SHALL proyectarse ningún widget de elección

#### Scenario: La pregunta no tiene que ser la última línea del párrafo final

**Given** un turno cuyo texto final es `### 📋 ¿Seguimos?` o una línea `…o se lo deje más sobrio? Confirme y sale disparado. 🎩`
**When** el turno cierra
**Then** SHALL proyectarse un widget de elección para esa pregunta
**And** el `span` SHALL delimitar la pregunta: hasta el final de la línea cuando tras el `?` solo haya emoji o puntuación, y hasta el `?` inclusive cuando tras él venga prosa

#### Scenario: Una pregunta que no cierra el turno no proyecta elección binaria

**Given** un turno cuyo texto final lleva una pregunta de sí/no en un párrafo anterior, seguida de una línea en blanco y de un párrafo final sin pregunta
**When** el turno cierra
**Then** NO SHALL proyectarse ningún widget de elección

#### Scenario: Una pregunta dentro de un bloque de código no se proyecta

**Given** un turno cuyo texto final contiene una pregunta de sí/no dentro de un bloque de código delimitado por vallas
**When** el turno cierra
**Then** NO SHALL proyectarse ningún widget de elección para esa pregunta

#### Scenario: El turno ya proyectó opciones y no se añade la elección binaria

**Given** un turno cuyo texto final presenta una lista de alternativas cerrada por una interrogativa y además una pregunta de sí/no al final
**When** el turno cierra
**Then** SHALL proyectarse un solo widget de opciones
**And** SHALL NOT proyectarse ningún widget de elección de cierre

#### Scenario: Un procedimiento seguido de una pregunta de sí/no no se degrada a opciones

**Given** un turno cuyo texto final presenta una lista numerada de pasos y termina en una pregunta cerrada de sí/no (por ejemplo «1. Abre el fichero\n2. Edita la línea\n¿Quieres que continúe?»)
**When** el turno cierra
**Then** SHALL proyectarse un widget de elección con las opciones `Sí` y `No`
**And** SHALL NOT proyectarse un widget de opciones para el procedimiento
