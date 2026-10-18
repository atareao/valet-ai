## ADDED Requirements

### Requirement: El `JournalWorker` SHALL juzgar un día por vuelta y SHALL NOT juzgar dos veces el mismo día

El `JournalWorker` SHALL correr en su propia tarea de Tokio, con el mismo intervalo de sondeo que la
memoria episódica (`MEMORY_POLL_INTERVAL_MINUTES`) y la misma señal de parada del `WorkerPool`, y SHALL
ser el único responsable de decidir cuándo hay algo que contar: no cuelga de la pasada episódica ni
comparte estado con ella.

En cada vuelta SHALL procesar **como mucho un día**: el **día cerrado más antiguo que aún no se haya
juzgado**. El objetivo SHALL calcularse comparando el último día juzgado con el último día cerrado, de
modo que un parón de varios días se recupere **día a día** (una vuelta por día) y ningún día se
procese dos veces. Cuando no haya ningún día pendiente, la vuelta SHALL terminar sin llamar al modelo y
sin escribir nada.

El progreso SHALL vivir en el ajuste `journal_last_judged_day` (una fecha ISO `YYYY-MM-DD`), que el
worker SHALL escribir **para todos los desenlaces del día**: la entrada escrita, la respuesta «nada
que contar», el día sin material y el fallo agotado del modelo. Un día juzgado SHALL NOT volver a
juzgarse nunca, sea cual sea el desenlace que lo marcó. El ajuste SHALL NOT sembrarlo ninguna
migración: su ausencia significa «nunca se ha juzgado nada».

En su **primer arranque** (sin marca de progreso), el worker SHALL empezar por el **último día
cerrado** y SHALL NOT recorrer la historia hacia atrás.

#### Scenario: Sin marca de progreso, el primer objetivo es el último día cerrado

- **Given** un `settings` sin `journal_last_judged_day` y sin ninguna entrada en la bitácora
- **When** el worker ejecuta su primera vuelta
- **Then** el día que juzga es el último día cerrado
- **And** NO juzga ningún día anterior a ese

#### Scenario: Un día ya juzgado no se vuelve a juzgar

- **Given** un worker que ya juzgó el día `'2026-10-18'` y lo dejó marcado en `journal_last_judged_day`
- **When** el worker ejecuta otra vuelta el mismo 19 de octubre
- **Then** no juzga el `'2026-10-18'` otra vez
- **And** no llama al modelo por ese día

#### Scenario: Un parón de varios días se recupera un día por vuelta

- **Given** `journal_last_judged_day = '2026-10-15'` y el último día cerrado es el `'2026-10-18'`
- **When** el worker ejecuta cuatro vueltas seguidas con material los tres días pendientes
- **Then** la primera vuelta juzga el `'2026-10-16'`
- **And** la segunda el `'2026-10-17'`, la tercera el `'2026-10-18'` y la cuarta no juzga nada

#### Scenario: Una vuelta sin nada que juzgar no hace nada

- **Given** el último día cerrado ya juzgado
- **When** el worker ejecuta una vuelta
- **Then** NO llama al modelo
- **And** NO escribe ninguna entrada
- **And** NO toca `journal_last_judged_day`

### Requirement: El día juzgado SHALL calcularse en la zona del usuario y con la hora de corte

Un día `D` SHALL considerarse **cerrado** cuando, en la zona horaria del usuario, ya se ha pasado la
**hora de corte** del día siguiente: con la hora de corte por defecto (4), a las 04:05 del 19 de
octubre el día cerrado es el 18, y a las 03:59 sigue siendo el 17. La zona horaria SHALL salir del
ajuste `timezone` (el que el navegador guarda en cada turno) y, si falta o no es válida, SHALL usarse
UTC. La hora de corte SHALL salir del ajuste `journal_hour`.

