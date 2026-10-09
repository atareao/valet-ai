# Spec Delta

## ADDED Requirements

### Requirement: El frontend presenta una pantalla de login

Cuando no exista una sesión válida, el frontend SHALL mostrar una pantalla de login con un control que inicie el flujo OIDC navegando a `/api/auth/login`; NO SHALL mostrar el contenido autenticado de la aplicación mientras la sesión no esté resuelta.

#### Scenario: Sin sesión se muestra la pantalla de login
- **Given** un usuario sin sesión válida
- **When** se carga la aplicación
- **Then** se muestra la pantalla de login y no el contenido autenticado

#### Scenario: El control de login redirige a /api/auth/login
- **Given** la pantalla de login visible
- **When** el usuario activa el control de inicio de sesión
- **Then** el navegador navega a `/api/auth/login`

### Requirement: Todas las peticiones incluyen credenciales

El cliente HTTP del frontend SHALL enviar las cookies de sesión en todas las peticiones a la API (`credentials: include`), incluidas las de streaming, para que la sesión viaje en cada petición.

#### Scenario: El cliente envía credenciales
- **Given** el cliente HTTP del frontend
- **When** se realiza cualquier petición a `/api/*`
- **Then** la petición incluye las cookies de sesión (`credentials: include`)
- **And** la sesión se envía igualmente en las peticiones de streaming

### Requirement: Un 401 global redirige a login

Cuando cualquier petición a la API reciba `401`, el frontend SHALL redirigir al usuario a la pantalla de login, salvo que se trate del propio flujo de autenticación, evitando dejar al usuario en un estado autenticado inconsistente.

#### Scenario: Un 401 fuerza la vuelta al login
- **Given** una sesión expirada o inexistente
- **When** una petición a la API responde `401`
- **Then** el frontend redirige a la pantalla de login
- **And** no continúa mostrando contenido autenticado

### Requirement: El header incluye un botón de logout

El `AppLayout` SHALL mostrar un botón de logout cuando el usuario está autenticado; al activarlo, SHALL cerrar la sesión local llamando a `POST /api/auth/logout` y, a continuación, SHALL redirigir a la URL de cierre de sesión del proveedor devuelta por el backend (campo `end_session_url` de la respuesta) para cerrar también la sesión SSO.

#### Scenario: Logout local y SSO desde el header
- **Given** un usuario autenticado viendo la aplicación
- **When** activa el botón de logout
- **Then** se llama a `POST /api/auth/logout`
- **And** la sesión local deja de estar disponible
- **And** el navegador redirige a la URL de cierre de sesión del proveedor devuelta por el backend (`end_session_url`)

### Requirement: Estado de carga mientras se resuelve la sesión

Mientras se resuelve `GET /api/auth/me` al cargar la aplicación, el frontend SHALL mostrar un estado de carga y NO SHALL mostrar ni el contenido autenticado ni la pantalla de login hasta conocer el resultado.

#### Scenario: Estado de carga durante la comprobación de sesión
- **Given** la aplicación recién cargada a la espera de `/api/auth/me`
- **When** la petición está en curso
- **Then** se muestra un indicador de carga
- **And** no se muestra contenido autenticado ni la pantalla de login antes de recibir la respuesta
