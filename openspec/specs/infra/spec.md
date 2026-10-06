# Infraestructura: Docker y Despliegue

## Purpose

Infraestructura de despliegue de Valet: empaquetado Docker multi-stage que compila el backend Rust (musl) y el frontend Vite, embebiendo los assets en un único binario, además del compose de desarrollo y el stack TLS basado en rustls.

## Contratos

### Docker Compose (docker-compose.yml)

```yaml
services:
  valet:
    build:
      context: .
      dockerfile: Dockerfile
    ports: ["3000:3000"]
    volumes:
      - valet_data:/app/data
    environment:
      - DATABASE_URL
      - RUST_LOG
      - OPENROUTER_API_KEY
      - OPENWEATHER_API_KEY
      - AUTH_ENABLED
```

### Dockerfile (multi-stage)

```dockerfile
# Stage 1: Backend (Rust)
FROM docker.io/library/rust:1.98.1-alpine3.21 AS backend-builder
RUN apk add --no-cache build-base musl-dev pkgconfig
WORKDIR /build
RUN cargo init --bin --name valet . && echo "pub fn dummy() {}" > src/lib.rs
COPY Cargo.toml Cargo.lock ./
RUN cargo build --release && rm -rf src
COPY src ./src
RUN touch src/main.rs src/lib.rs && cargo build --release && strip target/release/valet

# Stage 2: Frontend (Node)
FROM docker.io/library/node:22-alpine AS frontend-builder
WORKDIR /build
COPY frontend/package.json frontend/package-lock.json ./
RUN npm ci
COPY frontend/ ./
RUN CI=true npm run build

# Stage 3: Runtime (Alpine)
FROM alpine:3.21
RUN apk add --no-cache ca-certificates && rm -rf /var/cache/apk/*
WORKDIR /app
COPY --from=backend-builder /build/target/release/valet /app/valet
COPY --from=backend-builder /build/target/release/valet-reindex /app/valet-reindex
COPY --from=frontend-builder /build/dist /app/static
EXPOSE 3000
CMD ["/app/valet"]
```

Tamaño final de imagen: ~30 MB (binario musl ~8 MB + assets frontend compilados).

### TLS stack (rustls)

```toml
reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls"] }
# → rustls puro (100% Rust, sin dependencias C nativas)
# → NO depende de openssl-sys, native-tls, ni libssl-dev
```

## Requirements

### Requirement: Docker build produce imagen con frontend embebido

El build SHALL producir una imagen con el frontend compilado embebido en `/app/static` y el binario musl respondiendo en el puerto 3000.

#### Scenario: Docker build produce imagen con frontend embebido
**Given** el directorio del proyecto con src/, Cargo.toml, Cargo.lock y frontend/  
**When** se ejecuta `docker build -t valet .`  
**Then** la imagen se construye sin errores  
**And** el directorio /app/static/ contiene index.html y assets JS/CSS compilados  
**And** el binario compilado con musl responde en el puerto 3000  
**And** la imagen pesa ~30 MB

### Requirement: Docker compose levanta un único servicio

`docker compose up -d` SHALL levantar únicamente el servicio `valet`, que sirve la API y el SPA en el puerto 3000.

#### Scenario: Docker compose levanta un único servicio
**Given** docker-compose.yml con las variables de entorno necesarias  
**When** se ejecuta `docker compose up -d`  
**Then** el servicio valet responde en localhost:3000/api/health  
**And** el servicio valet sirve el frontend SPA en localhost:3000/  
**And** no existe el servicio frontend en el compose

### Requirement: reqwest usa rustls-tls (sin openssl)

La dependencia `reqwest` SHALL usar `rustls-tls` sin depender de openssl.

#### Scenario: reqwest usa rustls-tls (sin openssl)
**Given** Cargo.toml con `default-features = false, features = ["json", "rustls-tls"]`  
**When** se ejecuta `cargo tree -i openssl-sys`  
**Then** el comando devuelve "package ID specification openssl-sys matched no packages"  
**And** la compilación no requiere openssl-dev ni libssl-dev  
**And** las peticiones HTTPS funcionan correctamente

### Requirement: CI SHALL validate the backend on every pull request

En cada `pull_request` contra `development` o `main`, y en cada `push` a esas ramas,
GitHub Actions SHALL ejecutar en un runner limpio `cargo fmt --all -- --check`,
`cargo clippy --all-targets -- -D warnings` y `cargo test`, con la toolchain fijada en
`rust-toolchain.toml`; SHALL fallar si cualquiera de los tres falla; y NO SHALL requerir
servicios externos.