Los mensajes y los hechos se guardan en UTC, así que la ventana del día SHALL calcularse **en local** y
convertirse: un mensaje o un hecho pertenece al día `D` si y solo si su instante UTC cae en
`[inicio(D), fin(D))`, donde el inicio es la medianoche local de `D` y el fin la medianoche local de
`D + 1`. La ventana SHALL cubrir el día local completo **aunque dure 23 o 25 horas** por un cambio de
hora.

El cálculo SHALL vivir en **funciones puras** (sin acceso a la base de datos ni al reloj del sistema,
que se recibe como parámetro), para poder probarlo a conciencia:

**Contracts:**

```rust
// src/workers/journal.rs
use chrono::{DateTime, Utc};

/// El día local cerrado más reciente: `D` está cerrado cuando en la zona del
/// usuario ya se ha pasado la hora de corte del día siguiente. Una zona ausente
/// o inválida cae a UTC.
pub fn last_closed_day(now_utc: DateTime<Utc>, timezone: &str, cut_hour: u8) -> String;

/// El día que toca juzgar: el más antiguo posterior a `last_judged`, sin
/// pasarse de `closed_day`. `None` cuando no hay nada que juzgar. Sin marca de
/// progreso, el objetivo es `closed_day` (nunca el principio de los tiempos).
pub fn target_day(last_judged: Option<&str>, closed_day: &str) -> Option<String>;

/// La ventana UTC `[inicio, fin)` de un día local `YYYY-MM-DD`, o `None` si la
/// fecha no encaja en el formato. Una zona ausente o inválida cae a UTC.
pub fn day_window(date: &str, timezone: &str) -> Option<(DateTime<Utc>, DateTime<Utc>)>;
```

Una fecha mal formada SHALL ser `None`, nunca una ventana inventada. Si la medianoche local de un día
no existe o es ambigua (la zona cambia de hora justo entonces), SHALL tomarse el primer instante
válido de ese día local.

#### Scenario: A las 04:05 del 19 de octubre se juzga el 18

- **Given** el instante UTC que corresponde a las `04:05` del `19` de octubre en `Europe/Madrid`
- **And** `journal_hour = 4`
- **When** se calcula el último día cerrado
- **Then** el día cerrado es `'2026-10-18'`

#### Scenario: Antes de la hora de corte el día anterior todavía no está cerrado

- **Given** el instante UTC que corresponde a las `03:59` del `19` de octubre en `Europe/Madrid`
- **And** `journal_hour = 4`
- **When** se calcula el último día cerrado
- **Then** el día cerrado es `'2026-10-17'`

#### Scenario: Con la hora de corte a 0 el día se cierra al empezar el siguiente

- **Given** el instante UTC que corresponde a las `00:00` del `19` de octubre en `Europe/Madrid`
- **And** `journal_hour = 0`
- **When** se calcula el último día cerrado
- **Then** el día cerrado es `'2026-10-18'`

#### Scenario: Una zona ausente o inválida cae a UTC

- **Given** la zona horaria `"Marte/Olympus"` (o vacía)
- **When** se calcula el día cerrado, o la ventana de un día
- **Then** el cálculo se hace en UTC
- **And** no se devuelve ningún error ni una fecha inventada

#### Scenario: Un cambio de hora no descuadra la ventana

- **Given** el día `'2026-10-25'` en `Europe/Madrid`, que por el cambio de hora dura 25 horas
- **When** se calcula su ventana
- **Then** el inicio es la medianoche local del 25 y el fin la del 26
- **And** entre ambos hay 25 horas

#### Scenario: La pertenencia al día se decide en UTC

- **Given** un mensaje con instante UTC `2026-10-18T22:30:00Z` y `timezone = "Europe/Madrid"`
- **When** se decide a qué día pertenece
- **Then** pertenece al día local `'2026-10-19'`
- **And** NO pertenece al `'2026-10-18'`

#### Scenario: Una fecha mal formada no tiene ventana

- **Given** la fecha `"18/10/2026"`
- **When** se calcula su ventana
- **Then** no hay ventana
- **And** NO se inventa una fecha por defecto

