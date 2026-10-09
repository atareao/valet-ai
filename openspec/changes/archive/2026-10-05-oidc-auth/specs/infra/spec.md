# Spec Delta

## ADDED Requirements

### Requirement: El compose de producción levanta backend, frontend y PocketID

`docker-compose.prod.yml` SHALL definir tres servicios: `backend`, `frontend` (nginx) y `pocketid`, con las dependencias entre ellos (`frontend` tras `backend`); SHALL propagar al backend las variables `AUTH_ENABLED`, `AUTH_ISSUER_URL`, `AUTH_CLIENT_ID`, `AUTH_CLIENT_SECRET`, `JWT_SECRET`, `AUTH_REDIRECT_URL` y `AUTH_POST_LOGOUT_REDIRECT_URL`; y SHALL exponer PocketID para que el flujo OIDC sea alcanzable.

#### Scenario: El stack de producción declara los tres servicios
- **Given** `docker-compose.prod.yml`
- **When** se inspeccionan sus servicios
- **Then** existen `backend`, `frontend` y `pocketid`
- **And** `frontend` declara `depends_on: backend`

#### Scenario: AUTH_REDIRECT_URL llega al backend
- **Given** el servicio `backend` del compose de producción
- **When** se inspecciona su bloque `environment`
- **Then** incluye `AUTH_REDIRECT_URL` además de `AUTH_ISSUER_URL`, `AUTH_CLIENT_ID`, `AUTH_CLIENT_SECRET` y `JWT_SECRET`

#### Scenario: AUTH_POST_LOGOUT_REDIRECT_URL llega al backend
- **Given** el servicio `backend` del compose de producción
- **When** se inspecciona su bloque `environment`
- **Then** incluye `AUTH_POST_LOGOUT_REDIRECT_URL` junto al resto de variables `AUTH_*` y `JWT_SECRET`

### Requirement: Los Dockerfiles de backend y frontend existen y construyen

El repositorio SHALL contener `Dockerfile.backend`, que compila el binario Rust (musl) y arranca la API en el puerto 3000, y `Dockerfile.frontend`, que compila el frontend Vite y lo sirve con nginx. Ambos ficheros, referenciados por `docker-compose.prod.yml`, SHALL existir y construir sin error desde un clon limpio.

#### Scenario: Ambos Dockerfiles construyen
- **Given** un clon limpio del repositorio con `Dockerfile.backend` y `Dockerfile.frontend`
- **When** se ejecuta el build de ambos servicios
- **Then** las dos imágenes se construyen sin errores
- **And** la imagen de backend contiene el binario que responde en el puerto 3000
- **And** la imagen de frontend contiene el `dist` compilado servido por nginx

#### Scenario: El compose ya no referencia ficheros inexistentes
- **Given** `docker-compose.prod.yml` y los dos Dockerfiles
- **When** se valida la configuración del compose
- **Then** los `dockerfile:` referenciados existen en el repositorio y el build deja de fallar por ficheros ausentes

### Requirement: nginx sirve el SPA y hace proxy de /api

`nginx.conf` SHALL servir los assets estáticos del SPA con fallback a `index.html` para rutas de cliente, y SHALL hacer proxy de `/api` al servicio `backend`. El proxy SHALL propagar las cabeceras necesarias para SSE (`Connection`, `Cache-Control`) y NO SHALL bufferizar las respuestas de streaming.

#### Scenario: Ruta de cliente devuelve el SPA
- **Given** el contenedor `frontend` en ejecución
- **When** se solicita una ruta de cliente como `/calendar` que no corresponde a ningún fichero
- **Then** nginx responde el `index.html` del SPA

#### Scenario: /api se enruta al backend
- **Given** el contenedor `frontend` en ejecución
- **When** se solicita `/api/health`
- **Then** nginx hace proxy de la petición al servicio `backend` y devuelve su respuesta

#### Scenario: SSE sin buffering
- **Given** una ruta de streaming bajo `/api` que emite Server-Sent Events
- **When** nginx hace proxy de la conexión
- **Then** las respuestas NO SHALL bufferizarse y los eventos llegan al cliente de forma incremental

### Requirement: El stack de producción arranca sano

Con el stack de producción levantado, `/api/health` SHALL responder `200` y la aplicación SHALL ser accesible a través de nginx en el puerto publicado, de modo que el despliegue de producción arranque de manera verificable.

#### Scenario: El health del stack responde tras el arranque
- **Given** el stack de producción levantado con `docker compose -f docker-compose.prod.yml up -d`
- **When** se consulta `/api/health`
- **Then** responde `200` con `status ok`
- **And** el SPA responde a través de nginx en el puerto publicado
