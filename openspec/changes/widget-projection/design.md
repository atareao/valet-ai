# Design

## Context

Ver `proposal.md` → *Why* para la motivación y la evidencia. Lo que condiciona el diseño:

- **El punto de integración ya existe y está aislado.** El cierre del turno en
  `src/orchestrator/agent.rs` ensambla `content_buffer`, recoge `rendered_widgets` de las llamadas del
  modelo a `render_widget`, y persiste con `MessagesRepo::set_widgets` justo antes de
  `publish_persisted_message` (sobre la línea 1777; el evento SSE `widget` se emite en vivo sobre la
  1663, cuando `render_widget` tiene éxito). La proyección entra **en ese cierre**, antes de persistir:
  no hay que tocar el bucle ReAct ni la emisión en vivo del widget del modelo.
- **El widget ya tiene un payload canónico, y el proyectado solo le añade un campo.** `messages.widgets` guarda
  `[{ id, name, data }]` como `serde_json::Value` —el modelo y el repo lo tratan como **JSON opaco**, sin tipo— y el
  frontend lo reconstruye al cargar el historial (`WIDGET_REGISTRY` + `WidgetRenderer`). El proyector **produce ese
  mismo formato** más un campo opcional `span`: el tramo del contenido que el widget ocupa. Añadir un campo a un
  JSON opaco **no toca el modelo de datos, ni el repo, ni la migración**: el campo es opcional, así que las filas
  antiguas y los widgets del modelo se comportan como hoy.
- **Los tres widgets y sus `data` están fijados por el registry.** `render_widget` valida `widget_name`
  contra `QuickForm`, `Checklist` y `LocationWidget`, y su definición documenta el `data` de cada uno
  (`fields[]` con `select`, `items[]` con `id`/`label`, `latitude`/`longitude`). El proyector reutiliza
  esos contratos, no inventa ninguno.
- **El evento `done` hoy NO lleva el contenido.** `SSEEvent::Done` trae `message_id`, `user_message_id`,
  `location`, `tools_used`, `user_location` y `user_created_at`. El frontend (`useSSE.ts` / `useMainChat.ts`)
  acumula los `chunk` y no recibe un texto autoritativo. Con la opción (c) el texto persistido coincide con el
  streameado, así que el campo deja de ser «lo que hay que reemplazar» y pasa a ser la **base autoritativa contra
  la que los `span` están calculados**: el `done` gana un campo `content` con el texto íntegro.
- **La guía viaja en todos los turnos por el umbral cero de `widgets`** (`widgets-always-on`, D1–D3).
  Ese mecanismo **no se toca**: la guía sigue viajando, solo se recorta.
- **El modelo imita su propio precedente** (21 de los últimos 30 mensajes de asistente de producción
  son listas en prosa). Es la razón de fondo del giro: la decisión no puede depender del modelo.

## Goals / Non-Goals

**Goals:**

- Que un turno cuya respuesta **presenta** una lista de opciones, una lista de tareas o un lugar acabe
  con un widget, **sin depender de que el modelo llame a `render_widget`**.
- Que la detección sea **determinista y pura**, de modo que tenga tests unitarios de verdad (incluidas
  las NO detecciones) y no haga falta medirla contra el modelo.
- Que el widget **ocupe el sitio** de la lista que representa, sin reescribir el texto: el mensaje es lo que el
  modelo dijo, y el widget se coloca sobre el tramo que sustituye.
- Que `render_widget` siga funcionando como **override explícito** y no pierda riqueza.

**Non-Goals:**

- No se elimina `render_widget` ni su exposición en cada turno.
- No se añade ninguna columna, ni migración de datos, ni backfill: `messages.widgets` sigue siendo el mismo JSON
  opaco, ahora con un campo opcional `span`. No se toca el esquema de `render_widget`, ni el registry de widgets del
  frontend, ni el `data` de cada widget, ni el mecanismo de umbral cero de `widgets`.
- No se toca el mecanismo de umbral cero de `widgets` (la guía sigue viajando en todos los turnos).
- No se implanta detección semántica (LLM) ni heurísticas de intención: solo análisis del texto final.

## Decisions

### D1 — Alcance v1: `Options` + `Checklist` (+ `Location` si sale barato)

