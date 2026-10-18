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

1. **`journal-entries` — la bitácora como registro.** Migración, repositorio, herramienta `journal`, endpoints y sección en la app. Al terminarlo, el usuario puede escribir, corregir, listar y borrar entradas; Valet todavía **no** escribe solo.
2. **`journal-worker` — la autonomía.** Worker propio, prompt sembrado y editable, ajustes (encendida o apagada, hora, modelo) y la marca de día juzgado. Al terminarlo, la bitácora se escribe sola.

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
