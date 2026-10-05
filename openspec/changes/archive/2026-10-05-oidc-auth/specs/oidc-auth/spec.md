# Spec Delta

## Purpose

Autenticación de Valet mediante OpenID Connect: protege la puerta de entrada a la API con un flujo de Authorization Code, una sesión en cookie firmada y el cierre de sesión local y SSO, manteniendo un modelo de datos single-user.

## ADDED Requirements

### Requirement: El inicio de sesión redirige al proveedor OIDC

`GET /api/auth/login` SHALL generar un `state` y un `nonce` de un solo uso, SHALL persistirlos asociados a la sesión de navegación en curso y SHALL responder con una redirección HTTP 302 a la URL de autorización del proveedor OIDC, incluyendo `client_id`, `redirect_uri`, `response_type=code`, `scope=openid profile email`, el `state` y el `nonce` generados.

#### Scenario: Redirección a la autorización del proveedor
- **Given** un usuario sin sesión que visita `GET /api/auth/login`
- **When** el backend construye la petición de autorización
- **Then** responde `302` con `Location` apuntando al endpoint de autorización del proveedor
- **And** la URL incluye `response_type=code`, `client_id`, `redirect_uri`, `scope` con `openid`, y un `state` y un `nonce` no vacíos y distintos entre peticiones

#### Scenario: Valet no recibe credenciales en el login
- **Given** el flujo de login iniciado
- **When** el navegador se redirige al proveedor
- **Then** Valet NO SHALL pedir ni almacenar la contraseña del usuario
- **And** la contraseña solo SHALL introducirse en el dominio del proveedor OIDC

### Requirement: El callback valida state y nonce e intercambia el código

`GET /api/auth/callback` SHALL aceptar un `code` y un `state`; SHALL rechazar con 401 la petición cuyo `state` no coincida con el emitido por este navegador o haya sido ya consumido; SHALL intercambiar el `code` en el token endpoint del proveedor; y SHALL validar el ID token resultante comprobando firma contra el JWKS del proveedor, `issuer`, `audience` igual al `client_id`, no expiración y que el `nonce` coincida con el emitido. Solo tras una validación correcta SHALL establecer la cookie de sesión y redirigir al SPA.

#### Scenario: Callback correcto establece la sesión
- **Given** un callback con un `code` y un `state` válidos y no consumidos
- **When** el backend intercambia el código y valida el ID token contra el JWKS del proveedor
- **Then** responde `302` hacia el SPA
- **And** establece una cookie de sesión válida
- **And** el usuario queda autenticado

#### Scenario: State inválido o reutilizado
- **Given** un callback cuyo `state` no coincide con el emitido o que ya fue consumido
- **When** se procesa el callback
- **Then** responde `401` y NO SHALL establecer cookie de sesión
- **And** NO SHALL intercambiar el código en el proveedor

#### Scenario: ID token no válido
- **Given** un callback con `state` correcto pero un ID token con firma inválida, `issuer` incorrecto, `audience` distinta del `client_id`, expirado o con `nonce` distinto
- **When** se valida el ID token
- **Then** responde `401` sin establecer cookie de sesión
- **And** la respuesta NO SHALL revelar el detalle del fallo de validación

### Requirement: La sesión se transporta en una cookie firmada y HttpOnly

Tras un callback válido, Valet SHALL emitir una sesión como un token firmado (HS256 con el secreto configurado) que incluya al menos el identificador del sujeto, el email, el nombre y una expiración. La sesión SHALL retener además el ID token emitido por el proveedor (necesario para el logout SSO posterior), sin exponerlo a JavaScript; el ID token NO SHALL ser legible desde `document.cookie`. La cookie SHALL ser `HttpOnly`, `Secure` y `SameSite=Lax`, SHALL tener una expiración finita y NO SHALL ser legible desde JavaScript. Una cookie ausente, manipulada o expirada SHALL considerarse no autenticada.

#### Scenario: La cookie no es accesible desde JavaScript
- **Given** un usuario autenticado
- **When** el frontend intenta leer la cookie de sesión
- **Then** la cookie no está disponible para `document.cookie`
- **And** el token de sesión solo viaja entre navegador y backend

#### Scenario: La sesión conserva el ID token para el logout
- **Given** una sesión establecida tras un callback válido
- **When** se inspecciona la sesión emitida
- **Then** la sesión conserva el ID token del proveedor
- **And** el ID token NO SHALL ser accesible desde JavaScript

#### Scenario: Cookie manipulada o expirada se rechaza
- **Given** una cookie de sesión con la firma alterada o con la expiración superada
- **When** el backend valida la sesión
- **Then** la petición se trata como no autenticada y responde `401`

### Requirement: El endpoint de identidad devuelve los claims de la sesión

`GET /api/auth/me` SHALL devolver, para una sesión válida, los claims `sub`, `email` y `name`; SHALL responder `401` cuando no haya sesión válida; y NO SHALL devolver el token de sesión ni secretos en el cuerpo.