**Decisión (recomendada)**: la v1 del proyector detecta y proyecta **`Options`** (→ `QuickForm` con un
campo `select`) y **`Checklist`** (→ `Checklist`). `Location` (→ `LocationWidget`) se incluye **solo si
el coste de detección es bajo**: una latitud/longitud con formato claro, o una dirección reconocible.

**Por qué**:

- `Options` es el caso que **mide la batería** (los tres prompts reales responden con `A`/`B`/`C`). Es
  el que hay que cerrar sí o sí.
- `Checklist` es el caso que **el experimento de aislamiento sí logró** con la guía nueva (el único
  `Checklist` con tareas reales). Es la segunda forma de lista y comparte el parser de viñetas.
- `Location` es el widget con **más falsos positivos potenciales** (un número con decimales no es una
  coordenada) y su `data` exige `latitude`/`longitude` reales. Meterlo a la fuerza en la v1
  comprometería la regla «ante la duda, no proyectar». Si sale barato, entra; si no, se aparca.

**Alternativa descartada — solo `Options`**: dejar `Checklist` fuera simplifica la v1, pero deja fuera
justo el único caso que la guía nueva logró y que el patrón de producción muestra («que más tenemos que
despachar?» es, literalmente, una lista de cosas que marcar). El coste marginal de `Checklist` es bajo
porque comparte parser con `Options`.

### D2 — Composición: `content` verbatim y el widget declara su `span` (opción c)

**Decisión del usuario (2026-11-02)**: el texto del asistente se persiste **íntegro** —nunca se recorta— y cada
widget proyectado lleva un campo opcional `span`: el rango del contenido que el widget **ocupa**. El cliente
compone: pinta los tramos de texto y, **en el sitio del tramo**, el widget. El mensaje deja de ser «un texto con
adjuntos debajo» para ser una **secuencia de tramos**: texto, widget, texto.

**Por qué no recortar (opción a, la anterior de este diseño)**:

- `messages.content` alimenta **tres cosas a la vez**: el render del frontend, el contexto que ve el modelo
  (`effective_content = COALESCE(collapsed_content, content)`) y el índice FTS (los triggers
  `messages_fts_ai/au/ad` indexan `content`). Recortarlo deja al modelo sin su propio historial, saca la lista del
  buscador y **borra el único dato con el que se depuraría el proyector** si un día recorta de más.
- El repo **ya separa canónico de derivado** (`content` vs `collapsed_content`): recortar `content` iría contra el
  grano del modelo de datos.
- Y es lo que hace la referencia: **opencode** guarda el mensaje del asistente **verbatim** —`reasoning`, `text` y
  `tool` tal cual, en orden— y compone la UI del orden de sus bloques; nunca reescribe el registro (ver D9).

**Por qué `span` y no una columna `display_content` (opción b)**: `display_content` es una **copia** del texto
recortado; el `span` son **dos enteros** dentro del JSON que ya existe. La copia hay que mantenerla coherente con
`content` para siempre (y con cada mejora del detector), y además **no dice dónde iba el widget**, así que tampoco
sirve para colocarlo en su sitio. El `span` no duplica texto, no añade columna, no añade migración y **sí** coloca.

**Contrato del `span`** (fijado aquí para que Rust y TypeScript no puedan discrepar):

- Rango **semiabierto** `[start, end)` en **bytes UTF-8** sobre el contenido persistido del mensaje.
- **Opcional.** Sin `span`, el widget se pinta **debajo** del texto, como hoy: es el camino de los widgets del
  modelo (`render_widget`) y de **todas** las filas anteriores a este change. **Sin backfill.**
- Los tramos de un mismo mensaje SHALL NOT solaparse.
- El `content` del evento `done` es la **base autoritativa** contra la que el `span` está calculado (ver
  `stream-route`). Sin ella, el cliente compondría contra su propio texto acumulado y una normalización de un solo
  carácter en el servidor desviaría el corte; con ella, esa clase de fallo no existe.

**Coste y riesgo honesto**: el cliente pasa a hacer aritmética de tramos. Se acota con dos reglas: (1) la
composición es una **función pura y testeable** (`composeMessage(content, widgets)`), con tests de los casos límite
—`span` fuera de rango, solapado, texto vacío, widget sin `span`—; y (2) **ante cualquier `span` inválido se cae al
camino de hoy** (texto entero + widgets debajo). La degradación es siempre de colocación, nunca de contenido.