#### Scenario: El objetivo es el día siguiente al último juzgado

- **Given** `last_judged = '2026-10-16'` y `closed_day = '2026-10-18'`
- **When** se calcula el objetivo
- **Then** el objetivo es `'2026-10-17'`

#### Scenario: Un día juzgado no deja objetivo

- **Given** `last_judged = '2026-10-18'` y `closed_day = '2026-10-18'`
- **When** se calcula el objetivo
- **Then** no hay objetivo

### Requirement: Los ajustes de la bitácora SHALL gobernar el worker y leerse en cada vuelta

El worker SHALL leer sus ajustes de `settings` **en cada vuelta** (no al arrancar), de modo que
cambiarlos surta efecto sin reiniciar, como los parámetros de generación de los demás workers.

**Contracts:**

| Clave | Tipo | Valor por defecto | Efecto |
|:--|:--|:--|:--|
| `journal_enabled` | `'true'` / `'false'` | ausente ⇒ encendida | `false` apaga la bitácora; cualquier otro valor (ausente, vacío o irreconocible) la deja encendida |
| `journal_hour` | entero `0`–`23` | `4` | hora local a partir de la cual el día anterior está cerrado |
| `journal_prompt` | texto | respaldo mínimo del código | el prompt de sistema de la llamada |
| `journal_last_judged_day` | fecha ISO `YYYY-MM-DD` | ausente ⇒ nunca se ha juzgado | marca de progreso |
| `timezone` | zona IANA | ausente o inválida ⇒ UTC | la zona horaria del usuario |
| `JOURNAL_MODEL` (entorno) | modelo | cae a `MEMORY_MODEL` | el modelo de la llamada |

Un valor irreconocible SHALL caer a su defecto **con un aviso**, nunca a un estado a medias: una hora
sin parsear O FUERA del rango `0`–`23` es la `4`; un prompt ausente o en blanco es el respaldo mínimo;
una marca de progreso ilegible se trata como «nunca se ha juzgado» (el objetivo vuelve a ser el último
día cerrado, no el principio de los tiempos). La bitácora SHALL considerarse apagada **solo** cuando
`journal_enabled` valga `false` (sin distinguir mayúsculas ni espacios alrededor).

Con la bitácora apagada, el worker SHALL NOT llamar al modelo, SHALL NOT escribir ninguna entrada y
SHALL NOT tocar la marca de progreso: al volver a encenderla, el diario continúa por donde iba.

#### Scenario: Apagada no hace nada

- **Given** `journal_enabled = 'false'` y días pendientes con material
- **When** el worker ejecuta una vuelta
- **Then** NO llama al modelo
- **And** NO escribe ninguna entrada
- **And** `journal_last_judged_day` no cambia

#### Scenario: Solo el valor `false` apaga la bitácora

- **Given** `journal_enabled = 'FALSE'` (o `'false '`)
- **When** el worker decide si está encendida
- **Then** la bitácora está apagada

#### Scenario: Un valor irreconocible deja la bitácora encendida

- **Given** `journal_enabled = 'quizá'`
- **When** el worker decide si está encendida
- **Then** la bitácora está encendida
- **And** se registra un aviso

#### Scenario: Una hora inservible cae a 4

- **Given** `journal_hour = '25'` (o `'mediodía'`, o vacío)
- **When** el worker la lee
- **Then** la hora de corte es `4`
- **And** se registra un aviso

#### Scenario: Modelo por defecto cae a MEMORY_MODEL

- **Given** un entorno sin `JOURNAL_MODEL` y con `MEMORY_MODEL` definido
- **When** se construye la configuración
- **Then** el modelo del diario es el valor de `MEMORY_MODEL`

#### Scenario: Modelo personalizado del diario

- **Given** un entorno con `JOURNAL_MODEL` definido
- **When** se construye la configuración
- **Then** el modelo del diario es ese valor

#### Scenario: Un prompt ausente usa el respaldo mínimo

