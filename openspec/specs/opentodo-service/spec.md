# opentodo-service Specification

## Purpose
Cliente HTTP del servicio `Opentodo`: resolución de la configuración en cada llamada (settings → entorno → valor por defecto), errores tipados que nunca filtran el token, las vistas de OpenTodo calculadas en Rust, las etiquetas de estado y ámbito con reemplazo atómico del array, y la idempotencia de todas las escrituras.

## Requirements

### Requirement: El servicio `Opentodo` SHALL resolver su configuración en cada llamada y NO exponer nunca el token

El servicio SHALL resolver la URL base y el token **en cada llamada**: primero de `settings`
(`opentodo_base_url`, `opentodo_token`), después del entorno (`OPENTODO_BASE_URL`,
`OPENTODO_TOKEN`) y, solo para la URL, de un valor por defecto
`https://todo.territoriolinux.es`. Todas las peticiones SHALL enviar `Authorization: Bearer <token>`
y SHALL usar un timeout de 10 segundos. El token NO SHALL aparecer en ningún `Debug`, log ni mensaje
de error.

Los errores SHALL ser tipados y distintos entre sí: `NotConfigured` (no hay token configurado),
`Unauthorized` (`401`/`403`), `NotFound` (`404`), `InvalidInput` (`400`, con el mensaje del servidor)
y `Upstream` (fallo de red o `5xx`). `NotConfigured` NO SHALL confundirse con `Unauthorized`: el
primero indica dónde configurar el token y el segundo que el token no vale.

La deserialización SHALL ser tolerante: todos los campos opcionales de la API SHALL ser `Option` o
`#[serde(default)]`, de modo que una respuesta con campos ausentes (`start`, `deadline`, `evening`,
`labels`, `blocked_by`, `assignee_id`) o con campos desconocidos SHALL deserializarse sin error.

#### Scenario: La configuración sale de settings y cae al entorno
- **Given** `settings` sin `opentodo_token` y la variable `OPENTODO_TOKEN` con valor
- **When** el servicio construye su configuración
- **Then** usa el token del entorno
- **And** la URL base es la de `settings`, o la del entorno, o `https://todo.territoriolinux.es`

#### Scenario: Sin token se distingue de un token inválido
- **Given** ni `settings` ni el entorno con token
- **When** se invoca una operación del servicio
- **Then** devuelve `NotConfigured`
- **And** el mensaje indica que se configure en Ajustes → Integraciones → OpenTodo o en `OPENTODO_TOKEN`
- **And** una respuesta `401` del servidor devuelve `Unauthorized`, no `NotConfigured`

#### Scenario: Ningún error ni debug contiene el token
- **Given** un token configurado con el valor `SECRET`
- **When** se formatea el `Debug` de la configuración y el `Display` de cada variante de error
- **Then** ninguna de esas cadenas contiene `SECRET`

#### Scenario: Una tarea con campos ausentes deserializa
- **Given** una respuesta con una tarea que solo trae `id`, `title` y `priority`
- **When** el servicio la deserializa
- **Then** no hay error y los campos ausentes quedan en `None`

### Requirement: El servicio SHALL recalcular las vistas de OpenTodo porque el servidor no filtra

`GET /api/v1/tasks` SHALL considerarse una lista **sin filtrado**: el servidor solo acepta
`include_deleted` y `completed_after`/`completed_before`, y el parámetro `workspace_id` se ignora. Por
tanto el servicio SHALL calcular en Rust las vistas sobre las tareas recibidas, considerando
**abierta** toda tarea con `completed_at == None`, con estos predicados exactos:

- `inbox` — abierta y `project_id == null`;
- `today` — abierta y (`due.date <= hoy` o `deadline <= hoy`);
- `upcoming` — abierta y `due.date > hoy`, más las abiertas sin `due` cuyo `deadline` sea posterior
  a hoy;
- `anytime` — abierta y sin `due`, sin `deadline` y sin `start`;
- `someday` — abierta y `start == "someday"`;
- `doing` — abierta y con la etiqueta `doing`;
- `waiting` — abierta y con la etiqueta `waiting`, o con `blocked_by` no vacío, o con `assignee_id`
  distinto del usuario autenticado;
- `all` — todas las no completadas.