**Given** un `pull_request` contra `development` o `main`
**When** el job `backend` arranca en un runner limpio
**Then** SHALL ejecutarse `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings` y `cargo test`
**And** SHALL fallar si cualquiera de los tres falla
**And** NO SHALL requerir servicios externos

#### Scenario: Formato incorrecto bloquea el PR
**Given** una rama con código que no cumple `cargo fmt`
**When** se abre un pull_request contra `development`
**Then** el job `backend` SHALL fallar en `cargo fmt --all -- --check`

#### Scenario: Tests pasan sin infraestructura externa
**Given** un runner limpio sin base de datos levantada
**When** el job `backend` ejecuta `cargo test`
**Then** todos los tests SHALL pasar porque usan SQLite en memoria

### Requirement: CI SHALL validate the frontend on every pull request

En los mismos disparadores, SHALL ejecutarse con Node 22 sobre `frontend/`: `npm ci`,
`npx tsc --noEmit`, `npm run lint:ci`, `npx vitest run` y `npm run build`; SHALL instalar
con `npm ci` (no `npm install`) para respetar el lockfile.

**Given** un `pull_request` contra `development` o `main`
**When** el job `frontend` arranca
**Then** SHALL instalar con `npm ci` y ejecutar tipos, lint, tests y build
**And** NO SHALL usar `npm install`

#### Scenario: Error de tipos bloquea el PR
**Given** una rama con un error de tipos en el frontend
**When** el job `frontend` ejecuta `npx tsc --noEmit`
**Then** el job SHALL fallar

#### Scenario: Test del frontend bloquea el PR
**Given** una rama con un test del frontend roto
**When** el job `frontend` ejecuta `npx vitest run`
**Then** el job SHALL fallar

#### Scenario: El build de producción forma parte de la validación
**Given** una rama cuyo frontend compila en tests pero no en producción
**When** el job `frontend` ejecuta `npm run build`
**Then** el job SHALL fallar, porque el build de producción SHALL terminar correctamente y no basta con que pasen los tests

### Requirement: CI SHALL fail when the ESLint warning budget is exceeded

ESLint SHALL ejecutarse con `--max-warnings 11`, el recuento congelado en el momento de
este cambio (0 errores); superar 11 SHALL hacer fallar el job; el presupuesto NO SHALL
subir, y bajar el número real de warnings sin bajar el presupuesto se considera aceptable
pero no obligatorio.

**Given** la configuración de ESLint del frontend
**When** el job `frontend` ejecuta `npm run lint:ci`
**Then** SHALL fallar si los warnings superan 11
**And** el presupuesto NO SHALL subir

#### Scenario: Un warning nuevo rompe el build
**Given** un cambio que añade un warning y deja el total en 12
**When** se ejecuta `npm run lint:ci`
**Then** el job SHALL fallar

#### Scenario: Los 11 preexistentes no bloquean
**Given** el estado actual con exactamente 11 warnings y 0 errores
**When** se ejecuta `npm run lint:ci`
**Then** el job SHALL pasar

### Requirement: CI SHALL build the container image and verify it boots

En cada `push` a `main` y en cada disparo manual, SHALL construirse la imagen con el
`Dockerfile` del repo y SHALL ejecutarse el contenedor resultante; el job SHALL fallar si
la imagen no construye, si el contenedor no responde `200` en `/api/health`, si `db` no es
`connected` o si la `version` reportada no coincide con la de `Cargo.toml`.

**Given** un `push` a `main` o un disparo manual
**When** el job de imagen construye la imagen y arranca el contenedor
**Then** SHALL verificar `/api/health` con `200`, `db = connected` y la versión del paquete
**And** SHALL fallar si alguna de las tres condiciones no se cumple

#### Scenario: Una imagen que no arranca falla el job
**Given** una imagen cuyo contenedor no llega a responder
**When** el sondeo de `/api/health` agota su límite
**Then** el job SHALL fallar y SHALL volcar `docker logs valet-ci`

#### Scenario: Una imagen sana pasa el job
**Given** una imagen cuyo contenedor responde `status ok` y `db connected`
**When** la versión reportada coincide con la de `Cargo.toml`
**Then** el job SHALL pasar