- **Given** un `settings` sin `journal_prompt` (o con la clave vacía)
- **When** el worker construye la llamada
- **Then** el prompt de sistema es un respaldo mínimo no vacío
- **And** se registra un aviso

#### Scenario: Una marca de progreso ilegible se trata como «nunca juzgado»

- **Given** `journal_last_judged_day = 'ayer'`
- **When** el worker calcula el objetivo
- **Then** el objetivo es el último día cerrado
- **And** se registra un aviso

#### Scenario: Los ajustes se releen en cada vuelta

- **Given** una primera vuelta con `journal_hour = 4`
- **When** se cambia `journal_hour` a `23` y el worker ejecuta otra vuelta
- **Then** el objetivo se calcula con la hora nueva, sin reiniciar el worker

### Requirement: El worker SHALL juzgar con el material del día y SHALL NOT llamar al modelo sin material

Para decidir qué contar, el worker SHALL reunir el material del día objetivo:

1. **La entrada anterior**: la entrada de la bitácora más reciente **anterior** a ese día, si existe,
   para seguir el hilo y no repetirse.
2. **Los hechos del timeline** de ese día (el esqueleto de lo que pasó).
3. **Los mensajes de ese día** (la textura real), con su rol y su contenido, de más antiguo a más
   reciente.

Un mensaje o un hecho pertenece al día si su instante UTC cae en la ventana de ese día (el cálculo de
la ventana es el requisito anterior). El
bloque SHALL montarse en ese orden y SHALL declarar el día que se está juzgando. Los mensajes SHALL
estar acotados por un tope de tokens (constante con nombre,
`JOURNAL_MATERIAL_TOKEN_BUDGET = 2000`): cuando el día los supere, SHALL tomarse **los más recientes
que quepan**, con el mismo criterio que `MessagesRepo::list_by_token_budget`. La entrada anterior sola
SHALL NOT contar como material.

**Sin material no hay llamada al modelo**: si ese día no tiene **ni mensajes ni hechos**, el worker
SHALL marcar el día como juzgado **sin gastar un céntimo y sin escribir ninguna entrada** en la
bitácora (la marca de progreso sí se escribe, o el día se reexaminaría en cada vuelta).

Cuando sí hay material, la llamada SHALL pedir un **objeto JSON** con este contrato:

```json
{ "write": false }
{ "write": true, "content": "…", "mood": "great|good|neutral|bad|terrible",
  "achievements": ["…"], "struggles": ["…"], "tags": ["…"] }
```

- `write` SHALL ser obligatorio y booleano; cualquier otra forma SHALL ser un fallo de forma.
- Con `write = false` —«esa jornada no tiene nada que contar»— el worker SHALL marcar el día como
  juzgado y SHALL NOT escribir ninguna entrada. Es una respuesta **válida y definitiva**.
- Con `write = true`, `content` SHALL ser el relato, no vacío ni en blanco; un `content` vacío SHALL
  ser un fallo de forma (nunca una entrada en blanco).
- `mood` SHALL ser opcional; un valor fuera del conjunto cerrado (`great`, `good`, `neutral`, `bad`,
  `terrible`) SHALL tratarse como **ausente**, con un aviso, y SHALL NOT impedir que el relato se
  escriba ni SHALL persistirse tal cual.
- `achievements`, `struggles` y `tags` SHALL ser opcionales: un campo que no sea una lista SHALL
  tratarse como lista vacía, y un elemento vacío o no textual SHALL descartarse sin fallar.

La entrada SHALL escribirse con `JournalRepo::upsert` (una sola fila por perfil y día: juzgar un día
escrito no duplica), con la fecha del día **local**, con el perfil por defecto (`ProfilesRepo`), y
SHALL quedar escrita junto a la marca de progreso **en la misma transacción**. La entrada escrita
SHALL ser una entrada ordinaria: SHALL poder leerse y editarse con la herramienta `journal` y con
`GET`/`PUT /api/journal/{date}` como cualquier otra.

