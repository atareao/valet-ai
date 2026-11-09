# blog-api — Delta de especificación (change wordpress-credentials-settings)

## ADDED Requirements

### Requirement: La Application Password del blog SHALL ser un ajuste sensible con endpoints dedicados

`blog_app_password` SHALL formar parte de `SENSITIVE_KEYS`, de modo que NO aparezca en
`GET /api/settings` y SHALL ignorarse en `PUT /api/settings`. Como esa vía no puede escribirlo, la API
SHALL exponer `GET /api/blog/status` → `200 {"configured": <bool>, "base_url": <string>, "user": <string>}`
**sin** la contraseña, y `PUT /api/blog/password` con cuerpo `{"password": <string>}` que SHALL responder
`400` con una contraseña ausente o vacía y, con una contraseña válida, SHALL guardarla recortada en
`settings` y responder `204` sin devolverla.

Los ajustes `blog_base_url` (por defecto `https://atareao.es`) y `blog_service_user` NO SHALL ser
sensibles y SHALL viajar en `GET`/`PUT /api/settings`, igual que las URL base de apimail, Miniflux y
OpenTodo. `configured` SHALL ser `true` solo cuando `blog_service_user` y `blog_app_password` tengan
valor no vacío.

#### Scenario: La contraseña no aparece en la lectura de ajustes

- **Given** `blog_app_password` configurado
- **When** se hace `GET /api/settings`
- **Then** la respuesta NO contiene `blog_app_password`
- **And** sí contiene `blog_base_url` y `blog_service_user`

#### Scenario: El estado informa sin revelar la contraseña

- **Given** `blog_service_user` y `blog_app_password` configurados
- **When** se hace `GET /api/blog/status`
- **Then** la respuesta indica `configured: true` con `base_url` y `user`
- **And** NO incluye la contraseña

#### Scenario: Una contraseña vacía se rechaza

- **Given** una llamada `PUT /api/blog/password` con `{"password": "   "}`
- **When** se ejecuta
- **Then** la respuesta es `400`
- **And** no se guarda nada

#### Scenario: Una contraseña válida se guarda

- **Given** una llamada `PUT /api/blog/password` con una contraseña no vacía
- **When** se ejecuta
- **Then** la contraseña queda guardada en `settings` recortada
- **And** la respuesta es `204` y NO devuelve la contraseña

### Requirement: `GET /api/blog/check` SHALL sondear la conexión real sin revelar la contraseña

La API SHALL exponer `GET /api/blog/check`, que SHALL consultar de verdad el sitio de WordPress
—`GET <base_url>/wp-json/wp/v2/users/me` con autenticación básica— en lugar de limitarse a leer los
ajustes, y SHALL responder **siempre `200`** con `{"ok": <bool>, "detail": <string>}`, con un mensaje
en español y **nunca** la contraseña ni la cabecera `Authorization`:

- con credenciales válidas → `{"ok": true, "detail": "conexión correcta"}`;
- sin usuario o sin contraseña configurados → `{"ok": false, "detail": "faltan el usuario o la contraseña de aplicación del blog"}`;
- con credenciales rechazadas (`401`/`403`) → `{"ok": false, "detail": "el usuario o la contraseña de aplicación no son válidos"}`;
- con un fallo de red o `5xx` → `{"ok": false, "detail": "el blog no responde"}`.

El código de estado SHALL ser `200` incluso cuando `ok` sea `false`: la sonda informa de un estado, no
es un error de la API de Valet.

#### Scenario: Sonda correcta con credenciales válidas

- **Given** usuario y contraseña configurados y el sitio respondiendo a `GET /wp-json/wp/v2/users/me`
- **When** se hace `GET /api/blog/check`
- **Then** la respuesta es `200` con `ok: true`
- **And** `detail` informa de que la conexión es correcta

#### Scenario: Sin credenciales la sonda lo dice sin tocar la red

- **Given** `blog_service_user` o `blog_app_password` vacíos o ausentes
- **When** se hace `GET /api/blog/check`
- **Then** la respuesta es `200` con `ok: false`
- **And** el mensaje indica que faltan el usuario o la contraseña
- **And** no se envía ninguna petición al sitio de WordPress

#### Scenario: Unas credenciales inválidas se distinguen de una caída

- **Given** unas credenciales configuradas y el sitio respondiendo `401`
- **When** se hace `GET /api/blog/check`
- **Then** la respuesta es `200` con `ok: false`
- **And** el mensaje dice que el usuario o la contraseña no son válidos
- **And** el mensaje NO contiene la contraseña

#### Scenario: Un sitio caído no es un error de Valet

- **Given** unas credenciales configuradas y el sitio sin responder
- **When** se hace `GET /api/blog/check`
- **Then** la respuesta es `200` con `ok: false`
- **And** el mensaje dice que el blog no responde

### Requirement: Una migración SHALL sembrar los ajustes de Blog

Una migración aditiva e idempotente SHALL sembrar `blog_base_url` (`https://atareao.es`),
`blog_service_user` (vacío) y `blog_app_password` (vacío), sin pisar valores ya fijados por el usuario
y sin tocar ningún otro ajuste. NO SHALL sembrar la contraseña con un valor por defecto.

#### Scenario: Los ajustes se siembran con sus defaults

- **Given** la base de datos recién migrada
- **When** se consultan los ajustes
- **Then** `blog_base_url` vale `https://atareao.es`
- **And** `blog_service_user` está vacío
- **And** `blog_app_password` está vacío

#### Scenario: La migración respeta un valor ya fijado

- **Given** `settings.blog_base_url = "https://mi-blog.example"` antes de migrar
- **When** se ejecuta la migración
- **Then** el valor sigue siendo `https://mi-blog.example`
