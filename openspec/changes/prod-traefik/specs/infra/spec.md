## REMOVED Requirements

### Requirement: El compose de producción levanta backend, frontend y PocketID

**Reason**: el stack se simplifica a un único servicio con el `Dockerfile` monolítico del repo; PocketID es un IdP externo ya desplegado y deja de levantarse aquí.

**Migration**: ver el requisito añadido "El stack de producción sirve SPA y API con un único Dockerfile tras Traefik".

### Requirement: Los Dockerfiles de backend y frontend existen y construyen

**Reason**: `Dockerfile.backend` y `Dockerfile.frontend` se eliminan; el proyecto construye SPA + API con un único `Dockerfile`.

**Migration**: ver el requisito añadido "El stack de producción sirve SPA y API con un único Dockerfile tras Traefik".

### Requirement: nginx sirve el SPA y hace proxy de /api

**Reason**: se elimina `nginx.conf`; el propio binario sirve el SPA (frontend embebido en `/app/static`) y la API en el mismo puerto, así que ya no hay proxy inverso interno.

**Migration**: ver el requisito añadido "El stack de producción sirve SPA y API con un único Dockerfile tras Traefik".

## MODIFIED Requirements

### Requirement: El stack de producción arranca sano

Con el stack de producción levantado (`valet` construido desde el `Dockerfile` monolítico y unido a la red externa de Traefik), `/api/health` SHALL responder `200` y la aplicación SHALL ser alcanzable a través de Traefik en `https://${APP_HOST}`, sin publicar ningún puerto en el host.

#### Scenario: El health del stack responde tras el arranque
- **Given** el stack de producción levantado con `podman compose -f docker-compose.prod.yml up -d`
- **When** se consulta `/api/health` a través de Traefik
- **Then** responde `200` con `status ok`
- **And** el SPA responde en `https://${APP_HOST}/`
- **And** el servicio `valet` no publica ningún puerto en el host

## ADDED Requirements

### Requirement: El stack de producción sirve SPA y API con un único Dockerfile tras Traefik

`docker-compose.prod.yml` SHALL definir **un único servicio** llamado `valet`, que SHALL construir el `Dockerfile` monolítico del repositorio (frontend embebido en `/app/static`, API en el puerto `3000`); **NO SHALL definir** los servicios `backend` ni `frontend`, **NO SHALL referenciar** `Dockerfile.backend`/`Dockerfile.frontend` ni `nginx.conf`, y **NO SHALL publicar puertos** en el host. El servicio SHALL unirse a la red **externa** de Traefik (`TRAEFIK_NETWORK`, por defecto `traefik`) y SHALL anunciarse por labels (`Host(${APP_HOST})`, entrypoint HTTPS, `tls.certresolver=${TRAEFIK_CERT_RESOLVER}`, `loadbalancer.server.port=3000`). SHALL propagar las variables `AUTH_ENABLED`, `AUTH_ISSUER_URL`, `AUTH_CLIENT_ID`, `AUTH_CLIENT_SECRET`, `AUTH_REDIRECT_URL`, `AUTH_POST_LOGOUT_REDIRECT_URL` y `JWT_SECRET`, y SHALL persistir la base de datos en el volumen `valet_data`.

#### Scenario: El stack declara un único servicio valet
- **Given** `docker-compose.prod.yml`
- **When** se inspeccionan sus servicios
- **Then** existe únicamente `valet`
- **And** NO existen `backend`, `frontend` ni `pocketid`
- **And** `valet` construye `dockerfile: Dockerfile`

#### Scenario: El compose no referencia ficheros inexistentes
- **Given** `docker-compose.prod.yml`
- **When** se busca el `dockerfile:` referenciado
- **Then** solo referencia `Dockerfile`
- **And** el repositorio NO contiene `Dockerfile.backend`, `Dockerfile.frontend` ni `nginx.conf`

#### Scenario: El servicio se publica por labels de Traefik
- **Given** `docker-compose.prod.yml` con `APP_HOST` y `TRAEFIK_CERT_RESOLVER` definidos
- **When** se valida con `podman compose -f docker-compose.prod.yml config`
- **Then** `valet` declara una label de router con `Host(${APP_HOST})` y TLS por certresolver
- **And** declara `loadbalancer.server.port=3000`

#### Scenario: Ningún puerto se publica en el host
- **Given** el compose de producción
- **When** se inspecciona el servicio `valet`
- **Then** no declara un bloque `ports:`
- **And** el único ingress es Traefik, por la red externa

#### Scenario: Las variables de auth llegan al servicio
- **Given** el servicio `valet` del compose de producción
- **When** se inspecciona su bloque `environment`
- **Then** incluye `AUTH_ENABLED`, `AUTH_ISSUER_URL`, `AUTH_CLIENT_ID`, `AUTH_CLIENT_SECRET`, `AUTH_REDIRECT_URL`, `AUTH_POST_LOGOUT_REDIRECT_URL` y `JWT_SECRET`

#### Scenario: La base de datos persiste en un volumen
- **Given** el servicio `valet` del compose de producción
- **When** se inspeccionan sus volúmenes
- **Then** monta `valet_data` en la ruta de la base de datos
- **And** `DATABASE_URL` apunta a esa ruta

### Requirement: PocketID es un proveedor OIDC externo

El stack de producción SHALL NOT ejecutar PocketID; el servicio `valet` SHALL alcanzar el issuer público definido en `AUTH_ISSUER_URL` para el discovery, el intercambio de código y el JWKS; y la documentación SHALL indicar que el cliente OIDC (redirect y post-logout URIs) se registra en el PocketID **ya existente** del operador.

#### Scenario: El backend apunta al PocketID existente
- **Given** el compose de producción sin servicio `pocketid`
- **When** el servicio `valet` arranca con `AUTH_ENABLED=true` y `AUTH_ISSUER_URL` público
- **Then** resuelve el discovery del proveedor en `AUTH_ISSUER_URL`
- **And** el `iss` del ID token coincide con `AUTH_ISSUER_URL`

#### Scenario: El operador registra el cliente en su PocketID
- **Given** un PocketID ya desplegado en el VPS
- **When** el operador sigue la documentación
- **Then** registra Redirect URI `https://${APP_HOST}/api/auth/callback`
- **And** registra Post-logout URI `https://${APP_HOST}`

### Requirement: La documentación describe el despliegue tras Traefik con PocketID externo

`README.md`, `README.es.md`, `.env.example` y `.env.prod.sample` SHALL documentar que el stack de producción es **un único servicio** (`valet`, `Dockerfile` monolítico) y las variables del despliegue (`APP_HOST`, `TRAEFIK_NETWORK`, `TRAEFIK_CERT_RESOLVER`), el nombre DNS de la aplicación, la red externa de Traefik y las redirect/post-logout URIs que deben registrarse en el PocketID existente, de modo que un operador pueda configurar el stack sin leer el código.

#### Scenario: Un operador configura el stack solo con la documentación
- **Given** un VPS con Traefik y PocketID ya desplegados y un nombre DNS apuntando al VPS
- **When** el operador sigue `README.md` y `.env.example`
- **Then** conoce cada variable necesaria y su significado
- **And** entiende que el stack es un único servicio construido desde `Dockerfile`
- **And** sabe qué redirect URI y post-logout URI registrar en su cliente OIDC de PocketID
- **And** `.env.prod.sample` lista, comentada, cada variable que consume `docker-compose.prod.yml`
