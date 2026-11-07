# oximeal-api — Delta (change oximeal-m2m-resource)

## MODIFIED Requirements

### Requirement: El secreto del cliente M2M de OxiMeal SHALL ser un ajuste sensible con endpoints dedicados

`oximeal_client_secret` SHALL formar parte de `SENSITIVE_KEYS`, de modo que NO aparezca en `GET /api/settings` y SHALL ignorarse en `PUT /api/settings`. Como esa vía no puede escribirlo, la API SHALL exponer `GET /api/oximeal/status` → `200 {"configured": <bool>, "base_url": <string>, "issuer_url": <string>, "has_client_id": <bool>, "resource": <string>}` **sin** el secreto, y `PUT /api/oximeal/secret` con cuerpo `{"client_secret": <string>}` que SHALL responder `400` con un secreto ausente o vacío y, con un valor válido, SHALL guardarlo recortado en `settings` y responder `204` sin devolverlo. El campo `resource` SHALL ser el valor resuelto (nunca un secreto).

Los ajustes `oximeal_base_url` (por defecto `https://oximeal.territoriolinux.es`), `oximeal_resource`, `oximeal_issuer_url` y `oximeal_client_id` NO SHALL ser sensibles y SHALL seguir viajando en `GET`/`PUT /api/settings`, como las URL base de apimail, Miniflux y OpenTodo.

#### Scenario: El secreto no aparece en la lectura de ajustes
- **Given** `oximeal_client_secret` configurado
- **When** se hace `GET /api/settings`
- **Then** la respuesta NO contiene `oximeal_client_secret`
- **And** sí contiene `oximeal_base_url`, `oximeal_resource`, `oximeal_issuer_url` y `oximeal_client_id`

#### Scenario: El estado informa sin revelar el secreto
- **Given** `oximeal_client_id` y `oximeal_client_secret` configurados
- **When** se hace `GET /api/oximeal/status`
- **Then** la respuesta indica `configured: true` y `has_client_id: true` con `base_url` e `issuer_url`
- **And** NO incluye el secreto

#### Scenario: Un secreto vacío se rechaza
- **Given** una llamada `PUT /api/oximeal/secret` con `{"client_secret": "   "}`
- **When** se ejecuta
- **Then** la respuesta es `400`
- **And** no se guarda nada

#### Scenario: Un secreto válido se guarda
- **Given** una llamada `PUT /api/oximeal/secret` con un valor no vacío
- **When** se ejecuta
- **Then** el valor queda guardado en `settings` recortado
- **And** la respuesta es `204` y NO lo devuelve

#### Scenario: El estado expone el resource resuelto
- **Given** `oximeal_resource` sin definir y `oximeal_base_url` sin definir
- **When** se hace `GET /api/oximeal/status`
- **Then** la respuesta incluye `"resource": "https://oximeal.territoriolinux.es/api"`
- **And** NO incluye el secreto

### Requirement: `GET /api/oximeal/check` SHALL sondear la conexión real sin revelar el secreto ni el token

La API SHALL exponer `GET /api/oximeal/check`, que SHALL obtener un token M2M y consultar de verdad la Agent API de OxiMeal (`GET /api/agent/recent-recipes?days=1`) en lugar de limitarse a leer los ajustes, y SHALL responder **siempre `200`** con `{"ok": <bool>, "detail": <string>}`, con un mensaje en español y **nunca** el secreto ni el token:

- con un token válido y la Agent API respondiendo → `{"ok": true, "detail": "conexión correcta"}`;
- sin `client_id`/`client_secret` → `{"ok": false, "detail": "configuración incompleta"}`;
- con el IdP sin entregar token → `{"ok": false, "detail": "el IdP no entregó un token M2M: <detalle redactado del IdP>"}`;
- con el token rechazado (`401`/`403`) → `{"ok": false, "detail": "el token fue rechazado"}`;
- con un fallo de red o `5xx` → `{"ok": false, "detail": "OxiMeal no responde: <detalle redactado>"}`.

El `detail` de los fallos del IdP y de OxiMeal SHALL incluir el mensaje ya redactado por el servicio, de modo que la sonda explique la causa sin filtrar nunca el secreto. El código de estado SHALL ser `200` incluso cuando `ok` sea `false`: la sonda informa de un estado, no es un error de la API de Valet.

#### Scenario: Sonda correcta con credenciales válidas
- **Given** `client_id`/`client_secret` válidos y OxiMeal respondiendo
- **When** se hace `GET /api/oximeal/check`
- **Then** la respuesta es `200` con `ok: true`
- **And** `detail` informa de que la conexión es correcta

#### Scenario: Sin credenciales la sonda lo dice sin tocar la red
- **Given** ni `oximeal_client_id` ni `oximeal_client_secret`
- **When** se hace `GET /api/oximeal/check`
- **Then** la respuesta es `200` con `ok: false`
- **And** el mensaje indica configuración incompleta
- **And** no se envía ninguna petición al IdP ni a OxiMeal

#### Scenario: Un token rechazado se distingue de una caída
- **Given** credenciales configuradas y la Agent API respondiendo `401`
- **When** se hace `GET /api/oximeal/check`
- **Then** la respuesta es `200` con `ok: false`
- **And** el mensaje dice que el token fue rechazado

#### Scenario: Un OxiMeal caído no es un error de Valet
- **Given** credenciales configuradas y OxiMeal sin responder
- **When** se hace `GET /api/oximeal/check`
- **Then** la respuesta es `200` con `ok: false`
- **And** el mensaje dice que OxiMeal no responde

#### Scenario: La sonda no filtra credenciales
- **Given** un `client_secret` con valor `SECRET` y un token obtenido
- **When** se hace `GET /api/oximeal/check` en cualquiera de sus salidas
- **Then** el cuerpo de la respuesta NO contiene `SECRET` ni el token

#### Scenario: La sonda expone el error del IdP sin filtrar secretos
- **Given** un IdP que responde error con un mensaje que contiene `SECRET`
- **When** se hace `GET /api/oximeal/check`
- **Then** la respuesta es `200` con `ok: false`
- **And** el `detail` incluye el mensaje del IdP con el detalle de la causa
- **And** el cuerpo de la respuesta NO contiene `SECRET`
