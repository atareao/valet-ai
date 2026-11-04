## MODIFIED Requirements

### Requirement: El fragmento de la skill widgets SHALL ser un recordatorio breve que remita a render_widget

El fragmento `settings.SKILL_WIDGETS_PROMPT` SHALL NOT residir de forma permanente en
`settings.system_prompt`. La migración `20261008000001` SHALL retirar ese bloque del prompt base **solo si se
encuentra verbatim** (identificado por su encabezado) y SHALL depositarlo como contenido de
`SKILL_WIDGETS_PROMPT`; si el bloque no se encuentra verbatim, SHALL NOT retirar nada y el fragmento SHALL
sembrarse igualmente. La regla anti-duplicado del ensamblador de fragmentos SHALL seguir vigente como red de
seguridad. Como la skill `widgets` tiene umbral cero, la guía SHALL viajar en el mensaje de sistema de
**todos** los turnos, también con el enrutador caído o apagado. Con la skill `widgets` **deshabilitada** SHALL
NOT enviarse ni el esquema de la herramienta ni la guía.

El fragmento SHALL **declarar la forma** que el servidor sabe leer, no decidir la presentación. La proyección
determinista al cerrar el turno (ver `orchestrator/agent`) lee la tabla con cabecera de opciones que el modelo
escribe, de modo que el fragmento SHALL pedir esa forma como **instrucción de formato**: alternativas como
tabla Markdown con cabecera de opciones, una fila por alternativa, con la etiqueta y el texto de la
alternativa en la misma fila. El fragmento SHALL NOT enumerar los tres nombres de `widget_name` con ejemplos
de `data`, ni exponer un catálogo de widgets; SHALL limitarse a esa forma, a pedir que no se anuncie el widget,
no se pida permiso y no se repita la lista en prosa, y a señalar que `render_widget` sigue disponible para lo
que el servidor no puede inferir del texto (varios datos en un mismo formulario, un plan con pasos, un mapa
cuyos datos no aparezcan escritos).

La migración `20261102000001_widget_projection.sql` SHALL sustituir la guía extensa sembrada por
`20261101000001_widgets_always_on.sql` por el recordatorio **solo si su valor sigue siendo byte a byte** el
texto sembrado; si el usuario lo ha editado, SHALL NOT pisarlo.

La migración `20261104000001_widget_options_contract.sql` SHALL sustituir el fragmento por el texto que declara
la forma **solo si su valor es byte a byte uno de los dos textos conocidos**: el recordatorio sembrado por
`20261102000001` o la guía imperativa de 3.812 caracteres que quedó en producción al editarse aquel valor.
Cualquier otro valor es contenido del usuario y SHALL NOT pisarse.

El texto exacto del fragmento SHALL ser:

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

#### Scenario: El bloque verbatim se muda al fragmento
- **Given** un prompt base que contiene el bloque de widgets tal y como lo añadió la migración original
- **When** se aplica la migración
- **Then** el bloque deja de estar en el prompt base
- **And** su contenido queda en `SKILL_WIDGETS_PROMPT`

#### Scenario: Un bloque editado no se retira
- **Given** un prompt base en el que el usuario ha modificado el bloque de widgets
- **When** se aplica la migración
- **Then** el prompt base conserva el texto del usuario
- **And** el fragmento se siembra igualmente
- **And** la regla anti-duplicado impide que la guía viaje dos veces

#### Scenario: La guía viaja en todos los turnos
- **Given** la skill `widgets` habilitada con umbral `0` y un turno cualquiera
- **When** se compone el mensaje de sistema
- **Then** contiene la guía de widgets
- **And** la petición ofrece `render_widget`

#### Scenario: Con la skill inactiva no viaja la guía
- **Given** un turno en el que la skill `widgets` está deshabilitada (`ROUTER_SKILL_WIDGETS_ENABLED` a `false`)
- **When** se compone el mensaje de sistema
- **Then** no contiene la guía de widgets
- **And** la herramienta `render_widget` no figura entre las ofrecidas

#### Scenario: Un fragmento editado por el usuario no se pisa
- **Given** un `SKILL_WIDGETS_PROMPT` con texto del usuario
- **When** se aplica la migración `20261102000001_widget_projection.sql`
- **Then** el valor del usuario se conserva

#### Scenario: La migración es idempotente
- **Given** una base de datos ya migrada
- **When** se vuelve a aplicar la migración
- **Then** el prompt base y el fragmento quedan igual

#### Scenario: La guía se recorta al recordatorio y no prescribe la regla de las opciones
- **Given** el fragmento `SKILL_WIDGETS_PROMPT` con la guía extensa sembrada por `20261101000001`
- **When** se aplica la migración `20261102000001_widget_projection.sql`
- **Then** el fragmento pasa a ser un recordatorio breve
- **And** NO enumera los tres widgets con ejemplos de `data`
- **And** NO enuncia la regla de las opciones como una instrucción que el modelo deba aplicar
- **And** remite a que `render_widget` sigue disponible como vía explícita

#### Scenario: El fragmento declara la forma que el servidor sabe leer
**Given** el fragmento `SKILL_WIDGETS_PROMPT` con el recordatorio sembrado por `20261102000001`
**When** se aplica la migración `20261104000001_widget_options_contract.sql`
**Then** el fragmento pide escribir las alternativas como tabla Markdown con cabecera de opciones
**And** NO enumera los tres nombres de `widget_name` ni ejemplos de `data`
**And** remite a `render_widget` para lo que el servidor no puede inferir

#### Scenario: Un fragmento editado no se pisa
**Given** un fragmento `SKILL_WIDGETS_PROMPT` con un texto distinto de los dos textos conocidos
**When** se aplica la migración `20261104000001_widget_options_contract.sql`
**Then** el fragmento conserva el texto del usuario