#### Scenario: El disparo manual permite probar la rama
**Given** una rama distinta de `main`
**When** se ejecuta `workflow_dispatch` sobre esa rama
**Then** el job de imagen SHALL ejecutarse sobre esa rama

### Requirement: Cargo.lock SHALL be versioned so the container build works from a clean clone

`Cargo.lock` SHALL estar trackeado en git y NO SHALL figurar en `.gitignore`; el build del
contenedor SHALL poder ejecutarse desde un clon limpio del repositorio, donde
`COPY Cargo.toml Cargo.lock ./` SHALL encontrar el fichero; los builds SHALL ser
reproducibles a partir del lock.

**Given** el repositorio con `Cargo.lock` trackeado
**When** se clona limpio y se construye la imagen
**Then** el `COPY Cargo.toml Cargo.lock ./` SHALL encontrar el fichero
**And** los builds SHALL ser reproducibles a partir del lock

#### Scenario: Un clon limpio contiene el lock
**Given** `Cargo.lock` trackeado en git
**When** se ejecuta `git clone` y después el build de la imagen
**Then** `Cargo.lock` existe en el clon y el `COPY` del `Dockerfile` no falla

#### Scenario: Sin el lock el build falla
**Given** un checkout sin `Cargo.lock`
**When** se construye la imagen
**Then** el build SHALL fallar con un error de `COPY`, que es exactamente el fallo que motivó este cambio

### Requirement: The Rust toolchain SHALL be pinned and shared by local, CI and the image

El repo SHALL declarar `rust-toolchain.toml` con una versión concreta y los componentes
`rustfmt` y `clippy`; la CI SHALL usar esa versión y NO SHALL declarar una distinta; la
imagen SHALL construirse con un tag de compilador fijado, no con un tag móvil; las tres
SHALL ser la misma versión.

**Given** `rust-toolchain.toml` en la raíz con una versión concreta
**When** la CI y la imagen usan la toolchain
**Then** SHALL instalarse la versión exacta del fichero
**And** la imagen SHALL fijar la versión completa del compilador

#### Scenario: La CI usa la versión del fichero
**Given** `rust-toolchain.toml` en la raíz
**When** el job `backend` configura Rust sin input `toolchain`
**Then** SHALL instalarse la versión exacta del fichero

#### Scenario: La imagen no usa un tag móvil
**Given** el `Dockerfile`
**When** se inspecciona su `FROM`
**Then** SHALL fijar la versión completa del compilador, no un tag móvil como `alpine3.21`

### Requirement: just deploy SHALL build, restart and verify the service

`just deploy` SHALL descargar la imagen publicada en `ghcr.io/atareao/valet-ai` y recrear
el servicio; `just deploy-local` SHALL construir la imagen en local y recrear el servicio;
SHALL existir además `just build` y `just health`. Ambas recetas de deploy SHALL esperar de
forma acotada a que `/api/health` responda `status ok` con `db connected`, SHALL terminar
con código de salida distinto de cero si el servicio no queda sano, SHALL informar de la
versión reportada por el servicio y SHALL comprobar que el contenedor en ejecución usa la
imagen publicada y no una construida en local. `docker-compose.yml` SHALL declarar `image:`
con la ruta de GHCR además del `build:` existente.

**Given** el `justfile` con `build`, `deploy`, `deploy-local` y `health`
**When** se ejecuta `just deploy`
**Then** SHALL descargar la imagen publicada y recrear el servicio
**And** SHALL esperar de forma acotada a `/api/health` con `status ok` y `db connected`
**And** SHALL informar de la versión reportada por el servicio

#### Scenario: Un deploy sano informa de la versión
**Given** una imagen publicada que arranca correctamente
**When** se ejecuta `just deploy`
**Then** el healthcheck SHALL llegar a verde y SHALL informarse la versión del servicio

#### Scenario: Un servicio que no arranca falla el deploy
**Given** una imagen cuyo contenedor no queda sano
**When** se ejecuta `just deploy`
**Then** la receta NO SHALL devolver éxito al agotarse el límite del healthcheck

#### Scenario: El deploy no compila nada
**Given** el `justfile` con `deploy` y `deploy-local`
**When** se ejecuta `just deploy`
**Then** SHALL descargar la imagen publicada con `podman pull`
**And** NO SHALL compilar el backend ni el frontend en la máquina
**And** el contenedor en ejecución SHALL ser la imagen publicada, no una local