**Qué NO cambia**: ni la columna `messages.widgets` (sigue siendo `Option<Value>`, JSON opaco), ni
`MessagesRepo::set_widgets`, ni la migración, ni el registry de widgets, ni el `data` de cada widget.

### D3 — De dónde salen los datos: de la herramienta cuando exista, del texto cuando no

**Decisión (recomendada)**: `synthesize` toma los ítems del `data` de la herramienta del turno si el
turno llamó a una herramienta que **produjo esa lista** (`tasks::list_tasks`, `tasks::search_tasks`,
`email_list_unread`, `rss_list_unread`, `notifications::list_unread`); si no, los toma del texto.

**Por qué**:

- Cuando hay herramienta, su `data` es **la verdad** (ids reales, etiquetas reales). Parsear la prosa
  del modelo para reconstruir lo que el modelo ya recibió de la herramienta es un rodeo con pérdida.
- El experimento de aislamiento lo confirma: el único `Checklist` que salió llevaba las tareas
  **reales**; cuando el modelo inventa o resume, el `data` de la herramienta evita el invento.
- Cuando no hay herramienta (una lista de alternativas que el modelo compone de su cabeza), el texto es
  la única fuente. Es el caso de los tres prompts de la batería.

**Riesgo — «widgets de más»** y cómo se acota:

- El proyector decide **sin** el modelo, así que un falso positivo (widget donde no toca) es peor que
  un falso negativo. Por eso el **disparador es el texto**: solo se proyecta si el texto *presenta* una
  lista con la forma esperada (`detect` no devuelve `None`). Sin forma reconocible, no hay widget.
- El riesgo se acota además con la batería de tests: **cada NO detección es un test** (un dato único,
  una explicación, una tabla sin columna índice/casilla, un número suelto, una enumeración en prosa
  dentro de una frase).
- El falso negativo es benigno: se queda la prosa de siempre. El sistema nunca empeora por no
  proyectar.

### D4 — Qué hacer con `widgets-always-on` (34/38 tareas, batería 6.3 fallida)

**Decisión del usuario (2026-11-02): archivarlo antes de este change, y ya ejecutada.** El
`openspec archive widgets-always-on --yes` se ejecutó el **2026-11-02**, antes de empezar el código de
este change, y el change quedó en `openspec/changes/archive/2026-11-02-widgets-always-on`. Se archivó
documentando que su batería de aceptación falló, porque:

- Su **código está mergeado y desplegado** (v0.25.0): la herramienta se expone en todos los turnos y la
  guía viaja en el mensaje de sistema. Eso no se revierte: este change lo **supersede**, no lo anula.
- Sus **spec deltas sí están verificados**: la exposición de `render_widget` y la inyección del
  fragmento tienen tests (end-to-end de `agent.rs` y unitarios del router). Lo único que falló es la
  batería 6.3, que **no tenía test unitario** —es la verificación de un prompt— y que este change
  sustituye por una verificación determinista.
- Dejar el change abierto de forma indefinida bloquea el mecanismo de archive y mantiene un change
  «huérfano» en `openspec list` (patrón ya visto en este repo).

**Estado**: hecho el 2026-11-02 (`openspec archive widgets-always-on --yes` →
`openspec/changes/archive/2026-11-02-widgets-always-on`). No queda como recomendación pendiente ni
como tarea a ejecutar: su delta `MODIFIED` del requisito de la guía ya está volcado en la spec
principal, que es el estado sobre el que trabaja este change.

**Nota de modelado del delta de `skill-router`**: el requisito de la guía no se puede modelar como
`MODIFIED`. Un `MODIFIED` reemplaza el bloque entero y el archive **se niega a perder escenarios**; dos
de los ocho escenarios heredados (`La guía nombra los tres widgets y la regla de las opciones` y `La
guía no ampara la prosa con opciones`) afirman el contenido de la guía prescriptiva que este change
deroga, así que tienen que **desaparecer**. El delta se modela por tanto como **`REMOVED` + `ADDED`**:
se retira el requisito viejo y se añade el nuevo. El CLI rechaza `REMOVED` y `ADDED` con el **mismo**
encabezado (`Requirement present in both ADDED and REMOVED`), de modo que el requisito añadido lleva un
encabezado nuevo —`El fragmento de la skill widgets SHALL ser un recordatorio breve que remita a
render_widget`— y, para no dejar colgando la referencia cruzada de `db/schema`, ese change incluye
además un delta `MODIFIED` de `db/schema` que actualiza la cita al encabezado nuevo conservando todos
sus escenarios. El resto de escenarios heredados (los seis que siguen vigentes) se copian **verbatim**
del estado actual de la spec.

