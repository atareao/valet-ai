# oximeal-api Specification

## Purpose
TBD - created by archiving change oximeal-integration. Update Purpose after archive.

## Requirements

### Requirement: El secreto del cliente M2M de OxiMeal SHALL ser un ajuste sensible con endpoints dedicados

`oximeal_client_secret` SHALL formar parte de `SENSITIVE_KEYS`, de modo que NO aparezca en `GET /api/settings` y SHALL ignorarse en `PUT /api/settings`. Como esa vía no puede escribirlo, la API SHALL exponer `GET /api/oximeal/status` → `200 {"configured": <bool>, "base_url": <string>, "issuer_url": <string>, "has_client_id": <bool>}` **sin** el secreto, y `PUT /api/oximeal/secret` con cuerpo `{"client_secret": <string>}` que SHALL responder `400` con un secreto ausente o vacío y, con un valor válido, SHALL guardarlo recortado en `settings` y responder `204` sin devolverlo.

Los ajustes `oximeal_base_url` (por defecto `https://oximeal.territoriolinux.es`), `oximeal_issuer_url` y `oximeal_client_id` NO SHALL ser sensibles y SHALL seguir viajando en `GET`/`PUT /api/settings`, como las URL base de apimail, Miniflux y OpenTodo.

#### Scenario: El secreto no aparece en la lectura de ajustes
- **Given** `oximeal_client_secret` configurado
- **When** se hace `GET /api/settings`
- **Then** la respuesta NO contiene `oximeal_client_secret`
- **And** sí contiene `oximeal_base_url`, `oximeal_issuer_url` y `oximeal_client_id`

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

### Requirement: `GET /api/oximeal/check` SHALL sondear la conexión real sin revelar el secreto ni el token

La API SHALL exponer `GET /api/oximeal/check`, que SHALL obtener un token M2M y consultar de verdad la Agent API de OxiMeal (`GET /api/agent/recent-recipes?days=1`) en lugar de limitarse a leer los ajustes, y SHALL responder **siempre `200`** con `{"ok": <bool>, "detail": <string>}`, con un mensaje en español y **nunca** el secreto ni el token:

- con un token válido y la Agent API respondiendo → `{"ok": true, "detail": "conexión correcta"}`;
- sin `client_id`/`client_secret` → `{"ok": false, "detail": "configuración incompleta"}`;
- con el IdP sin entregar token → `{"ok": false, "detail": "el IdP no entregó token"}`;
- con el token rechazado (`401`/`403`) → `{"ok": false, "detail": "el token fue rechazado"}`;
- con un fallo de red o `5xx` → `{"ok": false, "detail": "OxiMeal no responde"}`.

El código de estado SHALL ser `200` incluso cuando `ok` sea `false`: la sonda informa de un estado, no es un error de la API de Valet.

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

### Requirement: Una migración SHALL sembrar los ajustes de OxiMeal y el fragmento de prompt de la skill `menus`

Una migración aditiva e idempotente SHALL sembrar `oximeal_base_url` con `https://oximeal.territoriolinux.es`, `oximeal_client_id` vacío y `oximeal_issuer_url` vacío, sin pisar valores ya fijados por el usuario. La misma migración SHALL sembrar el ajuste `SKILL_MENUS_PROMPT` con un texto por defecto que explique que `menus` cubre OxiMeal (recetas, plan semanal y lista de la compra), que `plan_week` exige el `recipe_id` de una receta obtenida antes con `search_recipes`, y que OxiMeal es un recetario externo distinto de la store interna de `recuerdos`. El texto SHALL conservar `# SKILL ACTIVA: MENUS` como **primera línea**. La siembra SHALL aplicarse solo si la clave falta o está vacía; una edición del usuario SHALL conservarse intacta.

#### Scenario: Los ajustes se siembran con sus defaults
- **Given** la base de datos recién migrada
- **When** se consultan los ajustes
- **Then** `oximeal_base_url` vale `https://oximeal.territoriolinux.es`
- **And** `oximeal_client_id`, `oximeal_issuer_url` y `oximeal_client_secret` están vacíos

#### Scenario: La migración respeta un valor ya fijado
- **Given** `settings.oximeal_base_url = "https://oxi.midominio.es"` antes de migrar
- **When** se ejecuta la migración
- **Then** el valor sigue siendo `https://oxi.midominio.es`

#### Scenario: El fragmento sembrado tiene su encabezado
- **Given** la base de datos recién migrada
- **When** se consulta `SKILL_MENUS_PROMPT`
- **Then** su primera línea es `# SKILL ACTIVA: MENUS`
- **And** menciona OxiMeal, el plan semanal y la lista de la compra

#### Scenario: Un fragmento editado por el usuario no se pisa
- **Given** `SKILL_MENUS_PROMPT` con un texto distinto del sembrado
- **When** se ejecuta la migración
- **Then** el valor NO cambia
