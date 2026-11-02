# PLAN-013 — El servidor proyecta los widgets, el modelo solo aporta contenido

**Estado:** **aprobado** por el usuario el 2026-11-02; el TDD arranca en la sesión siguiente.
**Fecha:** 2026-11-02
**Origen:** el cierre fallido de `PLAN-012` (`widgets-always-on`): su batería de aceptación (tarea 6.3)
dio **0/3 widgets** con la guía nueva y la herramienta expuesta en el 100 % de los turnos.
**Change de OpenSpec:** `openspec/changes/widget-projection/` (artefactos completos y validados; sin código).

---

## 1. Objetivo

Que un turno cuya respuesta **presenta** una lista de opciones, una lista de tareas o un lugar acabe
con un widget, **sin depender de que el modelo decida renderizarlo**. El widget deja de ser algo que el
modelo *decide renderizar* y pasa a ser algo que el servidor *proyecta*: el modelo aporta contenido, el
servidor decide presentación.

## 2. Por qué se abandona la vía del prompt

`PLAN-012` apostó por **inclinar la decisión del modelo**: exponerle `render_widget` en todos los turnos
y reescribir la guía en prescriptivo, nombrando los tres widgets y anclando el disparador en
comportamiento observable. La apuesta falló, y no por poco: con la guía nueva perseverante, la batería
dio **0/3** sobre la conversación real y **1/3** sobre una conversación vacía.

El diagnóstico de `PLAN-012` (§2) ya nombraba la causa dominante —«el modelo no llama a la herramienta
aunque la tenga delante»— y la causa de fondo —«la decisión se toma antes de que exista la respuesta»—.
Lo que faltaba era entender **por qué** el prompt no basta. La evidencia del 2-nov lo responde.

## 3. Evidencia medida (2-nov-2026, toda verificada)

### 3.1 La guía está viva y no basta

- `settings.SKILL_WIDGETS_PROMPT` mide **3.812 caracteres** (la guía nueva, reescrita en prescriptivo).
- `render_widget` figura en `exposed_tools` en el **100 %** de los turnos.
- Aun así, la batería de 3 prompts reales da **0/3 widgets**.

### 3.2 Experimento de aislamiento (misma imagen `valet-ai:v0.25.0`, mismos 3 prompts)

Contenedor aparte con una copia de la BD de producción:

| Escenario | Resultado |
|:---|:---|
| Conversación vacía + guía vieja (2.704 car.) | **0/3** (tabla Markdown de 9 tareas) |
| Conversación vacía + guía nueva perseverante (3.812 car.) | **1/3** — el único prompt cuya respuesta es genuinamente una lista renderizó un `Checklist` con las tareas **reales**; los otros dos no renderizaron widget **y era correcto** (un dato único y una explicación, los dos casos que la propia guía excluye) |
| Conversación real de producción (995 mensajes; **21 de los últimos 30 mensajes de asistente son listas de opciones en prosa**) + guía nueva | **0/3**, los tres en **tablas Markdown** |

**Conclusión:** el modelo **imita su propio precedente** y una instrucción de prompt no le gana. El
modelo es `deepseek/deepseek-v4.1-flash`, la petición va con `tool_choice: auto`, y el prompt base
(4.687 car.) *ordena* usar «listas y tablas», se autodeclara prioritario y trae **tres ejemplos, los
tres del patrón «herramienta → prosa», y cero de widget**.

### 3.3 Encuesta de industria (2-nov-2026)

Ningún producto serio deja que el modelo decida «texto o interfaz»:

- **ChatGPT Apps SDK / Vercel AI SDK «Generative UI» / mcp-ui**: la UI es **propiedad del resultado de
  la herramienta** («a widget is simply a React component»; «the tool call renders a React component»).
  Se llamó al tool ⇒ hay widget.
- **MCP elicitation** (revisión de julio de 2026): el **servidor** manda un esquema y el **cliente** lo
  renderiza como formulario y devuelve valores tipados. Lo inicia el servidor: el modelo no tiene
  discreción.
