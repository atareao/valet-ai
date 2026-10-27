# opentodo-api Specification

## Purpose
API HTTP de Valet para la integración con OpenTodo: el token como ajuste sensible con endpoints dedicados (`GET /api/opentodo/status` y `PUT /api/opentodo/token`) y una sonda de conexión real (`GET /api/opentodo/check`) que informa del estado sin revelar nunca el token.

## Requirements

### Requirement: El token de OpenTodo SHALL ser un ajuste sensible con endpoints dedicados

`opentodo_token` SHALL formar parte de `SENSITIVE_KEYS`, de modo que NO aparezca en
`GET /api/settings` y SHALL ignorarse en `PUT /api/settings`. Como esa vía no puede escribirlo, la
API SHALL exponer `GET /api/opentodo/status` → `200 {"configured": <bool>, "base_url": <string>}`
**sin** el token, y `PUT /api/opentodo/token` con cuerpo `{"token": <string>}` que SHALL responder
`400` con un token ausente o vacío y, con un token válido, SHALL guardarlo recortado en `settings` y
responder `204` sin devolverlo.

El ajuste `opentodo_base_url` (por defecto `https://todo.territoriolinux.es`) NO SHALL ser sensible y
SHALL seguir viajando en `GET`/`PUT /api/settings`, igual que las URL base de apimail y Miniflux.

#### Scenario: El token no aparece en la lectura de ajustes
- **Given** `opentodo_token` configurado
- **When** se hace `GET /api/settings`
- **Then** la respuesta NO contiene `opentodo_token`
- **And** sí contiene `opentodo_base_url`

#### Scenario: El estado informa sin revelar el token
- **Given** `opentodo_token` configurado
- **When** se hace `GET /api/opentodo/status`
- **Then** la respuesta indica `configured: true` con `base_url`
- **And** NO incluye el token

#### Scenario: Un token vacío se rechaza
- **Given** una llamada `PUT /api/opentodo/token` con `{"token": "   "}`
- **When** se ejecuta
- **Then** la respuesta es `400`
- **And** no se guarda nada

#### Scenario: Un token válido se guarda
- **Given** una llamada `PUT /api/opentodo/token` con un token no vacío
- **When** se ejecuta
- **Then** el token queda guardado en `settings` recortado
- **And** la respuesta es `204` y NO devuelve el token

### Requirement: `GET /api/opentodo/check` SHALL sondear la conexión real sin revelar el token

La API SHALL exponer `GET /api/opentodo/check`, que SHALL consultar de verdad la instancia remota
(`GET /api/v1/me` del servidor de OpenTodo) en lugar de limitarse a leer los ajustes, y SHALL
responder **siempre `200`** con `{"ok": <bool>, "detail": <string>}`, con un mensaje en español y
**nunca** el token:

- con un token válido → `{"ok": true, "detail": "conexión correcta"}`;
- sin token configurado → `{"ok": false, "detail": "no hay token de OpenTodo configurado"}`;
- con un token rechazado (`401`/`403`) → `{"ok": false, "detail": "el token no es válido o ha caducado"}`;
- con un fallo de red o `5xx` → `{"ok": false, "detail": "OpenTodo no responde"}`.

El código de estado SHALL ser `200` incluso cuando `ok` sea `false`: la sonda informa de un estado,
no es un error de la API de Valet.

#### Scenario: Sonda correcta con un token válido
- **Given** un token configurado y el servidor respondiendo a `GET /api/v1/me`
- **When** se hace `GET /api/opentodo/check`
- **Then** la respuesta es `200` con `ok: true`
- **And** `detail` informa de que la conexión es correcta

#### Scenario: Sin token la sonda lo dice sin tocar la red
- **Given** ni `settings` ni el entorno con token
- **When** se hace `GET /api/opentodo/check`
- **Then** la respuesta es `200` con `ok: false`
- **And** el mensaje indica que no hay token configurado
- **And** no se envía ninguna petición al servidor de OpenTodo

#### Scenario: Un token inválido se distingue de una caída
- **Given** un token configurado y el servidor respondiendo `401`
- **When** se hace `GET /api/opentodo/check`
- **Then** la respuesta es `200` con `ok: false`
- **And** el mensaje dice que el token no es válido o ha caducado

#### Scenario: Un servidor caído no es un error de Valet
- **Given** un token configurado y el servidor sin responder
- **When** se hace `GET /api/opentodo/check`
- **Then** la respuesta es `200` con `ok: false`
- **And** el mensaje dice que OpenTodo no responde

#### Scenario: La sonda no filtra el token
- **Given** un token configurado con valor `SECRET`
- **When** se hace `GET /api/opentodo/check` en cualquiera de sus salidas
- **Then** el cuerpo de la respuesta no contiene `SECRET`

### Requirement: Una migración SHALL sembrar los ajustes de OpenTodo y actualizar el fragmento de pendientes

Una migración aditiva e idempotente SHALL sembrar `opentodo_base_url` con
`https://todo.territoriolinux.es` y `opentodo_token` vacío, sin pisar valores ya fijados por el
usuario.

La misma migración SHALL promover `SKILL_PENDIENTES_PROMPT` a un texto nuevo que explique las
vistas de OpenTodo, la prioridad `1..4`, `start: "someday"`, los proyectos y las etiquetas
`doing`/`waiting`/`shared`/`personal`, y que mantenga que los recordatorios de Valet son otra cosa
distinta de las tareas. El texto nuevo SHALL conservar `# SKILL ACTIVA: PENDIENTES` como **primera
línea** (el encabezado que usa el ensamblador de fragmentos). La promoción SHALL aplicarse **solo**
si el valor vigente es exactamente el sembrado por la migración `20261012000003`: una edición del
usuario SHALL conservarse intacta. Si la clave falta o está vacía, SHALL insertarse el texto nuevo.

#### Scenario: Los ajustes se siembran con sus defaults
- **Given** la base de datos recién migrada
- **When** se consultan los ajustes
- **Then** `opentodo_base_url` vale `https://todo.territoriolinux.es`
- **And** `opentodo_token` está vacío

#### Scenario: La migración respeta un valor ya fijado
- **Given** `settings.opentodo_base_url = "https://todo.midominio.es"` antes de migrar
- **When** se ejecuta la migración
- **Then** el valor sigue siendo `https://todo.midominio.es`

#### Scenario: El fragmento sembrado se promueve
- **Given** `SKILL_PENDIENTES_PROMPT` con exactamente el valor sembrado por `20261012000003`
- **When** se ejecuta la migración
- **Then** el fragmento pasa al texto nuevo
- **And** su primera línea es `# SKILL ACTIVA: PENDIENTES`
- **And** menciona OpenTodo, las vistas, la prioridad 1–4 y `someday`

#### Scenario: Un fragmento editado por el usuario no se pisa
- **Given** `SKILL_PENDIENTES_PROMPT` con un texto distinto del sembrado
- **When** se ejecuta la migración
- **Then** el valor NO cambia