#### Scenario: Escribe la entrada de un día con material

- **Given** un día cerrado con mensajes y hechos, y una entrada anterior en la bitácora
- **When** el worker lo juzga y el modelo responde `write = true` con relato y ánimo
- **Then** queda una entrada en `journal_entries` con la fecha de ese día y el relato del modelo
- **And** el día queda marcado en `journal_last_judged_day`

#### Scenario: Sin material no hay llamada ni entrada, pero el día queda juzgado

- **Given** un día cerrado sin mensajes y sin hechos
- **When** el worker lo juzga
- **Then** NO se llama al modelo
- **And** NO se escribe ninguna entrada en `journal_entries`
- **And** el día queda marcado en `journal_last_judged_day`

#### Scenario: La entrada anterior sola no es material

- **Given** un día cerrado sin mensajes ni hechos, con entradas anteriores en la bitácora
- **When** el worker lo juzga
- **Then** NO se llama al modelo
- **And** el día queda marcado

#### Scenario: «Nada que contar» marca el día y no escribe nada

- **Given** un día con material
- **When** el modelo responde `{ "write": false }`
- **Then** NO se escribe ninguna entrada
- **And** el día queda marcado como juzgado
- **And** la llamada queda registrada como cualquier otra

#### Scenario: Un ánimo fuera del conjunto no invalida el relato

- **Given** un día con material
- **When** el modelo responde `write = true` con relato y `mood = "genial"`
- **Then** la entrada se escribe con su relato y sin ánimo
- **And** se registra un aviso

#### Scenario: Un relato en blanco es un fallo de forma

- **Given** un día con material
- **When** el modelo responde `write = true` con `content` vacío o solo espacios
- **Then** NO se escribe ninguna entrada en blanco
- **And** el fallo se trata como cualquier otro fallo de forma

#### Scenario: El bloque lleva los tres materiales y el día

- **Given** un día cerrado con mensajes, hechos y una entrada anterior
- **When** el worker construye el bloque de la llamada
- **Then** el bloque declara el día que se juzga
- **And** incluye los mensajes del día en orden cronológico
- **And** incluye los hechos del timeline de ese día
- **And** incluye la entrada anterior

#### Scenario: El tope de tokens se queda con los mensajes más recientes

- **Given** un día cuyos mensajes superan `JOURNAL_MATERIAL_TOKEN_BUDGET`
- **When** el worker construye el bloque
- **Then** el bloque no pasa del tope
- **And** los mensajes que entran son los más recientes del día

#### Scenario: La entrada se escribe con el perfil por defecto y por upsert

- **Given** un perfil sin entrada para ese día
- **When** el worker juzga el día y escribe
- **Then** la entrada queda con el perfil por defecto
- **When** se vuelve a juzgar el mismo día y se escribe otra vez
- **Then** sigue habiendo una sola fila para ese perfil y día
- **And** su relato es el último escrito

#### Scenario: La entrada escrita se lee y se edita como cualquier otra

- **Given** una entrada escrita por el worker para un día
- **When** se consulta `GET /api/journal/{date}` o `journal.get_entry`, y se edita con `PUT` o
  `journal.update_entry`
- **Then** la entrada se lee y se modifica igual que una escrita a mano
- **And** la herramienta `journal` no necesita ningún cambio para ello

### Requirement: El prompt sembrado de la bitácora SHALL pedir lectura, no un recuento

El valor sembrado de `journal_prompt` SHALL pedir explícitamente una **lectura** del día —cómo fue, qué
costó, qué ilusiona— y SHALL NOT pedir un recuento de lo que pasó: los hechos del día ya los escribe
el timeline, y una entrada que solo los repita duplica la feature. SHALL pedir el relato en primera
persona, como lo contaría el propio usuario, SHALL pedir que **describa en vez de diagnosticar** (el
modelo lee un día y puede leerlo mal) y SHALL NOT permitir inventar hechos que no estén en el material
recibido.