- **Slack Block Kit / Discord embeds / Telegram inline keyboards**: la app compone la interfaz; el
  modelo aporta texto como mucho.

Nadie resuelve esto con prompting. Valet es el único que pide al modelo *elegir presentar* —y, a
diferencia de los demás, **sí controla el pipeline** que produce la respuesta. Detalle útil: incluso en
el ecosistema del Apps SDK hay hilos de foro quejándose de que **el modelo repite en prosa lo que el
widget ya muestra**; ellos no pueden arreglarlo porque no controlan el pipeline. Valet **sí**.

### 3.4 Cómo lo resuelve opencode (medido el 2-nov-2026 sobre su base local)

Se auditó la implementación real de **opencode v2.2.5** en la máquina del usuario (`opencode.db`, 3 GB,
7.088 sesiones) porque es el agente que el propio usuario usa a diario. **No proyecta widgets**: su
solución es la del Apps SDK, y los números dicen tres cosas.

| Hallazgo | Dato |
|:---|:---|
| El mensaje del asistente es un **array ordenado de bloques** (`reasoning` / `text` / `tool`) dentro de la fila del mensaje | `tool` 277.476 · `reasoning` 192.266 · `text` 95.379 |
| El orden **es** la composición: cada bloque se pinta donde va | orden observado `reasoning → text → tool → tool → …` |
| El contenido se guarda **verbatim**, sin reescribir y sin campo derivado | claves del mensaje: `agent`, `model`, `content`, `finish`, `cost`, `tokens`, `time`, `error`, `snapshot` |
| Las tablas de partes (`part`, `session_part`) están **vacías**: el contenido vive embebido en la fila | 0 filas en ambas |
| La UI la define el **esquema de la herramienta**, no la prosa: `question` recibe `{questions:[{question, header, options:[{label, description}]}]}` y devuelve `{answers:[[…]]}` | 1.348 llamadas (814 en 30 días) |
| Aun así la prosa filtra: hay listas de opciones escritas a mano en bloques de texto | 1.753 |

**Conclusiones que se incorporan al diseño:**

1. **`content` verbatim** (regla 1 de la referencia). Recortar el texto queda descartado también por
   comparación: el líder de la industria no reescribe el registro del modelo.
2. **La composición vive en la fila del mensaje, no en una tabla aparte**: la referencia se movió *desde* una
   tabla de partes *hacia* un array embebido.
3. **El widget en su sitio**, vía el orden de los bloques. Valet **sintetiza** el bloque en vez de recibirlo
   autorado, así que ese orden se expresa con un **tramo** (`span`) sobre el texto.
4. **Nada de confiar en que el modelo use la herramienta**: opencode tiene un `question` dedicado, bloqueante,
   con el modelo ya en «modo herramienta» (2,9 llamadas a herramienta por cada bloque de texto) y **aun así**
   escribe 1.753 listas en prosa. Valet, cuyo turno **termina** en prosa, midió 0/3. Copiar la forma sin ese
   contexto sería repetir el error del prompt.

## 4. El giro

**Principio:** el widget deja de ser algo que el modelo *decide renderizar* y pasa a ser algo que el
servidor *proyecta*. El modelo aporta contenido; el servidor decide presentación.

### 4.1 Módulo nuevo puro `src/orchestrator/widget_projection.rs` (sin I/O, sin async)

- `detect(text: &str) -> Vec<Presentation>` — detección determinista sobre el texto final del
  asistente; devuelve **todos** los bloques reconocidos, en orden de aparición y con tramos que no
  se solapan:
  - `Presentation::Options { title, options: Vec<String>, span }`: listas de opciones etiquetadas (`A)`,
    `A.`, `A:`, `1)`, `1.`, `Opción A`), o alternativas en viñetas cerradas por una interrogativa del tipo
    «¿cuál…?».
  - `Presentation::Checklist { title, items, span }`: tablas Markdown cuya primera columna es un índice o
    una casilla, o listas de viñetas con forma de tarea (cosas que marcar).
  - `Presentation::Location { .., span }`: una latitud/longitud (la forma «dirección» queda aplazada, D1).
  - **Lista vacía**: todo lo demás (ningún bloque reconocido).
  - **`span` es el tramo que el bloque ocupa**: rango semiabierto `[start, end)` en bytes UTF-8 sobre el
    texto.
