# Proposal

## Why

El `docker-compose.prod.yml` de producción se escribió para una arquitectura de **dos imágenes** (`Dockerfile.backend` + `Dockerfile.frontend` con nginx) que **no es la del proyecto**: el repositorio construye SPA + API con **un único `Dockerfile`** multi-stage (frontend embebido en `/app/static`, servido por el propio binario en `:3000`), que es exactamente el que usa `docker-compose.yml` en desarrollo. Peor aún: el `docker-compose.prod.yml` que está en `development` referencia `Dockerfile.backend`/`Dockerfile.frontend` **que nunca existieron allí**, así que el compose de producción de la rama principal está roto.

A eso se suman dos hechos del VPS de destino: la cookie de sesión se emite siempre `Secure` (`src/routes/auth.rs`) y el TLS lo termina un **Traefik ya existente**; y **PocketID ya está desplegado**, de modo que el stack no debe levantarlo, solo consumirlo como IdP externo.

## What Changes

- Sustituir el stack de dos imágenes por **un único servicio `valet`** que construye el `Dockerfile` monolítico y sirve SPA + API en `:3000`.
- **Eliminar** `Dockerfile.backend`, `Dockerfile.frontend` y `nginx.conf`: no pertenecen a la arquitectura del repo.
- Publicar el servicio a través de un **Traefik existente** (red externa `TRAEFIK_NETWORK` + labels con `Host(${APP_HOST})` y TLS por `certresolver`), **sin publicar puertos** en el host.
- Dejar **PocketID fuera del stack**: es un IdP externo que el backend consume vía `AUTH_ISSUER_URL` (issuer público).
- Documentar el despliegue en `README.md`, `README.es.md`, `.env.example` y `.env.j2`.
- **BREAKING** (topología): el stack pasa a un solo servicio, ya no publica puertos y requiere una red externa de Traefik.

## Capabilities

### New Capabilities
- (ninguna)

### Modified Capabilities
- `infra`: el compose de producción pasa de tres servicios (backend + frontend nginx + PocketID) a uno solo con el `Dockerfile` monolítico tras Traefik; se retiran los requisitos de los Dockerfiles divididos y de nginx; y se actualiza la documentación de despliegue.

## Impact

- `docker-compose.prod.yml`: un solo servicio `valet` (`build.dockerfile: Dockerfile`), red externa de Traefik + labels, sin `ports:`.
- Se **eliminan** `Dockerfile.backend`, `Dockerfile.frontend` y `nginx.conf`.
- `.env.example`, `.env.j2`, `README.md`, `README.es.md`: variables y guía de despliegue.
- `Dockerfile` y `docker-compose.yml` (dev) **no se tocan**. Sin cambios en código Rust ni frontend. El modelo sigue siendo single-user.
