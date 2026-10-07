## MODIFIED Requirements

### Requirement: El frontend presenta una pantalla de login

Cuando no exista una sesión válida, el frontend SHALL mostrar una pantalla de login a pantalla completa con **fondo oscuro** (el color de fondo del layout de la aplicación, `#000000`), el **logo real de Valet** (`valet-icon.svg`) renderizado como imagen a un tamaño de **120 px**, y un control que inicie el flujo OIDC navegando a `/api/auth/login`; NO SHALL mostrar el contenido autenticado de la aplicación mientras la sesión no esté resuelta, y NO SHALL usar el emoji de chat (`💬`) como icono de la pantalla.

#### Scenario: Sin sesión se muestra la pantalla de login
- **Given** un usuario sin sesión válida
- **When** se carga la aplicación
- **Then** se muestra la pantalla de login y no el contenido autenticado

#### Scenario: El control de login redirige a /api/auth/login
- **Given** la pantalla de login visible
- **When** el usuario activa el control de inicio de sesión
- **Then** el navegador navega a `/api/auth/login`

#### Scenario: La pantalla de login usa fondo oscuro y el logo de Valet
- **Given** la pantalla de login visible
- **When** se inspecciona su contenedor raíz
- **Then** ocupa toda la ventana y tiene fondo oscuro (`#000000`)
- **And** se muestra el logo de Valet (`valet-icon.svg`) como imagen, no un emoji

#### Scenario: El logo de Valet se muestra a tamaño adecuado
- **Given** la pantalla de login visible
- **When** se inspecciona el logo
- **Then** se renderiza a 120 px de ancho y alto
