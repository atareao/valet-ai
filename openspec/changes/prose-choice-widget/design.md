# Design — prose-choice-widget

## Context

Ver `proposal.md` — Why y Evidencia de producción. La proyección determinista vive en
`src/orchestrator/widget_projection.rs` y es una función pura. La cuarta forma (`BinaryChoice`) se añadió en
`binary-choice-widget` (v0.28.0) y nunca ha disparado en producción.

## Goals / Non-Goals

**Goals:**

- Que la elección de cierre reconozca la prosa real del modelo: firma con emoji y pregunta de dos caminos.
- Mantener la proyección pura y verificable con tests unitarios.
- Fijar la realidad con un corpus de regresión extraído de producción.

**Non-Goals:**

- No se toca el frontend ni el contrato `QuickForm`.
- No se toca la guía de widgets (`SKILL_WIDGETS_PROMPT`): el disparador es el texto.
- No se reconocen preguntas de tres o más caminos (siguen sin widget).

## Decisions

### D1. Normalización antes del gate (F1)

El modelo firma con emoji después del `?` (`…? ☕️`, `…? 🎩`). El gate `ends_with('?')` falla siempre. Se
normaliza la línea en este orden: `strip_line_prefix` → `strip_markdown_emphasis` → recorte de los caracteres
finales que no sean alfanuméricos ni `?`/`¿`. El recorte es **solo al final**: no toca el interior de la línea ni
el `span`, que se calcula sobre el texto original.

Alternativa descartada: buscar el último `?` de la línea. Sería más laxo y aceptaría una pregunta seguida de
prosa (`…? Confirme y sale disparado.`), que es justo lo que F3 resuelve de forma controlada.

### D2. Dos caminos como dos opciones (F2)

La forma real es «¿X o Y?», no «¿X o no?». Un `Sí`/`No` sería una respuesta falsa. Se parte por **la**
conjunción `o`/`ó`/`u` a nivel de palabra y se emiten los dos tramos como opciones. Si la palabra siguiente a la
conjunción es `no`/`sí`/`si`, se mantiene la forma binaria `Sí`/`No`.

Guardas para no degradar elecciones de tres o más caminos ni tramos degenerados:

- más de una conjunción `o`/`ó`/`u` → no se proyecta («¿Prefieres A, B o C?»);
- algún tramo vacío o con menos de 3 caracteres alfanuméricos → no se proyecta;
- algún tramo de más de 160 caracteres → no se proyecta.

Limpieza de cada tramo: recorte, quita guiones/rayas y comas, `;` y `:` finales, quita el `¿` inicial y el `?`
final, y
capitaliza la primera letra.

Alternativa descartada: proyectar siempre `Sí`/`No` y dejar que el usuario escriba el camino. Es peor widget que
la prosa y no responde a la pregunta.

### D3. Pregunta no necesariamente última del párrafo (F3)

El modelo escribe `…o se lo deje más sobrio? Confirme y sale disparado. 🎩` (misma línea) y `### 📋 ¿Seguimos?`.
Se busca hacia atrás la última línea con contenido que contenga `?`, **sin cruzar una línea en blanco** (frontera
de párrafo) y sin salir de un bloque de código. Así se reconoce la pregunta de cierre del párrafo final y no una
pregunta retórica del cuerpo.

### D4. `ClosingChoice` sustituye a `BinaryChoice`

`Presentation::ClosingChoice { question, options: Vec<String>, span }` unifica los dos casos: `options` es
`["Sí","No"]` en el binario o los dos caminos en «¿X o Y?». El payload sigue siendo `QuickForm` con
`fields[0].type = "radio"`, la pregunta viaja como `label` del grupo y el widget no lleva `title` (el `span`
sustituye el texto). Sin cambios en el frontend.

### D5. Precedencia sin cambios

Si el turno ya produjo opciones (tabla/lista), no se proyecta elección. Si el modelo renderizó un `QuickForm`,
gana el suyo. Los `span` no se solapan.

### D6. Corpus de regresión obligatorio

Los tests del change anterior usaban texto sintético que acababa exactamente en `?` y decía «o no», y pasaron en
verde sin cubrir la realidad. El delta exige cadenas literales de producción (positivas y negativas) además de
los casos sintéticos.

## Risks / Trade-offs

- **Falso positivo en preguntas retóricas del párrafo final** → mitigado por la frontera de párrafo (F3) y por
  las guardas de F2; se prefiere dejar preguntas fuera antes que proyectar un widget que no corresponde.
- **Recorte final demasiado agresivo** (p. ej. `?»`) → se recortan solo caracteres no alfanuméricos ni `?`/`¿`;
  el `?` interior se conserva y el gate sigue exigiéndolo.
- **Tramos de dos caminos con texto raro** → guardas de longitud y de caracteres alfanuméricos mínimos.

## Migration Plan

Sin migración de datos. El cambio es de código y se despliega con el release habitual; rollback = revertir el
commit.