**Alternativa descartada — reabrir `widgets-always-on` y meter la proyección dentro**: sería mezclar
dos mecanismos con ciclos de vida distintos (uno desplegado y fallido, otro nuevo) en un mismo change,
y complicaría el archive. Un change nuevo, con su propio plan, deja la historia limpia.

**Dependencia de orden (importante)**: `widgets-always-on` llevaba un delta `MODIFIED` **del mismo
requisito** de la guía que toca este change (`La guía de widgets SHALL residir en el fragmento de su
skill`). El archive reemplaza el requisito por el contenido del delta que se archiva. Por tanto, si se
hubiera archivado **después** de este change, repondría la guía extensa y pisaría el recordatorio. Por
eso se archivó **antes** (D4), ya ejecutado el 2026-11-02.

### D5 — Dónde vive el contrato del campo `content` del evento `done`

**Decisión**: el campo `content` del evento `done` se documenta en **`stream-route`** (el requisito que
ya enumera un campo del `done`, `user_message_id`), y la **proyección y el contenido persistido** se
documentan en **`orchestrator/agent`**.

**Por qué**: el evento `done` viaja por `POST /api/chat/stream`, y el requisito de `stream-route` ya
documenta uno de sus campos; es el sitio natural para documentar el nuevo. La proyección, en cambio, es
comportamiento del orquestador (detección, síntesis, fusión, persistencia), y vive en `agent`.

### D6 — `render_widget` NO cambia de rol: sin delta en `tools/registry`

**Decisión**: no se modifica ningún requisito de `tools/registry`.

**Por qué**: la herramienta sigue registrada, sigue validando `widget_name` contra la lista permitida y
sigue emitiendo el evento `widget` al ejecutarse. Lo que cambia es que **además** el servidor proyecta
widgets sin ella; que un widget del modelo «gane» sobre la proyección es comportamiento del
orquestador (D-a, en `agent`), no del registry. El esquema de `data` de cada widget se reutiliza tal
cual. `render_widget` pasa de «la vía» a «la vía explícita de override», pero su contrato observable no
cambia.

### D7 — La guía se recorta con una migración quirúrgica, no se elimina

**Decisión**: una migración nueva (`20261102000001_widget_projection.sql`) sustituye el contenido de
`SKILL_WIDGETS_PROMPT` por un **recordatorio breve** (una o dos frases que remiten a que el widget ya se
proyecta), **solo si su valor sigue siendo byte a byte el texto sembrado por
`20261101000001_widgets_always_on.sql`**. Si el usuario lo ha editado, no se pisa.

**Por qué**:

- La guía larga (3.812 car. ≈ 1.000 tokens) viaja en **todos** los turnos por el umbral cero de
  `widgets`. Recortarla ahorra ese coste por turno, que era el efecto lateral medido.
- Se conserva el **mecanismo** (la guía sigue en `SKILL_WIDGETS_PROMPT`, sigue viajando, sigue sujeta a
  la regla anti-duplicado) porque retirarlo obligaría a deshacer `widgets-always-on` (D4). Lo que
  cambia es **cuánto pesa**, no dónde vive.
- El idioma de la guarda (pisa solo si sigue prístino) es el del repo y respeta a quien haya
  personalizado la guía.

**Nota de coherencia con D4**: la migración de recorte **depende** de que `widgets-always-on` esté
desplegado (la guía larga es la que hay que recortar). Si el usuario decidiera **no** archivar aquel
change, esta migración sigue siendo válida: recorta el texto que hay en la base de datos.

**Texto aprobado (2026-11-02)**: el recordatorio queda **aprobado por el usuario el 2026-11-02** y es
el literal que se transcribe en el requisito de `orchestrator/skill-router` (y, carácter a carácter, en
la migración). No es un borrador a fijar en la fase de migración: es el texto que gobierna.