#### Scenario: Sesión válida devuelve los claims
- **Given** una petición a `/api/auth/me` con una cookie de sesión válida
- **When** el backend valida la cookie
- **Then** responde `200` con `sub`, `email` y `name`
- **And** el cuerpo NO SHALL contener el token de sesión

#### Scenario: Sin sesión responde 401
- **Given** una petición a `/api/auth/me` sin cookie de sesión
- **When** el backend procesa la petición
- **Then** responde `401`

### Requirement: El logout cierra la sesión local y la sesión SSO

`POST /api/auth/logout` SHALL invalidar la cookie de sesión de Valet y SHALL devolver al cliente la URL del `end_session_endpoint` del proveedor OIDC construida con `id_token_hint` igual al ID token retenido en la sesión y, cuando esté configurado, `post_logout_redirect_uri`, para que se cierre también la sesión SSO; si la sesión no tiene ID token, SHALL igualmente cerrar la sesión local. El frontend SHALL redirigir al usuario a esa URL tras completar el cierre local.

#### Scenario: Logout limpia la cookie y ofrece el cierre SSO
- **Given** un usuario con sesión iniciada
- **When** envía `POST /api/auth/logout`
- **Then** la cookie de sesión deja de ser válida
- **And** la respuesta incluye la URL del `end_session_endpoint` del proveedor
- **And** el frontend redirige a esa URL para cerrar la sesión SSO

#### Scenario: Logout incluye id_token_hint
- **Given** una sesión iniciada que conserva el ID token del proveedor
- **When** envía `POST /api/auth/logout`
- **Then** la URL devuelta apunta al `end_session_endpoint` del proveedor
- **And** incluye `id_token_hint` igual al ID token retenido en la sesión
- **And** cuando `AUTH_POST_LOGOUT_REDIRECT_URL` está configurado, incluye `post_logout_redirect_uri` con ese valor

#### Scenario: Logout sin ID token cierra solo la sesión local
- **Given** una sesión de Valet que no conserva ID token del proveedor
- **When** envía `POST /api/auth/logout`
- **Then** la cookie de sesión deja de ser válida
- **And** NO SHALL fallar el cierre local por la ausencia del ID token

#### Scenario: Tras el logout las rutas protegidas vuelven a exigir sesión
- **Given** un usuario que ha cerrado sesión
- **When** solicita una ruta protegida de `/api/*`
- **Then** recibe `401` hasta que vuelva a autenticarse

### Requirement: Enforcement de autenticación sobre la API

Cuando `auth.enabled` es `true`, todas las rutas `/api/*` SHALL exigir una sesión válida y responder `401` en su ausencia, EXCEPTO `/api/health` y las rutas bajo `/api/auth/*`, que SHALL permanecer accesibles sin sesión. El middleware NO SHALL permitir el paso a ninguna ruta protegida por el mero hecho de presentar un token malformado.

#### Scenario: Ruta protegida sin sesión responde 401
- **Given** `AUTH_ENABLED=true` y una petición sin cookie a `/api/messages`
- **When** el middleware evalúa la petición
- **Then** responde `401` y NO SHALL ejecutar el handler de la ruta

#### Scenario: Health y auth quedan exentos
- **Given** `AUTH_ENABLED=true`
- **When** se solicita `/api/health`, `/api/auth/login` o `/api/auth/callback` sin sesión
- **Then** la petición NO SHALL ser rechazada por el middleware de sesión

#### Scenario: Ruta protegida con sesión válida pasa
- **Given** `AUTH_ENABLED=true` y una cookie de sesión válida
- **When** se solicita `/api/messages`
- **Then** la petición alcanza el handler y NO responde `401` por falta de sesión

### Requirement: Modo desarrollo sin OIDC

Cuando `auth.enabled` es `false`, el sistema SHALL permitir el acceso a todas las rutas como un usuario dev por defecto, sin exigir cookie ni redirección al proveedor, de forma que el desarrollo local y los tests puedan ejecutarse sin un proveedor OIDC.

#### Scenario: Sin OIDC la API es accesible como usuario dev
- **Given** `AUTH_ENABLED=false`
- **When** se solicita cualquier ruta de `/api/*` sin cookie
- **Then** la petición NO SHALL ser rechazada por el middleware de sesión
- **And** el sistema opera con un usuario dev por defecto

### Requirement: Seguridad de CORS, cookies y errores

El backend SHALL configurar CORS con credenciales usando un origen explícito derivado de la configuración y NO SHALL usar un comodín `Any` cuando las credenciales están habilitadas. Los `state` y `nonce` SHALL ser de un solo uso. Los errores de validación de token SHALL responderse como `401` sin filtrar detalles internos (mensaje del proveedor, contenido del token o trazas).

#### Scenario: CORS no usa comodín con credenciales
- **Given** la configuración de CORS con credenciales habilitadas
- **When** un cliente realiza una petición cross-origin
- **Then** el origen permitido es explícito y NO SHALL ser `*`

#### Scenario: Errores de token no filtran detalles
- **Given** un ID token o una cookie de sesión inválidos
- **When** el backend responde
- **Then** el cuerpo es un `401` genérico que NO SHALL incluir el mensaje del proveedor, el token ni trazas internas
