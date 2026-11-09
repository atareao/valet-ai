# blog-api Specification

## Purpose

Definir la superficie HTTP de la integración de Valet con el blog WordPress
(`https://atareao.es`): la sonda de conexión (`GET /api/blog/check`), el estado
(`GET /api/blog/status`) y la puerta dedicada para escribir la Application
Password (`PUT /api/blog/password`). Fija además que `blog_app_password` es un
ajuste **sensible** (omitido en `GET /api/settings` e ignorado en
`PUT /api/settings`), mientras que `blog_base_url` y `blog_service_user` no lo
son. El comportamiento de las herramientas del blog que consumen esta API
(`blog_comment_reply`, `blog_comment_delete`) vive en la spec `blog-comments`.

## Requirements

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
en **dos fases** —`GET <base_url>/wp-json/wp/v2/users/me` con autenticación básica para validar la
credencial, y `GET <base_url>/wp-json/` con la misma autenticación para comprobar la disponibilidad
de la vía de respuesta— en lugar de limitarse a leer los ajustes, y SHALL responder **siempre `200`**
con `{"ok": <bool>, "detail": <string>}`, con un mensaje en español y **nunca** la contraseña ni la
cabecera `Authorization`:

- con credenciales válidas y la vía de respuesta disponible → `{"ok": true, "detail": "conexión correcta"}`;
- con credenciales válidas pero la vía de respuesta no disponible →
  `{"ok": false, "detail": "las credenciales son válidas, pero la ruta de respuesta del blog no está disponible: revisa la política REST del plugin"}`;
- sin usuario o sin contraseña configurados → `{"ok": false, "detail": "faltan el usuario o la contraseña de aplicación del blog"}`;
- con credenciales rechazadas (`401`/`403`) → `{"ok": false, "detail": "el usuario o la contraseña de aplicación no son válidos"}`;
- con un fallo de red o `5xx` → `{"ok": false, "detail": "el blog no responde"}`.

El código de estado SHALL ser `200` incluso cuando `ok` sea `false`: la sonda informa de un estado,
no es un error de la API de Valet. La sonda SHALL NOT crear, modificar ni enumerar comentarios.

#### Scenario: Sonda correcta con credenciales válidas

- **Given** usuario y contraseña configurados, el sitio respondiendo `200` a `GET /wp-json/wp/v2/users/me` y su índice REST declarando `/wp/v2/comments` con `POST`
- **When** se hace `GET /api/blog/check`
- **Then** la respuesta es `200` con `ok: true`
- **And** `detail` es `"conexión correcta"`

#### Scenario: La vía de respuesta cerrada se distingue de unas credenciales válidas

- **Given** usuario y contraseña válidos, `users/me` respondiendo `200` y el índice REST sin la ruta `/wp/v2/comments` (o sin `POST` en sus métodos)
- **When** se hace `GET /api/blog/check`
- **Then** la respuesta es `200` con `ok: false`
- **And** `detail` dice que las credenciales son válidas pero la ruta de respuesta no está disponible y menciona la política REST del plugin
- **And** `detail` NO contiene la contraseña

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

### Requirement: La sonda SHALL comprobar la vía de respuesta por el índice REST y sin efectos secundarios

La comprobación de la vía de respuesta SHALL hacerse con `GET <base_url>/wp-json/` **autenticado**,
leyendo el mapa `routes` que el sitio publica ya filtrado para esa identidad, y SHALL considerar
disponible la vía solo cuando exista la clave `/wp/v2/comments` y su `methods` contenga `POST`.

La sonda SHALL NOT emitir ninguna petición de escritura ni de lectura sobre
`/wp-json/wp/v2/comments`: en particular, SHALL NOT publicar un comentario de prueba ni enumerar
comentarios. La clasificación de desenlaces SHALL ser:

- índice `200` con `/wp/v2/comments` y `POST` → `Ok`;
- índice `200` sin la ruta, sin `POST`, o con cuerpo sin `routes` parseable → `ReplyRouteUnavailable`;
- índice con cualquier código distinto de `200` y distinto de `5xx` → `ReplyRouteUnavailable`;
- índice con `5xx` o fallo de red → `Unavailable`.

`ReplyRouteUnavailable` SHALL ser una variante distinta de `Unauthorized`, `Forbidden`,
`RouteNotFound` y `Unavailable`, y su `Display` SHALL NOT incluir la contraseña ni la cabecera
`Authorization`. La sonda SHALL NOT reintentar ninguna de las dos peticiones.

#### Scenario: Un índice con `5xx` o caído es «el blog no responde»

- **Given** `users/me` respondiendo `200` y el índice REST devolviendo `503` (o fallando por red)
- **When** se hace `GET /api/blog/check`
- **Then** la respuesta es `200` con `ok: false`
- **And** `detail` dice que el blog no responde

#### Scenario: Un índice ilegible no se confunde con una conexión sana

- **Given** `users/me` respondiendo `200` y el índice devolviendo `200` con un cuerpo sin `routes`
- **When** se hace `GET /api/blog/check`
- **Then** la respuesta es `200` con `ok: false`
- **And** `detail` apunta a la política REST del plugin

#### Scenario: La sonda no toca la colección de comentarios

- **Given** unas credenciales válidas y un sitio de prueba
- **When** se hace `GET /api/blog/check`
- **Then** el sitio recibe exactamente `GET /wp-json/wp/v2/users/me` y `GET /wp-json/`
- **And** no recibe ninguna petición `POST` ni ninguna petición a `/wp-json/wp/v2/comments`
