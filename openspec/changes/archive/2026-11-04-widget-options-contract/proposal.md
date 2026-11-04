# Change: El widget de opciones SHALL leerse del texto completo de cada fila, y la guía SHALL declarar la forma que el servidor sabe leer

## Why

La proyección determinista de widgets (v0.26.0/v0.27.0) se midió el 2026-11-04 contra la base de datos real
de producción: de 23 `QuickForm` proyectados desde el despliegue, **20 mostraron como alternativas las letras
`A`, `B`, `C`, `D`** —la etiqueta de la columna `Opción`— en vez del texto de la alternativa; **2** se
proyectaron sobre tablas de datos (`| Lap | Dist. | Ritmo | Pulso |`, `| Split | Ritmo | Desnivel |`); **1**
salió correcto. Como el `span` delimita la tabla y el frontend sustituye ese tramo por el widget, la tabla con
las descripciones desaparece de la pantalla. El fallo rompe la interacción: el turno del 2026-11-04T05:58:11
envió `{"choice":""}` porque el usuario no podía saber qué era cada letra.

Y no se puede esperar a que el modelo lo arregle: con la guía más imperativa posible (3.812 caracteres,
«Tienes la herramienta `render_widget`. Úsala», con autocomprobación obligatoria) midió **0 llamadas en 39
turnos**, mientras 22 de esos turnos sí llamaron a otras herramientas. El modelo no cambia su forma de
escribir (tablas Markdown) por instrucción; el disparador del servidor, en cambio, acertó el 100 % de las
veces que el modelo declaró opciones con la cabecera `| Opción | … |`.

## What Changes

1. El texto de cada opción de una tabla proyectada SHALL ser el de su **fila completa** (todas las celdas con
   contenido, en el orden de la tabla, unidas por ` — `, etiqueta incluida), nunca una sola celda. **BREAKING**
   para las opciones de tabla: dejan de ser el contenido de una celda.
2. Una tabla SHALL leerse como opciones solo si las **declara**: cabecera que nombre la columna de opciones
   (`Opción`/`Alternativa`/`Option`) o primera columna con **letras** consecutivas. Una columna de **números**
   consecutivos SHALL leerse como índice y SHALL NOT producir opciones. **BREAKING**: las tablas de
   alternativas numeradas sin cabecera que nombre opciones dejan de proyectarse.
3. La guía `SKILL_WIDGETS_PROMPT` SHALL **declarar la forma** (tabla con cabecera de opciones, una fila por
   alternativa, etiqueta y texto en la misma fila) en lugar de prohibir la tabla, y SHALL mantener
   `render_widget` para lo que el servidor no puede inferir. Una migración SHALL sustituir, byte a byte y solo
   si la reconoce, cada texto anterior conocido.
4. El widget de opciones proyectado SHALL mostrar las alternativas **a la vista** mediante un tipo de campo
   `radio`, con la etiqueta del campo como pregunta, en vez de un desplegable.

## Capabilities

### New Capabilities

(ninguna)

### Modified Capabilities

- `orchestrator/agent`: el requisito de proyección determinista de widgets cambia la forma de leer las tablas
  de opciones (fila completa, solo opciones declaradas, columna numérica = índice).
- `orchestrator/skill-router`: el fragmento `SKILL_WIDGETS_PROMPT` deja de ser un recordatorio que solo remite
  a `render_widget` y pasa a declarar la forma que el servidor sabe leer.
- `db/schema`: la migración de la guía deja de ser «aditiva y no pisa nada» para pasar a sustituir el texto
  reconocido.
- `frontend`: el widget `QuickForm` admite el tipo de campo `radio` (alternativas a la vista).

## Impact

- Código: `src/orchestrator/widget_projection.rs` (detector y síntesis), `frontend/src/components/widgets/`
  (`types.ts`, `QuickFormWidget.tsx`) y una migración de datos nueva.
- Datos: `settings.SKILL_WIDGETS_PROMPT` (solo si su valor es uno de los textos reconocidos). No se toca
  `messages.widgets` de las filas ya persistidas (decisión explícita del usuario).
- No cambia: el esquema de `render_widget`, la forma del payload del widget, el evento SSE `widget`, el
  cálculo del `span` ni la composición del mensaje en el frontend.
