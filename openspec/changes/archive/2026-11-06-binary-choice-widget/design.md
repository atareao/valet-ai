# Design — binary-choice-widget

## D1. El disparador es la pregunta de cierre, no cualquier pregunta

Solo se reconoce la **última línea con contenido** del turno. Una pregunta en medio del texto es casi siempre
retórica (una figura del propio relato) o una cita de lo que preguntó el usuario. La última, en cambio, es la
que pide una decisión: es la posición en la que el agente pide permiso o confirmación.

«Última línea con contenido» significa **la última fuera de los bloques de código**: el autómata de vallas
(`last_content_line_outside_fences`) ignora las líneas encerradas entre ``` ``` ``` o ```~~~```, de modo que un
bloque de código de cierre no oculta una pregunta que lo precede (p. ej. «¿Hacemos esto o no?\n```\ncode\n```»
sigue proyectando la elección binaria).

## D2. Precisión antes que cobertura

Dos exclusiones acotan el falso positivo, que es el riesgo real de esta forma:

- **Interrogativos abiertos** (`qué`, `cuál`, `quién`, `cómo`, `cuándo`, `dónde`, `cuánto`, `por qué`): una
  pregunta abierta no se contesta con sí o no.
- **Alternativas nombradas coordinadas con `o`** («¿Lo hago ahora o mañana?», «¿Por correo o por nota?»): un
  `Sí`/`No` sería una respuesta falsa. Se exceptúan `o no` y `o sí`, que **son** la forma binaria
  («¿Hacemos esto o no?»).

Se prefiere dejar preguntas fuera antes que proyectar un widget que no corresponde a la pregunta. Las preguntas
con alternativas reales siguen el camino ya existente del modelo (declarar las alternativas).

## D3. La pregunta viaja dentro del widget

El `span` **sustituye** el texto que ocupa, no lo decora: en `frontend/src/components/widgets/compose.ts` el
tramo se recorta (`cursor = span.end`) y el widget se inserta en su lugar. Como aquí el bloque **es** la
pregunta, si el widget no la llevara dentro desaparecería de la pantalla. Por eso:

- la pregunta se emite como **etiqueta del grupo de opciones** (`fields[0].label`), que además es lo que el
  `aria-labelledby` del grupo de radios necesita para ser etiquetable;
- el widget **no** lleva `title`, porque hoy la etiqueta se deriva del título (`label = title` cuando el título
  no está vacío) y la pregunta aparecería **duplicada** (una vez como título y otra como etiqueta);
- el `span` delimita exactamente la línea de la pregunta.

## D4. `QuickForm` y `Sí`/`No` fijos

Se reutiliza el contrato `QuickForm` con `fields[0].type = "radio"` y `options = ["Sí", "No"]`. `QuickFormField.label`
ya es obligatorio en los tipos del frontend (`frontend/src/components/widgets/types.ts`), así que **no hay
trabajo de frontend**: ni componente nuevo, ni entrada de registro, ni tests de React.

Coste aceptado: el `QuickForm` se contesta en **dos toques** (elegir y *Enviar*) en lugar de uno. Un widget
binario de un solo toque (dos botones) sería un change posterior si el doble toque resulta molesto.

Las etiquetas son fijas en español, como `DEFAULT_OPTION_LABEL`. No se añade un ajuste: sería superficie de
configuración sin uso real.

## D5. Precedencia y fusión

- Si el turno ya produjo un widget de **opciones** (lista, tabla o etiquetas), la elección binaria **no** se
  proyecta: compiten por el mismo significado y solo se admite una.
- Si el modelo ya renderizó con éxito un `QuickForm`, gana el suyo: es la regla de fusión que ya existe
  (`merge_projected_widgets` descarta la proyección cuyo `name` coincide con un widget del modelo).

## D6. Fuera de alcance y riesgos

**Fuera de alcance:** el frontend; la guía de widgets (`SKILL_WIDGETS_PROMPT`), porque el disparador es el texto
y no el modelo; un ajuste para las etiquetas; y un widget binario de un toque.

**Cambio de comportamiento aceptado:** el test `no_detection_procedure_with_yes_no_question` (heredado del cambio
`widget-projection`) exigía que «1. Abre el fichero\n2. Edita la línea\n¿Quieres que continúe?» no produjera
**nada**. Codificaba la política antigua («una pregunta de sí/no deja un procedimiento como procedimiento»), que
este change revierte para la pregunta de cierre. Pasa a `procedure_with_yes_no_question_projects_only_the_choice`:
los pasos siguen sin ser opciones y la pregunta se convierte en la elección. La spec vigente no respaldaba la
aserción antigua (no había escenario de «procedimiento + sí/no»), así que el vuelco no contradice ningún requisito
preexistente; queda cubierto por el escenario nuevo del delta.

**Riesgos:**

- **Falso positivo** (una pregunta de cierre que no sea de sí/no y acabe con widget). Se acota con D1 y D2 y se
  blinda con los **escenarios negativos** de la spec, que son la mitad del trabajo del change.
- **Pregunta duplicada o perdida en pantalla**. Mitigado por D3, con su escenario propio.
- **Concurrencia con la forma de opciones**. Mitigado por la regla de precedencia de D5.
- **Solapamiento de tramos** con otra forma (`Location`, `Checklist`): un turno con una línea de coordenadas que
  además termina en pregunta generaría dos widgets sobre el mismo `span` y el cliente degradaría uno a prosa.
  Mitigado por el filtro de intersección de `detect`: la elección de cierre solo se proyecta si su `span` no pisa
  el de ninguna presentación ya recogida, lo que preserva el invariante «spans never overlap».
