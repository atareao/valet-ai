# Valet v2 — Bitácora autónoma

## Objetivo

Que Valet mantenga una bitácora personal **por su cuenta**: que decida *cuándo* hay algo que merezca quedar y *qué* escribir, sin preguntar y sin formularios. Es la sustitución del apartado §2 de `plans/PLAN-004-PENDING.md` —que diseñaba un cuestionario a las 20:00— por un **juicio autónomo del propio asistente**, decidido el 2026-10-18.

## Qué se descarta del diseño original

- La pregunta diaria «¿Cómo fue tu día?» y el worker que la hacía (`JournalPrompt`).
- El resumen semanal (`JournalDigest`) y la herramienta de «insights»: fuera de la v1, y candidatos naturales al segundo paso.
- El campo `weather_context`: si el tiempo del día importa, saldrá en el relato.

## Decisiones

- **D1 — Un worker propio, una vez al día.** No cuelga de la pasada episódica: despierta con su propio ciclo y actúa cuando el día que le toca ya se ha cerrado. La hora es configurable; por defecto, de madrugada. Así una conversación de última hora nunca se queda fuera.
- **D2 — El día es el del usuario.** La zona horaria ya está en los ajustes (`timezone`, la que el navegador guarda en cada turno). Los mensajes y los hechos se guardan en UTC, así que la ventana del día se calcula en local y se convierte. Si el ajuste falta o no es válido, se cae a UTC.
- **D3 — Qué mira para decidir.** Los mensajes de ese día (la textura real), los hechos del timeline de ese día (el esqueleto, que es lo que apuntaba `PLAN-005`) y **la entrada anterior** (para seguir el hilo y no repetirse). Con tope de tokens, como el lote de la memoria.
- **D4 — Una entrada por día, y puede no haberla.** Una fila por perfil y fecha. «Esa jornada no tiene nada que contar» es una respuesta **válida y definitiva**: se anota que el día ya se juzgó para no volver a preguntárselo en cada vuelta del ciclo.
- **D5 — El relato manda sobre los hechos.** El prompt pide lectura —cómo fue, qué costó, qué ilusiona—, no un recuento. El timeline ya escribe qué pasó; si la entrada repite eso, la feature sobra.
- **D6 — Nace encendida.** Con ajuste para apagarla, para cambiar la hora y para cambiar el modelo.
- **D7 — Se lee en la app, y también por el agente.** Sección propia en la interfaz (entradas por fecha, con ánimo, editables y borrables) y herramienta para que el agente lea, apunte lo que se le pide, corrija y borre.
- **D8 — El modelo es configuración, no código.** Variable de entorno propia, como los otros roles mecánicos, y **sin sembrar modelos que razonan** en las plantillas (lección de `llm-worker-resilience`, 2026-10-18).
- **D9 — Tabla propia `journal_entries`**, no notas con categoría `journal`: hace falta fecha única por día, ánimo y estructura para poder leer tendencias.
- **D10 — La herramienta entra en la skill existente de notas**, sin estrenar una skill nueva. El enrutado se mide con el arnés del repositorio antes y después de tocar sus criterios.

## Reparto en dos changes