> Los widgets los proyecta el servidor a partir de tu respuesta: si presentas una lista de opciones,
> una lista de tareas o un lugar, aparecerá un widget. No lo anuncies ni repitas la lista en prosa;
> escribe solo la prosa que lo acompaña. Usa `render_widget` únicamente si necesitas un widget con
> datos más ricos de los que el servidor puede inferir.

### D8 — Structured outputs / `response_format: json_schema` (descartada sin medir)

**Qué es**: la técnica estándar para forzar la **forma** de la salida de un modelo. OpenAI Structured
Outputs (`response_format: {type: "json_schema", strict: true}`), el *strict tool use* de Anthropic, la
*controlled generation* de Gemini (`responseSchema`), el `generateObject`/`streamObject` del Vercel AI
SDK, Instructor, y las librerías de **decodificación restringida** (Outlines, XGrammar, llguidance, las
gramáticas GBNF de llama.cpp) son la misma idea con distintas superficies.

**Mecanismo**: el esquema se compila a una gramática, la gramática a un autómata, y el autómata
**enmascara los logits** token a token, de modo que la salida no puede violar la forma declarada. Es
potente y es lo que hace que la técnica parezca la respuesta obvia a «que el modelo devuelva un
`presentation`». Pero **garantiza la forma, no el juicio**: puede obligar a que exista un campo
`presentation`, no a que el modelo acierte si el texto es una lista.

**Argumento decisivo — `render_widget` ya es structured output**: la herramienta tiene un `parameters`
que es un esquema JSON, viaja delante del modelo en **cada** turno (por el umbral cero de `widgets`) y
valida `widget_name` contra `QuickForm`/`Checklist`/`LocationWidget`. El modelo lo tuvo delante y **no
lo usó**. Cambiar `tool_choice` por `response_format: json_schema` es **la misma apuesta con otra
superficie**: sigue pidiendo al modelo que decida, que es justo la decisión que este change saca del
modelo (D1–D3).

**Matiz honesto**: un *tool* es **opcional**; un `json_schema` es **obligatorio**, así que fuerza una
elección **tipada en cada turno** —más fuerte que un prompt—. El problema es que fuerza la
clasificación también en los turnos **sin** lista (una explicación, un dato único), y entonces un
`presentation` equivocado deja de ser «prosa mejorable» para convertirse en «widget mal puesto». El
riesgo se invierte pero no desaparece: sigue siendo el modelo quien juzga.

**Coste en Valet**: rompe el **streaming** —la respuesta entera tendría que ser JSON, y para que se
viera el texto habría que inventar un esquema híbrido texto+widget y confiar en el orden de claves, que
es precisamente el problema para el que existe `streamObject`—, es una **reforma del protocolo** (todo
el camino de `chunk`/`done` y el cliente cambian), y **depende del proveedor**: muchos «JSON mode»
garantizan **JSON válido**, no **adherencia al esquema**, y habría que verificar el comportamiento real
de `deepseek/deepseek-v4.1-flash` en OpenRouter antes de contar con la garantía.

**Decisión del usuario (2026-11-02)**: se documenta como **descartada sin medir**. El motivo es que la
relación coste/beneficio es adversa —reforma de protocolo, pérdida de streaming y dependencia del
proveedor a cambio de una garantía de forma— y, sobre todo, que la técnica **no saca la decisión del
modelo**, que es lo que este change persigue.

### D9 — El destino es el modelo de bloques; `span` es su forma barata y presente

**Decisión del usuario (2026-11-02)**: el destino del mensaje del asistente es el **modelo de bloques**: una
secuencia ordenada de bloques tipados, de modo que en el mismo sitio puedan convivir `text`, `widget` y —más
adelante— `image`, `chart` y `citation`. Esta entrega **no** introduce la columna de bloques: introduce el `span`,
que es ese mismo modelo reducido a lo que hoy hace falta (un bloque `widget` colocado en su sitio). La columna
`blocks` explícita se decidirá **con el proyector ya validado**; entonces `span` será su caso degenerado.

**Qué se copia de la referencia (opencode v2.2.5, medido el 2026-11-02 sobre la base local del usuario)**:

