# Proposal: La elección de cierre reconoce la prosa real del modelo

## Why

El change `binary-choice-widget` (v0.28.0) añadió la proyección de una pregunta cerrada de sí/no, pero **no ha
disparado ni una sola vez en producción**: 0 widgets con `Sí`/`No` en toda la base de datos. El modelo no escribe
`…?` limpio ni pregunta «¿X o no?»: firma con emoji después del `?` y ofrece dos caminos («¿X o Y?»). El gate
`ends_with('?')` y el rechazo de alternativas nombradas lo descartan siempre.

## What Changes

- **F1 — Gate tolerante a la firma.** Antes de comprobar el cierre, la línea se normaliza en este orden:
  `strip_line_prefix` → `strip_markdown_emphasis` → recorte de los caracteres finales que no sean alfanuméricos
  ni `?`/`¿` (emoji, puntuación, cierre de comillas, marcadores Markdown). Después se exige `ends_with('?')`.
- **F2 — Forma «¿X o Y?».** Si la pregunta lleva una conjunción `o`/`ó`/`u` a nivel de palabra y la palabra
  siguiente **no** es `no`/`sí`/`si`, se parte por **la** conjunción y se emiten **dos opciones** con los
  dos tramos. Si la palabra siguiente **sí** es `no`/`sí`/`si`, se mantiene la forma binaria `Sí`/`No`.
- **F3 — Última línea *con* pregunta del párrafo final.** En vez de exigir que la pregunta sea la última línea
  con contenido del turno, se busca hacia atrás la última línea con contenido que contenga `?`, sin cruzar una
  línea en blanco (frontera de párrafo) y sin salir de un bloque de código.
- **Modelo de datos:** `Presentation::BinaryChoice { question, span }` pasa a
  `Presentation::ClosingChoice { question, options: Vec<String>, span }`. El payload sigue siendo `QuickForm`
  con `fields[0].type = "radio"`. Sin cambios en el frontend.
- **Corpus de regresión:** el delta SHALL exigir cadenas literales extraídas de producción, además de los casos
  sintéticos, porque los tests del change anterior usaban texto que acababa exactamente en `?` y decía «o no»,
  y por eso pasaron en verde sin cubrir la realidad.

## Capabilities

### New Capabilities

(ninguna)

### Modified Capabilities

- `orchestrator/agent`: el requisito «El orquestador SHALL proyectar widgets deterministas al cierre del turno»
  generaliza la cuarta forma de «elección binaria» a «elección de cierre», con las tres correcciones (F1, F2, F3)
  y su corpus de regresión.

## Impact

- Código: `src/orchestrator/widget_projection.rs` (normalización de la línea, detección de dos caminos, variante
  `ClosingChoice` y su síntesis).
- Sin cambios en el frontend, en el esquema de base de datos ni en la API.
- Sin migración y sin tocar la guía de widgets (`SKILL_WIDGETS_PROMPT`): el disparador es el texto.
- Comportamiento observable: los turnos que cierran con una pregunta de sí/no **firmada con emoji** pasan a
  mostrar un widget `Sí`/`No`; los que cierran con «¿X o Y?» pasan a mostrar un widget con **los dos caminos**
  como opciones. Las preguntas abiertas y las de tres o más caminos siguen sin widget.

## Evidencia de producción

Medición sobre los **últimos 200 turnos** del asistente en la base de datos real de producción:

| Métrica | Valor |
|:---|:---:|
| Widgets con `Sí`/`No` en toda la BD | **0** |
| Turnos que disparan la regla actual (`ends_with('?')`) | **8 / 200** |
| Turnos que dispararían ignorando el emoji final | **17 / 200** |
| Turnos que cierran con pregunta (techo real) | **66 / 200** |
| Turnos que cierran con pregunta de dos caminos «¿X o Y?» | **33 / 200** |
| Ocurrencias de «o no?» en toda la BD | **0** |

Cierres reales que hoy se pierden: `…o va directo a algo concreto? ☕️`, `…? 😒☕️`, `…? 🎩`. La forma «¿X o no?»
que el change anterior tomó por típica **no existe** en producción.

Medición contra 200 turnos reales de producción: 41 turnos (20,5 %) pasan a llevar widget de elección de cierre (9 binarios, 32 de dos caminos), frente a 0 antes del change.

El 41 % restante de turnos con QuickForm son tablas de opciones que ya se proyectaban antes de este change.