- `synthesize(presentations, tool_results) -> Vec<Widget>` — construye el payload (`{ id, name, data, span }`).
  **Orden de preferencia para los datos:** si el turno llamó a una herramienta que produjo la lista
  (`tasks::list_tasks`, `tasks::search_tasks`, `email_list_unread`, `rss_list_unread`,
  `notifications::list_unread`), los ítems salen del `data` de esa herramienta (precisión, sin parsear
  prosa); si no, salen del texto.
- **No hay `strip`**: el texto se persiste íntegro y el widget se coloca por su `span`.

### 4.2 Integración en el turno (`src/orchestrator/agent.rs`, al cierre y **antes** de persistir)

El punto exacto ya existe: el turno ensambla `content_buffer`, recoge `rendered_widgets` (de las
llamadas del modelo a `render_widget`) y persiste con `MessagesRepo::set_widgets` justo antes de
`publish_persisted_message` (sobre la línea 1777; el evento SSE `widget` se emite en vivo sobre la 1663
cuando `render_widget` tiene éxito). Al cierre: `detect` sobre `content_buffer` → `synthesize` →
fusionar con `rendered_widgets` (deduplicar por nombre de widget + payload) → `set_widgets` → emitir
el/los evento(s) SSE `widget` **antes** de `done`.

### 4.3 `render_widget` sigue funcionando y gana

Si el modelo ya renderizó un widget de ese tipo, la proyección para ese tipo se omite (el payload del
modelo es más rico). La herramienta queda como **vía explícita de override**.

### 4.4 El texto se guarda íntegro; el widget declara su tramo

El `content_buffer` se persiste **verbatim** —`content` alimenta a la vez el render del frontend, el contexto
que ve el modelo (`effective_content = COALESCE(collapsed_content, content)`) y el índice FTS—, y cada widget
proyectado lleva un `span` con el tramo que ocupa. El cliente compone: pinta los tramos de texto y, en el
sitio del tramo, el widget. Sin `span` (widgets del modelo y todas las filas anteriores) el widget se pinta
debajo, como hoy: **sin migración y sin backfill**. El evento `done` lleva el contenido íntegro en `content`,
que es la base autoritativa contra la que se calculan los `span`.

### 4.5 `render_widget` y la guía se DEGRADAN, no se eliminan

La herramienta se queda como vía explícita; la guía se recorta a un recordatorio breve, porque ya no
carga el peso. Efecto lateral medido: la guía actual (3.812 car. ≈ 1.000 tokens) viaja en **todos** los
turnos; recortarla ahorra ese coste por turno.

### 4.6 Tests

El proyector es puro ⇒ tests unitarios de **cada detección y, sobre todo, de cada NO detección** (los
falsos positivos son el riesgo real: «widgets de más»). Más un test end-to-end del camino del agente.
Esto cierra el agujero que `PLAN-012` declaró explícitamente: «la guía no tiene test unitario».

## 5. Decisiones (resumen; el detalle está en `design.md`)

- **D1 — Alcance v1.** Recomendado: `Options` + `Checklist` (y `Location` solo si sale barato). La
  alternativa (solo `Options`) se descarta porque deja fuera el único caso que la guía nueva logró y el
  patrón más común de producción (listas de tareas).
- **D2 — Composición.** **Decidido (2026-11-02): opción (c)** — el contenido se persiste **íntegro** y cada
  widget proyectado declara el tramo que ocupa (`span`), de modo que el cliente lo coloca **en su sitio**.
  Descartada (a) recortar el texto: pierde la lista para el FTS, deja al modelo sin su historial y borra el
  dato con el que se depurarían los recortes. Descartada (b) columna `display_content`: es una copia que hay
  que mantener coherente y no dice dónde iba el widget.
