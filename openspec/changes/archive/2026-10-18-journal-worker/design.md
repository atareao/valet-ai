# Design: La bitácora autónoma

## Context

`plans/PLAN-007.md` parte la bitácora en dos changes: `journal-entries` (la mitad registrable:
migración, modelo, repositorio, herramienta, endpoints y sección en la app — archivado el 2026-10-18,
PR #215) y `journal-worker` (la autonomía). Este change es el segundo: no crea almacén, **usa** el que
ya existe.

Lo que condiciona el diseño:

- **El worker episódico es el molde**: tarea propia, `tokio::time::interval`, señal de `shutdown` por
  broadcast, `evaluate()` idempotente, lecturas de `settings` en caliente y tests con un proveedor LLM
  simulado.
- **`llm-worker-resilience` (archivado el 2026-10-18) dejó hechas las piezas del fallo**: degradar en
  vez de abortar, reintento con presupuesto doblado cuando la respuesta se trunca, y la fila de
  `llm_requests` con el diagnóstico (motivo de finalización, uso y preview). Este change las
  **aprovecha**; no las reinventa.
- **`episodic-call-integrity` (archivado el 2026-10-12)** fijó el idioma de siembra de prompts: una
  migración que rellena la clave ausente o vacía y **nunca** pisa un valor presente.
- **El timeline (`PLAN-005`, `activity-timeline`) es el esqueleto del día**: hechos atómicos con
  fecha, hasta 280 caracteres cada uno. La bitácora no lo reescribe ni lo consulta por similitud: lee
  los hechos de ese día y los usa como andamio de una **lectura**, no como texto a repetir.
- **La unidad de la bitácora es el día, no el lote.** Los demás workers procesan un lote de mensajes;
  este juzga una jornada. De ahí que no cuelgue de la pasada episódica.
- **La zona horaria ya está en `settings.timezone`** (la guarda el navegador en cada turno, hoy
  `Europe/Madrid`) y los mensajes y los hechos viven en UTC.

## Goals / Non-Goals

**Goals**

- Que la bitácora se escriba sola cuando hay algo que contar y **no escriba nada** cuando no lo hay.
- Que lo que escriba sea **relato**, no un recuento de los hechos que ya están en el timeline.
- Que un parón de varios días se recupere solo, día a día, y que ningún día se juzgue dos veces.
- Que el fallo del modelo sea **visible** (fila honesta en `llm_requests`), **acotado** (ese día se
  pierde) y **no contamine** a ningún otro worker.

**Non-Goals**

- No se tocan la herramienta `journal`, la skill `recuerdos`, el timeline ni la memoria.
- No hay resumen semanal, «insights», avisos por push, clima ni vistas nuevas.
- No se estrena un rol de generación (ver D11): el modelo del diario es configuración por entorno. Sus
  dos ajustes de comportamiento (`journal_enabled` y `journal_hour`) se leen y escriben por
  `GET`/`PUT /api/settings` como el resto, sin pestaña propia; el prompt, en cambio, se afina desde la
  pestaña «Prompts» que ya existe (D15).
- No se toca `docker-compose.prod.yml`.

## Decisions

**D1 — Un worker propio, `JournalWorker`, que actúa una vez al día.**
No cuelga de la pasada episódica porque su unidad no es el lote: es la jornada, y el lote se indexa
cuando se procesa (atar el diario a esa marca obligaría a juzgar los días por donde va la memoria, no
por donde va el calendario). Tiene su propio ciclo, con el intervalo de sondeo que ya existe para la
memoria (`MEMORY_POLL_INTERVAL_MINUTES`), y su propia señal de parada. El ciclo es barato: no llama al
modelo salvo que haya **un día que juzgar con material**. En cada vuelta procesa **el día cerrado más
antiguo que aún no se haya juzgado**, de modo que un parón de varios días se recupera día a día y
nunca se procesa dos veces. El precio es que un día puede esperar hasta un intervalo de sondeo para
ser juzgado: treinta minutos de retraso en un diario no significan nada.

**D2 — El «día» es el del usuario.**
La zona sale de `settings.timezone`; si falta o no es válida, se cae a UTC. Un día `D` está cerrado
cuando en local ya se ha pasado la **hora de corte** del día siguiente, y esa hora también es un
ajuste (`journal_hour`, entero `0`–`23`, por defecto `4`): a las 04:05 del 19 de octubre se juzga el
18, de modo que una conversación de la noche —o de la madrugada— entra en su día natural. Los mensajes
y los hechos se guardan en UTC, así que la ventana se calcula en local y se convierte: pertenece al
día `D` lo que caiga en `[medianoche(D), medianoche(D+1))`, que en un cambio de hora son 23 o 25
horas. **El cálculo vive en funciones puras** (`last_closed_day`, `target_day`, `day_window`), sin
base de datos ni reloj del sistema —el instante entra como parámetro—, para poder probar el corte, la
zona inválida y los cambios de hora sin dormir ni depender de la máquina.

**D3 — Qué mira para decidir.**
Los **mensajes de ese día** (la textura real), los **hechos del timeline de ese día** (el esqueleto,
que es justo lo que apuntaba `PLAN-005`) y **la entrada anterior** (para seguir el hilo y no
repetirse), con un tope de tokens como el lote de la memoria. **Sin material no hay llamada al
modelo**: si ese día no tiene ni mensajes ni hechos, se marca como juzgado sin gastar un céntimo. La
entrada anterior sola no es material: un día vacío es un día vacío, aunque el día anterior sí tenga
entrada.

**D4 — Qué escribe.**
Una entrada de la tabla que ya existe, por `JournalRepo::upsert` (una sola fila por perfil y día, la
misma puerta que usa la herramienta del agente): relato y, opcionalmente, ánimo, logros, dificultades
y etiquetas. **Y puede no escribir nada**: «esa jornada no tiene nada que contar» es una respuesta
válida y definitiva, no un fallo. Sin esa salida, el modelo tendría que rellenar todos los días con
texto de relleno, que es exactamente lo contrario de una bitácora.

**D5 — La marca de día juzgado.**
El progreso vive en un ajuste (`journal_last_judged_day`, una fecha ISO: el último día juzgado) y no
en una tabla nueva: es un dato de estado, no de dominio. En su primera vuelta el worker **no** empieza
por el principio de los tiempos —su primer objetivo es el último día cerrado—, o el primer despliegue
dispararía decenas de llamadas de golpe. Un día juzgado **no se vuelve a juzgar nunca**, ni siquiera
cuando la respuesta fue «nada que contar»: de ahí que la marca se escriba en **los cuatro desenlaces**
(entrada, nada que contar, sin material, fallo agotado).

**D6 — El relato manda sobre los hechos.**
El prompt se siembra en los ajustes y es **editable** (`journal_prompt`), con el mismo patrón que los
otros prompts del repositorio: una migración que lo siembra solo si la clave está ausente o vacía,
para no pisar un prompt editado —la lección de `episodic-call-integrity`—. Pide **lectura** —cómo fue,
qué costó, qué ilusiona—, no un recuento: el timeline ya escribe qué pasó, y una entrada que solo
repita hechos duplica la feature. El prompt también pide describir en vez de diagnosticar (el modelo
lee un día y puede leerlo mal) y prohíbe inventar lo que no esté en el material.

**D7 — Nace encendida, con ajuste para apagarla.**
`journal_enabled` (por defecto encendida: solo el valor `false` la apaga), `journal_hour` (por defecto
`4`) y el modelo por **variable de entorno propia** (`JOURNAL_MODEL`, que cae a `MEMORY_MODEL` cuando
no está definida, como `TIMELINE_MODEL`). En las plantillas (`.env.j2` y `.env.example`) se siembra un
modelo **sin razonamiento**, porque un modelo que razona se come el presupuesto de tokens y devuelve
contenido vacío sin error: la lección medida de `llm-worker-resilience`.

**D8 — Cuando el modelo falla.**
Se reintenta **una vez**, y con **presupuesto doblado** si el corte fue por techo (`finish_reason =
length`), como ya sabe hacer la consolidación. Si vuelve a fallar: se registra la fila honesta en
`llm_requests` (`status='error'` con el motivo de finalización, el uso y un preview) y **el día se
marca como juzgado** —el fallo se ve en las estadísticas y el diario pierde ese día— para no entrar en
el bucle infinito que costó dos días de memoria parada. El fallo queda **confinado**: ni toca otras
tablas, ni aborta el worker, ni puede afectar a ningún otro worker (tareas separadas, sin estado
compartido y sin más canal que la señal de parada).

**D9 — Lo que NO hace.**
Ni resumen semanal, ni «insights», ni avisos por push, ni clima, ni vistas nuevas. Todo eso queda como
lo que es: candidatos a un segundo paso, no trabajo de este change.

## Decisiones tomadas al escribir este change

Estas no venían fijadas por `PLAN-007.md`, pero hacen falta para que el change sea coherente. Se
presentan al usuario para que las pueda vetar antes de implementar (tarea 0.2).

**D10 — El origen `journal` entra en las listas cerradas que lo hacen visible.**
La fila honesta de D8 necesita un origen propio: `kind = 'journal'`. `llm_requests.kind` tiene un
`CHECK`, así que hace falta una migración que reconstruya la tabla para admitirlo (preservando todas
las filas, como se hizo con `'timeline'`). Y con el origen dentro tiene que entrar también en las
listas cerradas que gobiernan lo que la aplicación *muestra*: la agregación de procesos de fondo
(`db/repos`) descarta en silencio cualquier `kind` que no sea canónico, así que sin registrarlo la
fila existiría y **nadie la vería**, que es justo lo contrario de lo que pide D8; y su etiqueta en la
tarjeta («Bitácora»), con la misma lógica. Es el mismo cableado que `timeline-observability-wiring`
tuvo que hacer después para el timeline, hecho ahora en el mismo change para no dejar la feature
invisible.

**D11 — El diario reutiliza el rol de generación mecánico; no estrena uno.**
`PLAN-007.md` fija el **modelo** por entorno y no habla de un rol de generación propio. Se descarta
estrenar `GENERATION_JOURNAL_*` (con su siembra y su fila en la pestaña «Generación»): el diario es una
tarea mecánica de escritura, como la ficha del archivista, así que la llamada lee sus parámetros del
rol `GENERATION_MEMORY_*` (temperatura `0.3`, razonamiento `off`, `1024` tokens), que es el que ya
tienen las tareas mecánicas de escritura. Si el relato pide otra temperatura, un rol propio es un
change aparte, igual que lo fue el del timeline.

**D12 — «No escribir nada» no incluye la marca de progreso.**
D3 dice que un día sin material «se marca como juzgado sin gastar un céntimo y sin escribir nada». La
marca **sí** se escribe: es lo único que impide que ese día se reexamine en cada vuelta para siempre.
Lo que no se escribe es **ninguna entrada** en la bitácora, ni fila de error, ni llamada al modelo.

**D13 — El tope de material es una constante con nombre, no un ajuste nuevo.**
`JOURNAL_MATERIAL_TOKEN_BUDGET = 2000` acota los mensajes del día, con el mismo criterio que
`MessagesRepo::list_by_token_budget`: si el día los supera, entran los **más recientes** que quepan.
Los hechos del día y la entrada anterior no llevan tope propio: los hechos son atómicos y ya están
acotados a 280 caracteres cada uno, y la entrada anterior es una sola. Se descarta convertirlo en
ajuste: el diseño fija tres claves nuevas y ninguna es esta.

**D14 — El relato va en primera persona.**
El prompt pide el relato **en primera persona, como lo contaría el propio usuario**: la bitácora es su
diario, la sección de la app es suya y las entradas se leen como suyas. Es una decisión de redacción,
revisable editando el prompt (D6 no se rompe: el valor sembrado puede cambiarse).

**D15 — El prompt se edita desde Ajustes, no solo por API.**
`journal_prompt` **entra en la pestaña «Prompts» del diálogo de Ajustes como sexta sub-pestaña**, con
el mismo patrón que las cinco que ya existen (*System*, *Archivist*, *Collapse*, *Consolidator* y
*Timeline*, cableadas a mano en `SettingsDialog.tsx`): carga desde `GET /settings`, un `TextArea` y
guardado con el resto de prompts por `PUT /settings`. Corrige la decisión preliminar que lo dejaba
fuera: **el prompt es donde vive o muere el «relato en vez de recuento»** (D6, y el riesgo que puede
hundir la feature), así que poder afinarlo sin `curl` es **parte de la capacidad**, no un extra. El
resto de las claves de la bitácora (`journal_enabled` y `journal_hour`) siguen sin interfaz propia:
no se estrena una pestaña de ajustes de la bitácora, solo un campo en la que ya existe.

## Risks / Trade-offs

- **Redundancia con el timeline: el riesgo que puede hundir la feature.** Una bitácora que solo cuenta
  hechos es un duplicado del calendario. Se acota con el prompt (D6), que prohíbe el recuento, y se
  **verifica con entradas reales** al cerrar el change (tarea de cierre), como manda `PLAN-007.md`. La
  redundancia no es verificable con un test unitario: se mira.
- **El relato es más caro que un recuento.** Una llamada por día, más una por cada día pendiente
  después de un parón; y el reintento de un truncamiento cuesta otra llamada al doble de presupuesto.
  Con un modelo barato y una llamada diaria, el coste es marginal, pero un parón largo de varios días
  se paga de golpe (una llamada por día pendiente, en vueltas separadas).
- **Un fallo del modelo pierde ese día del diario.** Es deliberado (D8): entre un día perdido, visible
  en las estadísticas, y un bucle infinito que no avanza, el día perdido es el mal menor. Un fallo
  corregido a mano (rejuzgar el día) exige borrar la marca de progreso: no hay interfaz para eso.
- **La frontera del día.** Zonas raras y cambios de hora: el cálculo se prueba a conciencia (incluido
  el día de 25 horas y la pertenencia en UTC) y sin ajuste válido se usa UTC. Un día local puede durar
  23 o 25 horas; la ventana lo respeta.
- **La reconstrucción de `llm_requests`.** Ampliar el `CHECK` obliga a reconstruir la tabla más grande
  de la base (registro de todas las llamadas LLM). Se hace una vez, preservando todas las filas, y es
  el precio de que el origen nuevo sea insertable; se descarta dejarlo fuera porque sin origen propio
  D8 no se cumple.
- **El ánimo inferido a la ligera.** El modelo puede leer mal un día. Por eso la entrada **describe**
  en vez de diagnosticar, va silenciosa (sin avisos) y se puede corregir y borrar como cualquier otra.
- **La marca de progreso es estado en `settings`.** Un ajuste pensado para valores de configuración
  guarda ahora un dato de avance. Se acepta porque es un único valor, cabe en el idioma existente
  (`get`/`set`) y no merece tabla propia; si algún día hace falta por perfil, se migra.

## Migration Plan

Una migración nueva (`migrations/20261018000002_journal_worker.sql`), aditiva en la intención y con
dos bloques:

1. **Ampliar el `CHECK` de `llm_requests.kind` para admitir `'journal'`.** SQLite no permite alterar un
   `CHECK` en línea: la migración reconstruye la tabla (`CREATE TABLE ... _new` → `INSERT INTO ... _new
   SELECT ...` → `DROP TABLE` → `RENAME`) conservando **todas** las filas y sus valores, y recreando
   los índices. Es el mismo procedimiento que usó `activity-timeline` para `'timeline'`.
2. **Sembrar los ajustes de la bitácora.** `journal_prompt` (el prompt del relato), `journal_enabled`
   (`'true'`) y `journal_hour` (`'4'`), con el idioma de `20261010000004_timeline_prompts.sql`:
   rellenar solo si la clave está ausente o vacía, nunca pisar un valor presente. `journal_last_judged_day`
   **no** se siembra: su ausencia es el estado «nunca se ha juzgado nada».

**Rollback:** revertir el commit. Sin el código, la clave nueva no molesta a nadie, el `CHECK` más
ancho no invalida ninguna fila anterior y las tres claves sembradas son inertes. No hay datos que
deshacer: `journal_entries` nace vacía y este change no la toca.

**Despliegue:** el habitual del despliegue de trabajo (`just dev`, `podman compose up -d --build`),
que aplica las migraciones al arrancar. Tras el despliegue, la verificación de cierre mira entradas
reales (tarea 8.6).

## Open Questions

- ¿El worker debería rejuzgar un día cuando el usuario **edita o borra** su entrada a mano? Hoy no:
  la marca manda y la edición humana es definitiva. Si al usar la feature aparece la necesidad, será
  otra decisión (posible ajuste «rehacer el día»).
- ¿Conviene un límite de días pendientes por arranque (p. ej. recuperar como mucho 7 y esperar) para
  que un parón largo no genere una ráfaga de llamadas? Hoy no se pone: cada vuelta procesa un día y el
  ritmo lo marca el intervalo de sondeo.