1. **`journal-entries` — la bitácora como registro.** ✅ **Completado** (PR #215). Migración, repositorio, herramienta `journal`, endpoints y sección en la app. Al terminarlo, el usuario puede escribir, corregir, listar y borrar entradas; Valet todavía **no** escribe solo.
2. **`journal-worker` — la autonomía.** ✅ **Completado** (PR #218). Worker propio, prompt sembrado y editable, ajustes (encendida o apagada, hora, modelo) y la marca de día juzgado. Al terminarlo, la bitácora se escribe sola.

El orden no es casual: la autonomía necesita la tabla y el repositorio del primero, y el primero ya entrega algo utilizable aunque el segundo se aparque.

## Riesgos

- **Redundancia con el timeline.** Es el riesgo que puede hundir la feature: una bitácora que solo cuenta hechos es un duplicado del calendario. Se vigila con el prompt y revisando entradas reales al cerrar el segundo change.
- **Ánimo inferido a la ligera.** El modelo puede leer mal un día. La entrada describe en vez de diagnosticar, va silenciosa y se puede corregir.
- **Coste.** Una llamada al día, más una por cada día pendiente después de un parón. El modelo por defecto es uno de los baratos de los roles mecánicos.
- **La frontera del día.** Cambios de hora y zonas raras: el cálculo del día local se prueba a conciencia, y sin ajuste válido se usa UTC.

## Definición de terminado

- La bitácora se escribe sola cuando hay algo que contar, y **no escribe nada** cuando no lo hay.
- Lo que escribe es relato, no un recuento de hechos ya presentes en el timeline.
- El usuario puede leerla en la app, corregirla y borrarla, y el agente puede hacer lo mismo cuando se le pide.
- Nada escribe sobre el usuario sin que él pueda verlo y deshacerlo.
- El enrutado de skills no empeora con la herramienta nueva, medido con el arnés.
- Todo con tests y con su verificación en CLI, como el resto del repositorio.

## Observaciones de revisión aceptadas

Al cerrar `journal-entries`, los dos revisores dejaron hallazgos **no bloqueantes** que se aceptaron sin acción, y quedan aquí para que no se redescubran:

- La validación estricta de fecha está duplicada en la herramienta del agente y en el handler de la API (misma regla, distinto tipo de error). Se acepta: deduplicarla cruzaría dos capas que hoy no comparten nada.
- La vista memoriza de menos (`handleSave`/`handleDelete` se recrean en cada render). Se acepta: con el volumen de un diario no hay problema medible.
- El hook de la vista replica el patrón de `useEvents`, con su misma debilidad ante el desmontaje. Se acepta por coherencia con el molde del repositorio.
- El test de carga diferida comprueba la forma del fuente (`lazy` + `Suspense`) y no el paquete construido; la prueba de verdad de que la vista no entra en el paquete inicial es el comprobador de presupuesto del `postbuild`. El test se queda como guarda barata.
- La tabla no lleva `CHECK` en `mood`: la estrictez vive en el tipo y en el repositorio, que rechaza un valor fuera del conjunto incluso si entra por SQL a mano. Hay test.
- La sincronía `frontend/dist` → `static/` sigue siendo **manual** (no hay receta en el `.justfile`) y ya ha hecho falta otra vez al cerrar este change.

Del cierre de `journal-worker` (PR #218), la misma revisión dejó tres recomendaciones **no bloqueantes** y un apunte del revisor de frontend, aceptados sin acción:

- **La lectura del material puede fallar sin marcar el día.** Si falla la *lectura* (p. ej. la consulta del material), el día no se marca en ese momento: se reintenta en la vuelta siguiente. No es un bucle —la marca impide el de los fallos del modelo—, pero un error persistente dejaría el día colgado hasta que sane.
- **El aviso de `mood` fuera del conjunto no tiene aserción de test.** El aviso que registra un `mood` que no está en el conjunto no está asertado por ningún test; el comportamiento sí lo está (el relato se escribe sin ánimo). Se acepta.
- **El shim de test que reconstruye `llm_requests` a mano es redundante.** Desde la migración que ensancha `kind`, ese shim no hace falta; se acepta sin reescribir los tests a estas alturas.
- **El `aria-label` de grupo de la pestaña «Prompts».** Con la sexta sub-pestaña añadida, la pestaña de Prompts sigue sin `aria-label` de grupo, como las demás del patrón preexistente. Se tocaría en otro change, junto con la accesibilidad de la pestaña entera.

Gotchas operativos que deja la **medición** de `journal-worker` (2026-10-18), para no redescubrirlos:

- **Remedir un día exige borrar `journal_last_judged_day`.** Además de sembrar material hay que borrar la marca: una vuelta sin material marca el día al instante y lo saca del alcance del worker.
- **`JOURNAL_MODEL` no está fijado en el despliegue de trabajo.** El worker usa `MEMORY_MODEL`, que de todos modos es el modelo sin razonamiento por defecto de las plantillas.