SHALL definir el contrato de salida de la sección anterior: el objeto JSON con `write`, el relato y los
campos opcionales, y la respuesta `{ "write": false }` para la jornada que no tiene nada que contar.
El respaldo mínimo del código SHALL ser coherente con ese contrato.

#### Scenario: El prompt sembrado pide lectura

- **Given** el valor sembrado de `journal_prompt`
- **When** se inspecciona
- **Then** pide contar cómo fue el día, qué costó y qué ilusionó
- **And** pide el relato en primera persona

#### Scenario: El prompt sembrado prohíbe el recuento de hechos

- **Given** el valor sembrado de `journal_prompt`
- **When** se inspecciona
- **Then** dice que los hechos ya están registrados y que no se repitan
- **And** pide describir en vez de diagnosticar

#### Scenario: El prompt sembrado define el contrato de salida

- **Given** el valor sembrado de `journal_prompt`
- **When** se inspecciona
- **Then** describe el objeto JSON con `write`, el relato y los campos opcionales
- **And** define la respuesta `{ "write": false }` para la jornada sin nada que contar

#### Scenario: El respaldo del código es coherente con el contrato

- **Given** el respaldo mínimo del código para un `journal_prompt` ausente
- **When** se inspecciona
- **Then** pide el mismo objeto JSON con `write` y el relato
- **And** no pide una forma distinta de la del prompt sembrado

### Requirement: Un fallo del modelo SHALL reintentarse una vez, marcar el día y SHALL NOT contagiar a los demás workers

La llamada del día SHALL intentarse **como mucho dos veces**. El segundo intento SHALL repetir los
parámetros configurados, salvo cuando el primer intento se haya cortado por presupuesto (motivo de
finalización `length`), en cuyo caso SHALL usar `max_tokens` al **doble** del presupuesto del rol
—el mismo comportamiento que ya tiene la consolidación de la memoria—.

Si el segundo intento también falla (error de transporte, contenido vacío, JSON inválido, contrato
incumplido o relato en blanco), el worker SHALL:

- registrar el fallo como fila `status = 'error'` en `llm_requests` con el diagnóstico (motivo de
  finalización, uso y un preview corto del contenido);
- **marcar el día como juzgado**, para no entrar en el bucle infinito que costó dos días de memoria
  parada: el fallo se ve en las estadísticas y el diario pierde ese día;
- registrarlo como aviso y **seguir vivo**: SHALL NOT abortar, SHALL NOT entrar en pánico y SHALL
  NOT tocar ninguna otra tabla (`messages`, `memory`, `timeline_events`, …).

El worker SHALL poder funcionar con total independencia de los demás: un fallo suyo SHALL NOT
impedir que otro worker procese lo suyo, ni SHALL el worker de la bitácora depender de la pasada
episódica.

#### Scenario: Un truncamiento provoca un reintento con el doble de presupuesto

- **Given** un día con material cuyo primer intento se corta por presupuesto
- **When** el worker reintenta
- **Then** la petición lleva `max_tokens` al doble del presupuesto del rol
- **And** si el reintento devuelve un contrato válido, la entrada se escribe

#### Scenario: Un fallo que no es un truncamiento repite los parámetros

- **Given** un día con material cuyo primer intento devuelve un contrato inválido sin truncarse
- **When** el worker reintenta
- **Then** el reintento usa los parámetros configurados
- **And** no dobla el presupuesto

#### Scenario: Dos fallos dejan la fila honesta y no se reintenta el día

- **Given** un día con material cuyos dos intentos fallan
- **When** el worker gestiona el fallo
- **Then** queda una fila `status = 'error'` en `llm_requests` con el motivo de finalización, el uso
  y un preview del contenido
- **And** el día queda marcado como juzgado
- **And** la vuelta siguiente NO vuelve a llamar al modelo por ese día

#### Scenario: El worker sobrevive al fallo y sigue con el día siguiente