- **D3 — De dónde salen los datos.** Recomendado: del resultado de la herramienta cuando exista, del
  texto cuando no. El riesgo de «widgets de más» se acota con el **disparador en el texto** (solo se
  proyecta si el texto *presenta* una lista) y con la batería de NO detección.
- **D4 — Qué hacer con `widgets-always-on`.** Recomendado: archivarlo con `--yes` documentando que su
  batería falló, porque su código está **mergeado y desplegado** (v0.25.0) y sus spec deltas sí están
  verificados; el change nuevo supersede el papel de la guía. **No se ejecuta `openspec archive`**: la
  decisión es del usuario.
- **D5–D7** (añadidas por este diseño): dónde vive el contrato del `done` (en `stream-route`), por qué
  `tools/registry` no cambia, y cómo se recorta la guía (migración quirúrgica que solo pisa lo prístino).
- **D8–D9** (añadidas por este diseño): los structured outputs (`response_format: json_schema`) quedan
  descartados sin medir —garantizan la forma, no el juicio—; y el **destino** del mensaje es el modelo de
  bloques (`text`, `widget` y, después, `image`, `chart`, `citation`), pero esta entrega **no** introduce la
  columna de bloques: el `span` es su forma barata y presente, y la columna se decidirá con el proyector ya
  validado.

## 6. Mapa de la arquitectura tocada

| Pieza | Fichero | Qué cambia |
|:---|:---|:---|
| Proyector puro | `src/orchestrator/widget_projection.rs` | **Nuevo**: `detect` / `synthesize` (con el tramo del bloque) |
| Cierre del turno | `src/orchestrator/agent.rs` | `detect` → `synthesize` → fusión → `set_widgets` → evento `widget` (con `span`) antes de `done`; `SSEEvent::Done` gana `content` y `SSEEvent::Widget` gana un `span` opcional |
| Migración | `migrations/20261102000001_widget_projection.sql` | **Nuevo**: recorta `SKILL_WIDGETS_PROMPT` al recordatorio, solo si sigue prístino. **Sin migración de datos de mensajes** |
| Frontend | `frontend/src/components/widgets/types.ts`, `compose.ts` (nuevo), `MessageBubble.tsx`, `useSSE.ts`, `useMainChat.ts` | El cliente compone el mensaje por tramos y coloca cada widget en su `span` |
| Specs | `orchestrator/agent`, `orchestrator/skill-router`, `db/schema`, `stream-route`, `frontend` | 2 añadidos, 2 modificados, 1 añadido, y el `REMOVED` + `ADDED` de la guía |

**No se toca:** el esquema de `render_widget` (`src/tools/widget.rs`), el registry de widgets del frontend
(`WIDGET_REGISTRY`, `WidgetRenderer`, los tres componentes), el `data` de cada widget, la columna
`messages.widgets` (mismo JSON opaco, con un campo opcional más) ni la persistencia (`set_widgets`). Tampoco
el umbral cero de `widgets` (la guía sigue viajando, solo se recorta).

## 7. Tareas

El detalle vive en `openspec/changes/widget-projection/tasks.md`. Resumen por fases:

1. **Planificación (SDD)** — proposal, design, deltas, validación y aprobación del usuario.
2. **RED del proyector** — cada detección (con su tramo) y, sobre todo, cada NO detección.
3. **GREEN del proyector** — los parsers y la síntesis con el `span`.
4. **RED de la integración** — el cierre del turno, la fusión, el texto íntegro y el `content` del `done`.
5. **GREEN de la integración** — `agent.rs`.
6. **Migración y recordatorio** — el SQL y el texto del recordatorio.
7. **Frontend** — el compositor puro por tramos y el render.
8. **REFACTOR** — clippy, fmt, `just check-all`.
9. **Verificación end-to-end** — determinista (sin modelo) + comprobación sobre el despliegue.

## 8. Criterios de aceptación

- [ ] Un turno cuya respuesta presenta una lista de opciones acaba con `messages.widgets` no vacío,
      **sin** que el modelo llame a `render_widget`.
