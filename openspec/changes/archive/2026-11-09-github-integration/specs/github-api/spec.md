# github-api — Delta de especificación (change github-integration)

## ADDED Requirements

### Requirement: `github_token` SHALL ser un ajuste sensible con endpoints dedicados

`github_token` SHALL formar parte de `SENSITIVE_KEYS`, de modo que NO aparezca en
`GET /api/settings` y SHALL ignorarse en `PUT /api/settings`. Como esa vía no puede escribirlo, la
API SHALL exponer `GET /api/github/status` → `200 {"configured": <bool>, "default_repo": <string>}`
**sin** el token, y `PUT /api/github/token` con cuerpo `{"token": <string>}` que SHALL responder
`400` con un token ausente o vacío y, con un valor válido, SHALL guardarlo recortado en `settings` y
responder `204` sin devolverlo. `configured` SHALL ser `true` solo cuando `github_token` tenga valor
no vacío.

Los ajustes `github_default_repo` (por defecto `atareao/valet-ai`) y `github_default_labels` NO SHALL
ser sensibles y SHALL viajar en `GET`/`PUT /api/settings`, igual que las URL base de apimail,
Miniflux y OpenTodo.

#### Scenario: El token no aparece en la lectura de ajustes

- **Given** `github_token` configurado
- **When** se hace `GET /api/settings`
- **Then** la respuesta NO contiene `github_token`
- **And** sí contiene `github_default_repo` y `github_default_labels`

#### Scenario: El estado informa sin revelar el token

- **Given** `github_token` configurado
- **When** se hace `GET /api/github/status`
- **Then** la respuesta indica `configured: true` con `default_repo`
- **And** NO incluye el token

#### Scenario: Un token vacío se rechaza

- **Given** una llamada `PUT /api/github/token` con `{"token": "   "}`
- **When** se ejecuta
- **Then** la respuesta es `400`
- **And** no se guarda nada

#### Scenario: Un token válido se guarda

- **Given** una llamada `PUT /api/github/token` con un token no vacío
- **When** se ejecuta
- **Then** el token queda guardado en `settings` recortado
- **And** la respuesta es `204` y NO lo devuelve

#### Scenario: Los ajustes no sensibles de GitHub viajan con los ajustes

- **Given** una llamada `PUT /api/settings` con `github_default_repo` y `github_default_labels`
- **When** se ejecuta
- **Then** ambos valores se persisten
- **And** `github_token` se ignora si viaja en el cuerpo

### Requirement: `GET /api/github/check` SHALL sondear la conexión real sin revelar el token

La API SHALL exponer `GET /api/github/check`, que SHALL consultar de verdad GitHub —`GET
https://api.github.com/user` con `Authorization: Bearer <token>`— en lugar de limitarse a leer los
ajustes, y SHALL responder **siempre `200`** con `{"ok": <bool>, "detail": <string>}`, con un mensaje
en español y **nunca** el token ni la cabecera `Authorization`:

- con un token válido y GitHub respondiendo → `{"ok": true, "detail": "conexión correcta"}`;
- sin `github_token` configurado → `{"ok": false, "detail": "configuración incompleta"}`;
- con el token rechazado (`401`/`403`) → `{"ok": false, "detail": "el token fue rechazado"}`;
- con un fallo de red o `5xx` → `{"ok": false, "detail": "GitHub no responde"}`.

El código de estado SHALL ser `200` incluso cuando `ok` sea `false`: la sonda informa de un estado,
no es un error de la API de Valet.

#### Scenario: Sonda correcta con un token válido

- **Given** `github_token` configurado y GitHub respondiendo a `GET /user`
- **When** se hace `GET /api/github/check`
- **Then** la respuesta es `200` con `ok: true`
- **And** `detail` informa de que la conexión es correcta

#### Scenario: Sin token la sonda lo dice sin tocar la red

- **Given** `github_token` vacío o ausente
- **When** se hace `GET /api/github/check`
- **Then** la respuesta es `200` con `ok: false`
- **And** el mensaje indica configuración incompleta
- **And** no se envía ninguna petición a GitHub

#### Scenario: Un token rechazado se distingue de una caída

- **Given** un token configurado y GitHub respondiendo `401`
- **When** se hace `GET /api/github/check`
- **Then** la respuesta es `200` con `ok: false`
- **And** el mensaje dice que el token fue rechazado
- **And** el mensaje NO contiene el token

#### Scenario: Un GitHub caído no es un error de Valet

- **Given** un token configurado y GitHub sin responder
- **When** se hace `GET /api/github/check`
- **Then** la respuesta es `200` con `ok: false`
- **And** el mensaje dice que GitHub no responde

#### Scenario: La sonda no filtra el token

- **Given** `github_token` con valor `SECRET`
- **When** se hace `GET /api/github/check` en cualquiera de sus salidas
- **Then** el cuerpo de la respuesta NO contiene `SECRET`

### Requirement: Una migración SHALL sembrar los ajustes de GitHub y el fragmento de prompt de la skill `github`

Una migración aditiva e idempotente SHALL sembrar `github_default_repo` con `atareao/valet-ai` y
`github_default_labels` con el texto vacío, sin pisar valores ya fijados por el usuario. La misma
migración SHALL sembrar el ajuste `SKILL_GITHUB_PROMPT` con un texto por defecto que explique que
`github` cubre la creación de issues con `create_issue`, que el repositorio sale del parámetro `repo`
o del ajuste `github_default_repo` y que los labels salen de `labels` o de
`github_default_labels`. El texto SHALL conservar `# SKILL ACTIVA: GITHUB` como **primera línea**. La
siembra SHALL aplicarse solo si la clave falta o está vacía; una edición del usuario SHALL
conservarse intacta. La migración SHALL NOT sembrar `github_token`: es material sensible y su única
puerta de escritura es `PUT /api/github/token`.

#### Scenario: Los ajustes se siembran con sus defaults

- **Given** la base de datos recién migrada
- **When** se consultan los ajustes
- **Then** `github_default_repo` vale `atareao/valet-ai`
- **And** `github_default_labels` está vacío
- **And** `github_token` está vacío

#### Scenario: La migración respeta un valor ya fijado

- **Given** `settings.github_default_repo = "otro/repo"` antes de migrar
- **When** se ejecuta la migración
- **Then** el valor sigue siendo `otro/repo`

#### Scenario: El fragmento sembrado tiene su encabezado

- **Given** la base de datos recién migrada
- **When** se consulta `SKILL_GITHUB_PROMPT`
- **Then** su primera línea es `# SKILL ACTIVA: GITHUB`
- **And** menciona la creación de issues, el repositorio y los labels

#### Scenario: Un fragmento editado por el usuario no se pisa

- **Given** `SKILL_GITHUB_PROMPT` con un texto distinto del sembrado
- **When** se ejecuta la migración
- **Then** el valor NO cambia

#### Scenario: La migración es idempotente

- **Given** la migración ya aplicada
- **When** se vuelve a ejecutar
- **Then** los valores resultan iguales a los de la primera ejecución
