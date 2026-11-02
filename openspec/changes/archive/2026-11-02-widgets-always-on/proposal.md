# Change: Los widgets, siempre presentes

## Why

El usuario lo dijo sin rodeos: «Quiero que los widgets estuvieran siempre presentes. En las últimas
ocasiones me ofrece varias alternativas, y en lugar de mostrar un widget me dice que elija A, B o C…
y esto como experiencia de usuario es muy frustrante».

La auditoría del despliegue de producción (`co1`, 1-nov-2026) confirma la queja y localiza la causa.
Hechos medidos sobre la base de datos de producción:

- **466 mensajes de asistente, 16 con widget (3,4 %).** Entre el 10 y el 25 de octubre: 315 turnos,
  16 widgets (5,08 %). **Del 26 de octubre a hoy: 118 turnos, cero widgets.** Con la tasa anterior,
  la probabilidad de encadenar 118 turnos sin un solo widget es ≈ 0,2 %: hay una degradación real,
  sobre una base que ya era mala.
- **Los 16 widgets históricos casi nunca fueron automáticos.** Son sobre todo `LocationWidget`
  disparados por peticiones explícitas —«un planito?», «donde está el parking?», «donde esta la
  oficina?»— y el último de todos, el 25-oct a las 09:58, lo provocó el usuario escribiendo
  literalmente `Widget`.
- **La herramienta está expuesta y el modelo no la llama.** Hoy el router seleccionó `widgets` en 4
  de 7 turnos; en esos cuatro, `render_widget` figuraba en `exposed_tools` y hubo **cero**
  invocaciones. En toda la base de datos, `tool_results` no contiene ni una sola llamada a
  `render_widget`.
- **Las instrucciones de widgets ya no están en el prompt base.** La migración
  `20261008000001_skill_router_tuning.sql` movió la guía de `settings.system_prompt` a
  `SKILL_WIDGETS_PROMPT`. El prompt de producción (4.687 caracteres) no menciona `render_widget`,
  `QuickForm`, `Checklist` ni `LocationWidget`: cuando el router dice «no», el modelo no sabe
  siquiera que los widgets existen.
- **El gate es ruidoso.** `widgets` es la única skill con umbral `0.20` (el resto, `0.10`). Las
  probabilidades medidas hoy —`0.10, 0.10, 0.21, 0.26, 0.29, 0.37`— están apiñadas alrededor del
  umbral: la skill se enciende y se apaga casi al azar. Hoy se activó para «marca todo como leído» y
  para un simple «A», y **no** para «ghostty versión 1.4 no está para Arch?».
- **El patrón que frustra al usuario, reproducido hoy dos veces.** «que mas tenemos que despachar?» →
  el asistente responde con una tabla de opciones `A`/`B` y el usuario contesta «B» por texto. Igual
  con «donde utilizo nicegui?» → opciones `A`/`B` → el usuario contesta «A». Eso es exactamente un
  `QuickForm` o un `Checklist` que no se renderiza.

## What Changes

Dos cambios, uno de comportamiento y otro de texto. No se toca ni el esquema de la herramienta ni
el frontend.

1. **La skill `widgets` deja de estar condicionada por el clasificador.** Su umbral por defecto pasa
   de `0.20` a `0`, y **un umbral efectivo de cero SHALL significar «no condicionada»**: la skill se
   selecciona en todos los turnos, incluso cuando el clasificador no devuelve respuesta para ella.
   Consecuencia: `render_widget` figura en el conjunto expuesto y `SKILL_WIDGETS_PROMPT` viaja en el
   mensaje de sistema **en todos los turnos**, también en los caminos de fallo abierto.
2. **La guía de widgets se reescribe en prescriptivo.** La guía actual es genérica, no nombra ni uno
   de los tres widgets y contiene una puerta de salida explícita —«si la respuesta se resuelve con
   una explicación conceptual o un dato directo, NO invoques la herramienta»— que es justo la que
   los datos de producción muestran usándose. La nueva guía nombra `QuickForm`, `Checklist` y
   `LocationWidget` con un ejemplo mínimo de `data` para cada uno, y ancla el disparador en
   comportamiento observable —**si vas a escribir dos o más opciones etiquetadas (A/B/C, 1/2/3) o
   una lista de alternativas, NO las escribas: renderiza un widget**— en lugar de en criterios
   abstractos.
3. **Spec**: se modifican cinco requisitos de `orchestrator/skill-router` y uno de `db/schema`.

## Impact

- **Specs afectadas**: `orchestrator/skill-router` (5 requisitos modificados: el catálogo con el
  umbral del widget, la selección con umbral cero, la inyección de fragmentos, la guía de widgets y
  la configuración de umbrales) y `db/schema` (1 requisito modificado: la sección de widgets termina
  en el fragmento de su skill, no en el prompt base). `skill-router-ui`, `orchestrator/agent` y
  `tools/registry` **no** cambian.
- **Código afectado**: `src/orchestrator/skills.rs` (umbral compilado de `widgets` y el test que lo
  fija), `src/orchestrator/skill_router.rs` (selección con umbral cero y composición de fragmentos)
  y `migrations/20261101000001_widgets_always_on.sql` (nuevo).
- **Fuera de alcance**: el esquema de `render_widget`, el registro de widgets del frontend, la
  persistencia en `messages.widgets` y el evento SSE `widget` no se tocan: el problema medido es que
  la herramienta **no se invoca**, no que se invoque mal.
- **Riesgo**: la herramienta pasa a viajar en todos los turnos, así que el esquema de `data` (el
  bloque `anyOf` con los tres widgets) se envía siempre: del orden de 400–600 tokens más por turno
  en los turnos en que antes no se exponía (~43 % de los turnos medidos). Es el precio de que esté
  siempre presente.
- **Riesgo no cubierto por tests**: el punto 2 es un cambio de prompt y **no se puede verificar con
  tests unitarios**. La evidencia de producción dice que la exposición sola no basta (0 invocaciones
  en 4 turnos expuestos), así que la guía es la pieza que carga el peso. Su verificación es una
  batería medida sobre el despliegue (tarea 6.3).
- **Si el punto 2 no basta**: la red de seguridad es una detección determinista de listas de
  opciones en la respuesta del modelo, que no depende de él. Queda **fuera** de este cambio.
