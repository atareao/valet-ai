# Design — widget-options-contract

## D1. El disparador se queda en el servidor; lo que cambia es la lectura

**Medido:** la pista que el servidor usa para decidir «esto son opciones» es una palabra del propio modelo
(la cabecera `| Opción | … |`) y acertó 20 de 20 veces. La alternativa —que el modelo decida llamar a
`render_widget`— está medida y descartada dos veces (0 de 4 turnos el 1-nov; 0 de 39 turnos con la guía
imperativa del 2-nov). Conclusión: no se toca el disparador, se arregla la lectura.

## D2. El texto de la opción es la fila entera

Regla única, sin casos especiales: las celdas con contenido de la fila, en orden, unidas por ` — `. Se
descartó «solo la columna de texto» y «solo la etiqueta con la columna contigua» porque el widget **sustituye**
la tabla: cualquier celda que no entre en la opción desaparece de la pantalla. Una fila sin ninguna celda con
texto no aporta opción; si quedan menos de dos opciones, no hay presentación.

## D3. Una columna numérica es un índice

Los números consecutivos son un ordinal (vueltas, splits, avisos); las letras son una elección. La rama
heurística (sin cabecera que nombre opciones) exige etiquetas alfabéticas. Coste aceptado y documentado: una
tabla de alternativas numeradas `1,2,3` sin cabecera `Opción` deja de proyectarse. La rama de cabecera
nombrada sigue cubriendo `| Opción | … |` con etiquetas numéricas.

## D4. El contrato en la guía

El modelo ya escribe `| Opción | … |` por su cuenta, así que el arreglo **no depende** de la guía: la guía
solo refuerza y elimina la contradicción actual (la guía vigente **prohíbe** justo la tabla que el servidor
lee). Texto exacto que la migración SHALL sembrar:

> # Cómo presentar alternativas
>
> Cuando tu respuesta ofrezca alternativas para elegir —dos o más opciones, un «¿cuál prefieres?»— escríbelas
> como una **tabla Markdown con cabecera de opciones**, una fila por alternativa, con la etiqueta y el texto
> en la misma fila:
>
> | Opción | Acción |
> |---|---|
> | **A** | **Marcar los 38 como leídos** — bandeja a cero |
> | **B** | **Marcar solo los 4 de IA** que ya hemos visto |
>
> El servidor convierte esa tabla en un formulario de elección: no lo anuncies, no pidas permiso y no repitas
> la lista en prosa. Escribe solo la frase que la acompaña.
>
> Las listas de cosas que marcar (tareas, avisos, correos) y un punto en el mapa también los proyecta el
> servidor a partir de tu texto.
>
> Para lo que el servidor no puede inferir —pedir **varios datos** en un mismo formulario, un **plan con
> pasos** o un mapa cuyos datos no aparezcan escritos— usa la herramienta `render_widget`, que sigue
> disponible.

**Guardas de la migración.** Los textos anteriores conocidos son dos: (a) el recordatorio corto sembrado por
`20261102000001_widget_projection.sql`; (b) el texto imperativo de 3.812 caracteres que la producción tiene
hoy (la guarda byte a byte de aquella migración no disparó porque el usuario lo había editado). La migración
lleva **una guarda byte a byte por texto conocido** y no pisa nada que no reconozca: un texto editado por el
usuario es contenido suyo. El texto (b) se extrae de la base de datos de producción, no se transcribe a mano.

## D5. Alternativas a la vista (`radio`)

Un desplegable esconde el texto de la alternativa, que es justo lo que el usuario necesita leer. El widget
proyectado emite `fields[0].type = "radio"` con `options` = las alternativas y `label` = la etiqueta del
campo. El tipo `radio` se añade al frontend (antd `Radio.Group`) y **no** se ofrece al modelo en el esquema de
`render_widget`: la superficie del modelo no cambia, el productor del `radio` es la proyección. Un `type`
desconocido sigue tratándose como texto (comportamiento vigente).

## D6. Fuera de alcance y por qué

- **Limpiar el histórico** (`messages.widgets` de las 22 filas proyectadas): el usuario decidió que se quede.
- **Envío en blanco del formulario** (pasó a las 05:58:11): es una mejora de frontend independiente; con las
  alternativas a la vista deja de ser el caso probable.
- **El título del widget**: la cabecera en prosa («### ¿Qué hacemos?») queda fuera del `span`; incluirla
  cambiaría el cálculo de tramos.
- **El esquema del widget, el payload, el evento SSE y el `span`**: sin cambios.

## D7. Riesgos y evidencias

| Riesgo | Mitigación / evidencia |
|:---|:---|
| El modelo no obedece el contrato | El arreglo no depende de él: escribió la tabla con cabecera `Opción` en 20 de 20 casos medidos |
| La unión de celdas alarga la opción | El texto íntegro queda en el payload del `submit` y el `radio` lo muestra entero |
| Regresión silenciosa en los tests | Los tests nuevos usan fixtures literales de producción y fijan el **texto** de cada opción (lección del test ciego `test_2_6`, que solo comprobaba la longitud) |
| Guarda de la migración que no dispara | Se verificó el valor real de producción y se añade una guarda por cada texto conocido; si ninguno coincide, la migración no pisa nada y el arreglo sigue funcionando |
