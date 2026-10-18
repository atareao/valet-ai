## MODIFIED Requirements

### Requirement: Enforcement de autenticación sobre la API

Cuando `auth.enabled` es `true`, todas las rutas `/api/*` SHALL exigir una sesión válida y responder `401` en su ausencia, EXCEPTO `/api/health` y las rutas bajo `/api/auth/*`, que SHALL permanecer accesibles sin sesión, y EXCEPTO el par (método, ruta) `POST /api/notifications`, que SHALL permanecer accesible sin sesión **porque lo protege el token de ingesta** (capability `ingest-keys`) y que SHALL responder `401` cuando no se presente un token de ingesta válido. La exención SHALL decidirse por el par (método, ruta), no por prefijo: un `GET` a `/api/notifications/unread` y un `POST` a `/api/notifications/mark-read` SHALL exigir sesión válida y responder `401` en su ausencia. El middleware NO SHALL permitir el paso a ninguna ruta protegida por el mero hecho de presentar un token malformado.

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

#### Scenario: La ingesta sin sesión pero con token válido alcanza el handler
- **Given** `AUTH_ENABLED=true`, una petición sin cookie de sesión a `POST /api/notifications` y un token de ingesta válido
- **When** el middleware evalúa la petición
- **Then** la petición NO SHALL ser rechazada por el middleware de sesión
- **And** alcanza el handler

#### Scenario: La ingesta sin token responde 401
- **Given** `AUTH_ENABLED=true` y una petición a `POST /api/notifications` sin token de ingesta válido
- **When** se procesa la petición
- **Then** responde `401`
- **And** NO SHALL ejecutar el handler

#### Scenario: Las lecturas de notificaciones exigen sesión aunque haya token de ingesta
- **Given** `AUTH_ENABLED=true` y una petición a `GET /api/notifications/unread` sin sesión pero con un token de ingesta válido presente
- **When** el middleware evalúa la petición
- **Then** responde `401` y NO SHALL ejecutar el handler