- [ ] El contenido persistido conserva el texto **íntegro** del modelo, y el widget proyectado declara un
      `span` que delimita exactamente el bloque que representa.
- [ ] El evento `done` lleva el contenido íntegro en `content`, y la UI pinta el widget **en el sitio** de la
      lista, no debajo.
- [ ] Un turno sin forma reconocible deja `widgets` como `null` (sin widgets de más).
- [ ] `render_widget` sigue funcionando y su widget gana sobre la proyección del mismo tipo.
- [ ] `openspec validate --all --strict` sin errores y `just check-all` verde.
- [ ] La verificación del proyector es **determinista** y no depende del modelo.

## 9. Riesgos

| Riesgo | Mitigación |
|:---|:---|
| **Widgets de más** (falso positivo del proyector) | El disparador es el texto: solo se proyecta si `detect` reconoce la forma. Un test por cada NO detección. Ante la duda, `None`. |
| **Detección frágil** ante formatos inesperados | El falso negativo es benigno (queda la prosa). Cada variante es un test. |
| **Aritmética de tramos en el cliente** (el `span` se corta mal) | La composición es una función pura con tests de los casos límite; el `content` del `done` es la base autoritativa de los `span`; y ante cualquier `span` inválido se cae al render de hoy (texto íntegro + widgets debajo). |
| **Un `span` que se desvía por una normalización del servidor** | El tramo se calcula sobre el **mismo** texto que se persiste y que viaja en el `content` del `done`; el cliente compone contra ese valor, no contra su texto acumulado. Test de que ambos coinciden. |
| **La fusión duplica un widget** | Deduplicación por nombre + payload; `render_widget` gana por tipo. Test unitario de la fusión. |
| **Recortar la guía** reintroduce el problema si la proyección falla | `render_widget` sigue expuesto y funcionando; la proyección no sustituye al modelo donde el modelo es mejor. |
| **La migración de recorte es byte a byte** | Se verifica el texto de producción antes de escribirla; si el usuario editó la guía, no se pisa. |
| **Orden de archive**: si `widgets-always-on` se archiva **después** de este change, su delta reescribiría el requisito de la guía con el texto extenso | Archivar `widgets-always-on` **antes** (D4). Si no se archiva, este change no depende de ello para funcionar. |

## 10. Relevo para la sesión siguiente (2-nov-2026)

**Estado:** los artefactos del change están **completos y validados** (`openspec validate --all --strict`:
63 passed, 0 failed; `just check-spec`: verde), **actualizados a la opción (c)** de D2 —contenido íntegro +
`span` en el widget— y **aprobados** por el usuario el 2026-11-02. **Sin código, sin migración y sin commits
del código**: solo los artefactos del change.

1. **Aprobación explícita del change**: **otorgada el 2026-11-02** (tarea 1.7 marcada). El protocolo SDD
   queda cumplido y el TDD puede empezar.
2. **Abrir sesión nueva para el TDD** (higiene: esta sesión acumuló el diseño completo, la auditoría de
   opencode y la reescritura de artefactos). El estado está en ficheros, no en la conversación.
3. **RED → GREEN → REFACTOR** con TDD, delegando la escritura en los subagentes y auditando con
   `@rust-reviewer` (backend) y `@react-reviewer` (frontend). Empezar por el proyector puro y sus NO
   detecciones; seguir por la integración en `agent.rs`; después la migración y el frontend.
4. **Decidir D1** (`Location` en la v1) durante el RED, según el coste del detector.
5. **Rama y commits** (gitflow + gitmoji; nunca directamente a `main` ni a `development`):
   `git checkout development && git pull origin development` y luego
   `git checkout -b feature/widget-projection`. El grueso es un `feat` (minor) del orquestador; conviene
   separar la migración, el frontend y los tests.
6. **Release y despliegue** en `co1` por Dockge, y las verificaciones 10.4–10.5 de `tasks.md`.

**Nota de higiene:** no usar `route-eval --threshold` para validar este change (el arnés repone defaults y
mediría algo distinto de lo desplegado). La verificación del proyector es determinista y no depende del
modelo.
