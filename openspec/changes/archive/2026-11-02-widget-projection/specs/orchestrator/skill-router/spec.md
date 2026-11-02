## REMOVED Requirements

### Requirement: La guía de widgets SHALL residir en el fragmento de su skill

## ADDED Requirements

### Requirement: El fragmento de la skill widgets SHALL ser un recordatorio breve que remita a render_widget

El fragmento `settings.SKILL_WIDGETS_PROMPT` SHALL NOT residir de forma permanente en
`settings.system_prompt`. La migración `20261008000001` SHALL retirar ese bloque del prompt base
**solo si se encuentra verbatim** (identificado por su encabezado) y SHALL depositarlo como contenido de
`SKILL_WIDGETS_PROMPT`; si el bloque no se encuentra verbatim, SHALL NOT retirar nada y el fragmento
SHALL sembrarse igualmente. La regla anti-duplicado del ensamblador de fragmentos SHALL seguir vigente
como red de seguridad. Como la skill `widgets` tiene umbral cero, la guía SHALL viajar en el mensaje de
sistema de **todos** los turnos, también con el enrutador caído o apagado. Con la skill `widgets`
**deshabilitada** SHALL NOT enviarse ni el esquema de la herramienta ni la guía.

El fragmento SHALL ser un **recordatorio breve**, NO el mecanismo que decide la presentación. El
servidor proyecta los widgets de forma determinista al cerrar el turno (ver `orchestrator/agent`), así
que la guía ya no carga el peso de convencer al modelo: SHALL limitarse a recordar que `render_widget`
sigue disponible como **vía explícita de override** y que el servidor puede haber proyectado ya un
widget. La guía SHALL NOT enumerar los tres nombres de `widget_name` con ejemplos de `data`, ni
enunciar la regla de las opciones como una instrucción que el modelo deba aplicar: esa decisión ya no
es suya.

La migración `20261102000001_widget_projection.sql` SHALL sustituir la guía extensa sembrada por
`20261101000001_widgets_always_on.sql` por ese recordatorio **solo si su valor sigue siendo byte a
byte** el texto sembrado; si el usuario lo ha editado, SHALL NOT pisarlo.

El texto exacto del recordatorio SHALL ser:

> Los widgets los proyecta el servidor a partir de tu respuesta: si presentas una lista de opciones,
> una lista de tareas o un lugar, aparecerá un widget. No lo anuncies ni repitas la lista en prosa;
> escribe solo la prosa que lo acompaña. Usa `render_widget` únicamente si necesitas un widget con
> datos más ricos de los que el servidor puede inferir.

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