### Requirement: just check-all SHALL cover the same checks as CI

`just check-all` SHALL incluir formato, clippy, tests Rust, lint del frontend, tests del
frontend y build del frontend, de forma que la verificación local cubra lo mismo que la CI
y un fallo se detecte antes de abrir el PR.

**Given** el `justfile`
**When** se ejecuta `just check-all`
**Then** SHALL ejecutar `fmt`, `clippy`, `test`, `frontend-lint`, `frontend-test` y `frontend-check`
**And** un fallo SHALL detectarse antes de abrir el PR

#### Scenario: Un test del frontend roto falla check-all
**Given** un test del frontend roto
**When** se ejecuta `just check-all`
**Then** SHALL fallar, mientras que antes `check-all` terminaba en verde sin ejecutar los tests del frontend

### Requirement: The health endpoint SHALL report the real package version

`/api/health` SHALL informar en `version` la versión declarada en `Cargo.toml` del propio
paquete, NO SHALL contener ningún literal de versión escrito a mano, y el test SHALL
comparar contra la versión del paquete en lugar de contra un literal.

**Given** el paquete en la versión declarada en `Cargo.toml`
**When** se consulta `/api/health`
**Then** `version` SHALL ser la del paquete
**And** NO SHALL haber ningún literal de versión escrito a mano en el handler

#### Scenario: La versión coincide con Cargo.toml
**Given** el paquete en `0.9.0`
**When** se consulta `/api/health`
**Then** la respuesta devuelve `version = 0.9.0`

#### Scenario: Subir la versión no requiere tocar el handler
**Given** un handler que usa la versión del paquete en lugar de un literal
**When** se sube la versión del paquete
**Then** la respuesta cambia sola y el test sigue en verde, a diferencia del literal `0.1.0` anterior

### Requirement: CI SHALL publish the verified image to GitHub Container Registry

En cada `push` a `main` y en cada tag `v*`, la imagen SHALL publicarse en
`ghcr.io/atareao/valet-ai` usando el token del workflow, sin PATs ni secretos añadidos, y el
job SHALL declarar `permissions: packages: write`. La publicación SHALL ocurrir únicamente
DESPUÉS de que el smoke test haya pasado: un smoke test fallido NO SHALL publicar ninguna
imagen. En un push a `main` los tags SHALL ser `latest` y `sha-<corto>`; en un tag `vX.Y.Z`
los tags SHALL ser `vX.Y.Z`, `X.Y` y `latest`; en un disparo manual SHALL publicarse
únicamente `sha-<corto>` y `latest` NO SHALL moverse desde una rama. La publicación SHALL
ser solo para `amd64`.

**Given** un push a `main` o un tag `v*` cuyo smoke test ha pasado
**When** el job publica la imagen
**Then** SHALL subir la misma imagen verificada a `ghcr.io/atareao/valet-ai`
**And** SHALL usar el token del workflow con `packages: write`
**And** NO SHALL publicar nada si el smoke test ha fallado

#### Scenario: Una imagen que no arranca no se publica
**Given** un smoke test que falla
**When** el job termina
**Then** NO SHALL existir ninguna imagen nueva en el registro

#### Scenario: Un push a main publica latest y el sha
**Given** un push a `main` que pasa el smoke test
**When** la imagen se publica
**Then** SHALL existir el tag `latest`
**And** SHALL existir un tag con los 7 primeros caracteres del SHA

#### Scenario: Un tag de release publica la versión y mueve latest
**Given** un tag `v0.9.0` que pasa el smoke test
**When** la imagen se publica
**Then** SHALL existir el tag `v0.9.0`
**And** SHALL existir el tag `0.9`
**And** `latest` SHALL apuntar a esa misma imagen

#### Scenario: Un disparo manual desde una rama no mueve latest
**Given** un `workflow_dispatch` sobre una rama distinta de `main`
**When** el job publica la imagen
**Then** SHALL publicarse únicamente el tag con el SHA
**And** `latest` NO SHALL moverse

### Requirement: The command runner file SHALL be named .justfile for consistency with the author's other repositories

El fichero de recetas SHALL llamarse `.justfile` (con punto) en la raíz del repo y NO SHALL
existir simultáneamente un `justfile` sin punto; el renombrado SHALL hacerse con `git mv`
para conservar el historial; `just` SHALL seguir resolviendo las recetas del proyecto y NO
SHALL caer en las del directorio padre; y NO SHALL reescribirse las menciones históricas a
`justfile` que viven en `openspec/changes/archive/`, que son registro histórico.

