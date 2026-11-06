# Proposal: La pregunta cerrada de sí/no acaba en un widget de elección

## Why

La proyección determinista reconoce hoy tres formas en el texto final del asistente —opciones, lista de tareas
y lugar— y **ninguna cubre una pregunta de sí/no**. Las dos ramas de opciones que podrían acercarse exigen
**dos o más ítems escritos** (`question_options_block` descarta la tanda si tiene menos de dos) y una
**interrogativa de la familia «¿cuál…?»** (`is_interrogative` solo acepta `cuál`/`cual`, por decisión escrita:
«a plain yes/no question leaves a procedure as a procedure»). Así, un turno que termina en «¿Hacemos esto o
no?» o «¿Te parece bien?» —las formas con las que el agente pide una decisión— se queda sin widget y `widgets`
persiste como `null`, de modo que hay que escribir la respuesta a mano.

La exclusión original era prudente (evitaba convertir en formulario cualquier pregunta retórica), pero es
demasiado conservadora para el caso con el que el agente pide permiso o confirmación a diario. Lo detectó el
usuario probando la aplicación desplegada: el resto de las proyecciones «funciona perfecto», y solo estas
preguntas se quedan sin widget.

## What Changes

- La proyección SHALL reconocer una **cuarta forma**: la **pregunta cerrada de sí/no**, y SHALL proyectarla como
  un widget de elección con las dos opciones fijas `Sí` y `No`.
- La forma SHALL reconocerse **solo** cuando la **última línea con contenido** del turno sea una pregunta
  cerrada: SHALL terminar en `?`, SHALL NOT llevar ningún interrogativo abierto (`qué`, `cuál`, `quién`,
  `cómo`, `cuándo`, `dónde`, `cuánto`, `por qué`) y SHALL NOT coordinar dos alternativas **nombradas** con `o`
  (salvo `o no` y `o sí`).
- La forma SHALL NOT reconocerse dentro de un bloque de código, ni cuando la pregunta no cierre el turno, ni
  cuando el turno ya haya proyectado un widget de **opciones**.
- La **pregunta SHALL viajar dentro del widget** como etiqueta del grupo de opciones (`label`) y el widget
  SHALL NOT llevar `title`: el `span` **sustituye** el texto que ocupa, así que sin la pregunta dentro del
  widget esta desaparecería de la pantalla. El `span` SHALL delimitar la línea de la pregunta.
- Sin cambios en el frontend: el widget SHALL emitirse con el mismo contrato `QuickForm` de opciones en `radio`
  que ya consume el cliente.

## Capabilities

### New Capabilities

(ninguna)

### Modified Capabilities

- `orchestrator/agent`: el requisito «El orquestador SHALL proyectar widgets deterministas al cierre del turno»
  gana la cuarta forma reconocible, sus reglas de precisión y sus escenarios.

## Impact

- Código: `src/orchestrator/widget_projection.rs` (detección de la forma, presentación nueva y su síntesis).
- Sin cambios en el frontend, en el esquema de base de datos ni en la API.
- Sin migración y sin tocar la guía de widgets (`SKILL_WIDGETS_PROMPT`): el disparador es el texto, no el modelo.
- Comportamiento observable: los turnos que terminan en una pregunta de sí/no pasan a mostrar un widget de
  `Sí`/`No` en el sitio de la pregunta. El resto de formas no cambia: con un `Location` o una `Checklist` en el
  turno **y** una pregunta de cierre se proyectan **dos** widgets, en tramos distintos. Solo se revierte **una**
  no-detección heredada: un procedimiento numerado cerrado por una pregunta de sí/no deja de no proyectar nada y
  pasa a proyectar la elección (el procedimiento en sí sigue sin ser opciones). Es el caso que motivó el change.