| Hallazgo | Dato |
|:---|:---|
| El mensaje del asistente es un **array ordenado de bloques** (`reasoning` / `text` / `tool`), dentro de la propia fila del mensaje | `tool` 277.476 · `reasoning` 192.266 · `text` 95.379 |
| El orden **es** la composición: cada bloque se pinta donde va | orden observado `reasoning → text → tool → tool → …` |
| El contenido se guarda **verbatim**, sin reescribir y sin campo derivado del texto | las claves del mensaje son `agent`, `model`, `content`, `finish`, `cost`, `tokens`, `time`, `error`, `snapshot` |
| Las tablas de partes (`part`, `session_part`) están **vacías**: el contenido vive embebido en la fila del mensaje | 0 filas en ambas |
| La UI la define el **esquema de la herramienta**, no la prosa: `question` recibe `{questions:[{question, header, options:[{label, description}]}]}` y devuelve `{answers:[[…]]}` | 1.348 llamadas (814 en 30 días) |
| Aun así la prosa filtra: hay listas de opciones escritas a mano dentro de bloques de texto | 1.753 |

**Lo que eso decide para Valet**:

1. **`content` verbatim** (regla 1 de la referencia).
2. **La composición vive en la fila del mensaje, no en una tabla aparte**: la referencia se movió *desde* una tabla
   de partes *hacia* un array embebido (`part` y `session_part` vacías).
3. **El widget en su sitio**, vía el orden de los bloques. En Valet, que **sintetiza** el bloque en vez de
   recibirlo autorado, ese orden se expresa con `span`.
4. **Nada de confiar en que el modelo use la herramienta**: la referencia tiene un `question` dedicado, bloqueante,
   con el modelo ya en «modo herramienta» (2,9 llamadas a herramienta por cada bloque de texto) y **aun así**
   escribe 1.753 listas en prosa. Valet, cuyo turno **termina** en prosa, midió 0/3.

**Por qué la columna `blocks` no entra ahora**: la proyección **aún no está validada en el mundo real**. Montar el
modelo de presentación completo antes de saber si el proyector acierta es levantar el andamio antes de saber si el
edificio se sostiene; y una columna `blocks` arrastra la decisión de qué hacer con las filas existentes y con todo
lo que lee `widgets`. El `span` compra **la parte que de verdad se usa hoy** —la colocación— sin arrastrar nada, y
deja la puerta abierta: cuando lleguen `image`, `chart` y `citation`, el `span` se convierte en el bloque `widget`
de una lista de bloques y el resto del diseño no se mueve.

**Roadmap de bloques** (fuera de este change): `image` (una URL o un adjunto devuelto por una herramienta),
`chart` (las series que ya produce el panel de estadísticas) y `citation` (una cita por fuente de `web_search` /
`unified_search`). Los tres son **aditivos** sobre el mismo contrato: un tipo nuevo, un componente nuevo.

## Risks / Trade-offs

| Riesgo | Mitigación |
|:---|:---|
| **Widgets de más** (falso positivo del proyector) | El disparador es el texto: solo se proyecta si `detect` reconoce la forma. Un test por cada NO detección. Ante la duda, `None`. |
| **Detección frágil** ante listas que el modelo formatea de forma inesperada | El falso negativo es benigno (queda la prosa). El formato se documenta en el delta de `agent` y se cubre con tests de cada variante. |
| **Aritmética de tramos en el cliente** (el `span` se corta mal) | La composición es una función pura con tests de los casos límite; el `content` del `done` es la base autoritativa contra la que se calculan los `span`; y ante cualquier `span` inválido (fuera de rango, solapado, contenido vacío) se cae al camino de hoy: texto entero y widgets debajo. |
| **Un `span` que se desvía por una normalización del servidor** | El `span` se calcula sobre el **mismo** texto que se persiste y que viaja en el `content` del `done`; el cliente compone contra ese valor, no contra su texto acumulado. Test de que el `content` del `done` coincide con el persistido. |
| **La fusión con `rendered_widgets` duplica un widget** | La deduplicación es por **nombre de widget + payload**; `render_widget` gana por tipo (D2 de `proposal`). Test unitario de la fusión. |
| **Recortar la guía reintroduce el problema si la proyección falla** | `render_widget` sigue expuesto y funcionando; la guía recortada solo remite a él. La proyección no sustituye al modelo en los casos que el modelo resuelve mejor. |
| **La migración de recorte es byte a byte** | Se verifica el texto de producción antes de escribirla (idioma del repo). Si el usuario editó la guía, no se pisa y la proyección sigue funcionando igual. |
| **La verificación ya no depende del modelo** | Es una mejora, no un riesgo: la batería de aceptación se sustituye por tests deterministas del proyector y una comprobación sobre `messages.widgets` (ver `tasks.md` → verificación end-to-end). |