**Given** el repo con `justfile` en la raíz
**When** se renombra a `.justfile` con `git mv`
**Then** SHALL existir `.justfile` y NO SHALL existir `justfile`
**And** `just` SHALL resolver las recetas del proyecto
**And** el historial de git SHALL conservarse

#### Scenario: just resuelve las recetas del proyecto tras el renombrado
**Given** el repo con `.justfile` en la raíz y un `.justfile` en el directorio padre
**When** se ejecuta `just --list` desde la raíz del proyecto
**Then** SHALL listarse las recetas del proyecto, incluidas `deploy` y `deploy-local`
**And** NO SHALL listarse las recetas del directorio padre

#### Scenario: No queda un justfile sin punto
**Given** el renombrado aplicado
**When** se inspecciona la raíz del repo
**Then** NO SHALL existir `justfile` ni `Justfile`
**And** `git ls-files` SHALL mostrar `.justfile`

#### Scenario: El historial no se reescribe
**Given** los changes archivados que mencionan `justfile`
**When** se aplica el renombrado
**Then** dichas menciones NO SHALL modificarse
**And** el renombrado SHALL afectar solo al nombre del fichero

### Requirement: Formato de inspección de imágenes sin caracteres sobrantes

Las recetas de despliegue DEBEN obtener el ID de la imagen en ejecución, el ID de la imagen publicada y el nombre de la imagen mediante formatos de `podman inspect` que se resuelvan exactamente a `{{.Image}}`, `{{.Id}}` y `{{.Config.Image}}`, sin ninguna secuencia de llaves sobrante.

La causa del defecto y su razón de ser: en `just`, `{{{{` se colapsa a `{{` pero `}}}}` no se colapsa, así que un formato escrito como `'{{{{.Image}}}}'` llega a podman como `{{.Image}}}}` y podman imprime el valor con dos llaves adheridas. El formato DEBE declararse una sola vez como variable de `just` e interpolarse, para que no haya llaves literales en las recetas.

#### Scenario: La interpolación produce el formato limpio

- **Given** una variable de `just` declarada como `podman_fmt_image := '{{.Image}}'`
- **When** una receta la interpola con `--format '{{ podman_fmt_image }}'`
- **Then** el argumento que recibe `podman` es exactamente `{{.Image}}`, sin llaves añadidas

#### Scenario: El ID se imprime sin llaves adheridas

- **Given** un contenedor en ejecución con una imagen publicada
- **When** la receta imprime el ID de la imagen en ejecución
- **Then** la salida es un `sha256` limpio, sin ninguna llave al final

#### Scenario: El `.justfile` no contiene llaves de cierre duplicadas en las recetas

- **Given** el `.justfile` completo
- **When** se inspeccionan las líneas de las recetas
- **Then** ninguna línea de receta contiene la secuencia de cuatro llaves de cierre

### Requirement: La comprobación de imagen sigue detectando discordancias

La receta `deploy` DEBE seguir fallando cuando el contenedor en ejecución no corresponde a la imagen publicada, y DEBE seguir pasando cuando coinciden. El arreglo del formato NO DEBE alterar esta lógica.

#### Scenario: El contenedor usa la imagen publicada

- **Given** un contenedor recreado a partir de la imagen publicada
- **When** se comparan ambos IDs
- **Then** coinciden y la receta continúa hacia la verificación de salud

#### Scenario: El contenedor usa otra imagen

- **Given** un contenedor cuya imagen en ejecución no es la publicada
- **When** se comparan ambos IDs
- **Then** la receta falla, muestra ambos IDs limpios y no continúa

### Requirement: El stack de producción arranca sano

Con el stack de producción levantado (`valet` construido desde el `Dockerfile` monolítico y unido a la red externa de Traefik), `/api/health` SHALL responder `200` y la aplicación SHALL ser alcanzable a través de Traefik en `https://${APP_HOST}`, sin publicar ningún puerto en el host.

#### Scenario: El health del stack responde tras el arranque
- **Given** el stack de producción levantado con `podman compose -f docker-compose.prod.yml up -d`
- **When** se consulta `/api/health` a través de Traefik
- **Then** responde `200` con `status ok`
- **And** el SPA responde en `https://${APP_HOST}/`
- **And** el servicio `valet` no publica ningún puerto en el host

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