- **Given** un fallo agotado del día `'2026-10-16'` y un día `'2026-10-17'` con material
- **When** el worker ejecuta la vuelta siguiente
- **Then** el worker sigue vivo y juzga el `'2026-10-17'`
- **And** la entrada del día nuevo se escribe con normalidad

#### Scenario: El fallo no toca nada más

- **Given** un día con material cuyos dos intentos fallan
- **When** el worker gestiona el fallo
- **Then** `messages` no cambia
- **And** `memory` no cambia
- **And** `timeline_events` no cambia
- **And** `journal_entries` no gana ninguna fila

### Requirement: Las llamadas de la bitácora SHALL registrarse con `kind = 'journal'` y perfil nulo

Cada intento de la bitácora SHALL registrarse en `llm_requests` como una llamada más: **una fila por
llamada**, con `kind = 'journal'`, `profile_id` NULL (es una operación del sistema, como las de los
demás workers), el modelo resuelto y su uso. Los intentos intermedios SHALL registrarse como
cualquier otra llamada.

La fila SHALL reflejar el resultado de la **validación**, no solo el del transporte: una respuesta que
llegue pero no cumpla el contrato SHALL quedar con `status = 'error'` y su motivo en `error_message`,
con la misma honestidad que la consolidación. Solo una respuesta que cumpla el contrato SHALL quedar
con `status = 'success'`.

El origen SHALL ser visible en la agregación de procesos de fondo: `journal` SHALL ser un origen
canónico más (ver `db/repos`), o sus filas se descartarían en silencio y el fallo del diario no se
vería en ninguna parte.

#### Scenario: Una llamada correcta se registra como éxito

- **Given** un día con material y una respuesta que cumple el contrato
- **When** se registra la petición
- **Then** `llm_requests.kind` es `'journal'`
- **And** `llm_requests.status` es `'success'`
- **And** `llm_requests.profile_id` es `NULL`

#### Scenario: Una respuesta que no valida se registra como error

- **Given** un día con material y una respuesta que llega pero no cumple el contrato
- **When** se registra la petición
- **Then** `llm_requests.status` es `'error'`
- **And** `llm_requests.error_message` lleva el motivo del fallo de validación

#### Scenario: Los dos intentos quedan registrados

- **Given** un día con material cuyos dos intentos fallan
- **When** se registran las peticiones
- **Then** hay dos filas con `kind = 'journal'` para ese día
- **And** las dos llevan su propio uso y su propio estado

#### Scenario: El origen aparece en la agregación de procesos de fondo

- **Given** una llamada con `kind = 'journal'` al mismo modelo
- **When** se agrega el uso de los procesos de fondo
- **Then** el origen `journal` aparece como una pareja (origen, modelo) más
- **And** sus llamadas, tokens y coste se suman como en los demás orígenes

### Requirement: El `WorkerPool` SHALL incluir el `JournalWorker`

El `WorkerPool` SHALL arrancar el `JournalWorker` junto al resto de workers y SHALL detenerlo con la
señal de `shutdown`, siguiendo el mismo patrón que los demás. A diferencia del worker episódico, el
`JournalWorker` SHALL NOT necesitar un proveedor de embeddings: su único requisito es la base de datos
y el proveedor LLM.

#### Scenario: El arranque del pool arranca el worker

- **Given** un `WorkerPool::start()`
- **When** se inicia el pool
- **Then** arranca el `JournalWorker`
- **And** la referencia al worker queda retenida en el pool

#### Scenario: El worker arranca sin proveedor de embeddings

- **Given** un `WorkerPool::start()` con `embedding_provider = None`
- **When** se inicia el pool
- **Then** el `JournalWorker` arranca igual
- **And** solo el worker episódico se queda sin arrancar

#### Scenario: El shutdown detiene el worker

- **Given** un `WorkerPool` en marcha
- **When** se emite la señal de `shutdown`
- **Then** el `JournalWorker` se detiene