Los predicados NO SHALL ser mutuamente excluyentes: una tarea PUEDE aparecer en varias vistas. La
vista `next` NO SHALL implementarse en este change.

#### Scenario: Una tarea vencida aparece en hoy
- **Given** una tarea abierta con `due.date` de ayer
- **When** se calcula la vista `today`
- **Then** la tarea está en el resultado

#### Scenario: Las completadas no aparecen en ninguna vista salvo all
- **Given** una tarea con `completed_at` informado
- **When** se calculan `inbox`, `today`, `upcoming`, `anytime`, `someday`, `doing`, `waiting` y `all`
- **Then** no aparece en ninguna de las siete primeras
- **And** tampoco aparece en `all`

#### Scenario: La vista en espera suma etiqueta y bloqueo
- **Given** una tarea abierta con `blocked_by` no vacío y sin la etiqueta `waiting`
- **And** otra tarea abierta con la etiqueta `waiting` y sin `blocked_by`
- **When** se calcula la vista `waiting`
- **Then** las dos aparecen

#### Scenario: Una tarea puede estar en varias vistas
- **Given** una tarea abierta sin proyecto, con la etiqueta `doing` y `due` de hoy
- **When** se calculan las vistas
- **Then** aparece en `inbox`, en `today`, en `doing` y en `all`

### Requirement: `set_label` SHALL fusionar las etiquetas en el servicio y nunca reemplazar el array a ciegas

`PATCH /api/v1/tasks/{id}` con `labels` **reemplaza el array completo** (comprobado contra el
servidor), así que la composición de la lista SHALL hacerla el servicio mediante una operación
`set_label(task_id, group, value)` con `group` en `state`|`scope` y `value` en
`doing`|`waiting`|`shared`|`personal`|`None`:

1. SHALL leer la tarea y sus etiquetas actuales;
2. SHALL resolver los ids de las etiquetas del grupo desde `GET /api/v1/labels`, por **nombre
   exacto** (el servidor no impone unicidad);
3. SHALL componer la lista final como «etiquetas actuales **menos** las del grupo, **más** la nueva
   si `value` no es `None`;
4. SHALL enviar la lista completa en un único `PATCH`.

`state` y `scope` SHALL ser **grupos mutuamente excluyentes**: fijar un valor del grupo SHALL quitar
el valor anterior del mismo grupo y NO SHALL tocar las etiquetas del otro grupo ni ninguna otra
etiqueta de la tarea. Con `value: None` SHALL quitar únicamente las etiquetas del grupo.

Si la etiqueta no existe SHALL crearla con `POST /api/v1/labels` (en el espacio por defecto) una sola
vez por llamada y SHALL informarlo en el resultado. Si ya existe una etiqueta con ese nombre SHALL
**adoptarla** en lugar de duplicarla. NO SHALL recrear en cada turno una etiqueta que el usuario haya
borrado.

#### Scenario: Poner doing no borra las demás etiquetas
- **Given** una tarea con las etiquetas `shared` y `casa`
- **When** se ejecuta `set_label(id, "state", "doing")`
- **Then** el `PATCH` envía `doing`, `shared` y `casa`
- **And** `casa` sigue en la tarea

#### Scenario: Los grupos son excluyentes
- **Given** una tarea con la etiqueta `doing`
- **When** se ejecuta `set_label(id, "state", "waiting")`
- **Then** el `PATCH` envía `waiting` y ya no envía `doing`
- **And** las etiquetas de ámbito de la tarea se conservan

#### Scenario: Quitar la etiqueta de un grupo
- **Given** una tarea con las etiquetas `doing` y `personal`
- **When** se ejecuta `set_label(id, "state", None)`
- **Then** el `PATCH` envía `personal` y ya no envía `doing`

#### Scenario: Una etiqueta ausente se crea una sola vez
- **Given** que `doing` no existe en el espacio por defecto
- **When** se ejecuta `set_label(id, "state", "doing")`
- **Then** se crea la etiqueta `doing` con una única llamada a `POST /api/v1/labels`
- **And** el resultado lo indica

#### Scenario: Una etiqueta homónima del usuario se adopta
- **Given** que el usuario ya tiene una etiqueta llamada `shared`
- **When** se ejecuta `set_label(id, "scope", "shared")`
- **Then** NO se crea una etiqueta nueva
- **And** se usa el id de la existente