## Alternativas descartadas (resumen)

| Alternativa | Por qué se descarta |
|:---|:---|
| **Seguir afinando el prompt** (más guía, más ejemplos, `tool_choice: required`) | Medido: guía nueva perseverante ⇒ 0/3 en conversación real y 1/3 en conversación vacía. El modelo imita su precedente y el prompt base ordena listas. `tool_choice: required` forzaría el tool en turnos que no lo necesitan. |
| **`tool_choice: required`** para `render_widget` | Renderizaría un widget en **todos** los turnos, incluidos los que no presentan lista (explicaciones, datos únicos). Es el extremo contrario del problema. |
| **Detección semántica por LLM** (una segunda llamada que clasifique «¿es una lista?») | Añade latencia, coste y una dependencia del modelo para algo que un parser determinista resuelve. Además, reintroduce la no-determinación que es la causa del fallo actual. |
| **Structured outputs / `response_format: json_schema`** | Garantiza la forma, no el juicio; `render_widget` ya es structured output y no se usó; obliga a una clasificación tipada en cada turno y rompe el streaming (reforma del protocolo). Descartada sin medir por decisión del usuario (2026-11-02). |
| **Solo `Options`** (D1) | Deja fuera el único caso que la guía nueva logró y el patrón más común de producción (listas de tareas). |
| **Reabrir `widgets-always-on`** (D4) | Mezcla dos mecanismos con ciclos de vida distintos y complica el archive. |
| **Retirar `render_widget` y su guía del todo** | El payload del modelo puede ser más rico que el que el proyector infiere del texto. La herramienta es la vía de override; degradarla basta. |
| **Recortar el texto persistido** (D2-a) | Pierde la lista para el FTS, deja al modelo sin su propio historial y borra el texto original, que es el dato con el que se depurarían los recortes del proyector. |
| **Columna derivada `display_content`** (D2-b) | Es una copia del texto que hay que mantener coherente para siempre y no dice **dónde** iba el widget: no coloca. El `span` no duplica texto y sí coloca. |
| **Columna `blocks` explícita ahora** (D9) | La proyección no está validada todavía: pagar el modelo de presentación completo antes de saber si el proyector acierta, y decidir de paso qué hacer con las filas existentes, es invertir el orden. El `span` cubre la necesidad real de hoy y es el peldaño hacia los bloques. |
| **Confiar en un tool bloqueante tipo `question`** (D9) | Funciona en opencode porque su turno vive en «modo herramienta» y la respuesta vuelve dentro del turno. En Valet el turno **termina** en prosa y `render_widget` es *fire-and-forget*: medido, 0/3. |

## Migration Plan

1. **Migración `20261102000001_widget_projection.sql`** (idempotente y quirúrgica): sustituye
   `settings.SKILL_WIDGETS_PROMPT` por el recordatorio breve **solo si su valor es byte a byte el
   sembrado por `20261101000001_widgets_always_on.sql`**. No toca `system_prompt` (la guía ya no vive
   ahí) ni ningún umbral.
2. **Sin migración de datos de mensajes**: `messages.widgets` es JSON opaco y el campo `span` es opcional; las
   filas anteriores a este change se renderizan con el camino de hoy (texto entero + widgets debajo).
3. **Despliegue**: el módulo y la integración viajan con el próximo release de la imagen.
4. **Rollback**: revertir el commit. La migración inversa repone la guía larga; el proyector deja de
   actuar y el sistema vuelve al estado de `widgets-always-on` (guía larga + `render_widget`). No hay
   estado que deshacer salvo la fila de settings.

## Open Questions

- **D1 — ¿entra `Location` en la v1?** Se decide durante el RED: si el detector de coordenadas/dirección
  sale con pocos falsos positivos y un test por variante, entra; si no, se aparca a una v2 sin cambiar
  el resto del diseño. No cambia los requisitos de `agent` (el delta ya enumera `Location` como forma
  reconocible), solo cuántas formas cubre la v1.
