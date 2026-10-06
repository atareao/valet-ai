# Design

## Context

Ver `proposal.md` — Why. Restricciones y hechos que condicionan el diseño:

- El repo construye SPA + API con **un único `Dockerfile`** multi-stage: el frontend Vite se compila y se copia a `/app/static`, y el binario musl lo sirve con `ServeDir` + fallback a `index.html` (`src/lib.rs:469`) en el puerto `3000`. `docker-compose.yml` (dev) levanta un único servicio `valet` con ese `Dockerfile`.
- El `docker-compose.prod.yml` de `development` levantaba `backend` + `frontend` (nginx) + `pocketid` y referenciaba `Dockerfile.backend`/`Dockerfile.frontend` que **nunca existieron en `development`**.
- La cookie de sesión y la de flujo se emiten siempre `Secure` (`src/routes/auth.rs`), así que el acceso debe ser HTTPS.
- El callback redirige a una ruta relativa (`/`), por lo que no depende de cabeceras de proxy para construir URLs absolutas.
- El VPS ya ejecuta **Traefik** terminando TLS y **PocketID ya desplegado**: ninguno de los dos se gestiona desde este repo.

## Goals / Non-Goals

**Goals:**
- Dejar el stack de producción como **un solo servicio** (`valet`, `Dockerfile` monolítico) desplegable tras un Traefik existente, con TLS y sin publicar puertos.
- Consumir el PocketID externo por su issuer público, sin levantarlo.

**Non-Goals:**
- Desplegar, configurar ni enrutar PocketID (ya existe; se asume alcanzable por navegador y backend).
- Gestionar Traefik (red y certresolver asumidos existentes).
- Automatizar DNS ni certificados más allá de las labels.
- Mantener nginx o los Dockerfiles divididos: se eliminan.
- Cambiar código Rust o frontend, ni el `Dockerfile` ni el `docker-compose.yml` de dev.

## Decisions

### Un único servicio con el Dockerfile monolítico

El compose de producción define un solo servicio `valet` con `build.dockerfile: Dockerfile` (el mismo del dev), `container_name`, `restart: unless-stopped`, el volumen `valet_data` y sin `ports:`. Alternativa descartada (y retirada): dos imágenes (`Dockerfile.backend` + `Dockerfile.frontend` con nginx); duplicaba la forma de construir el mismo crate, añadía un proxy inverso innecesario y no era la arquitectura del repo.

### Ingress por Traefik con labels

`valet` se une a la red **externa** `TRAEFIK_NETWORK` (por defecto `traefik`) y declara labels del provider Docker de Traefik: `Host(${APP_HOST})`, entrypoint HTTPS, `tls.certresolver=${TRAEFIK_CERT_RESOLVER}` y `loadbalancer.server.port=3000`. Alternativa descartada: publicar `3000:3000`; perdería TLS y el aislamiento. `APP_HOST` debe ser un **nombre** (sin esquema) que resuelva al VPS.

### PocketID externo: fuera del compose

No se define el servicio `pocketid`. El backend se configura con `AUTH_ISSUER_URL` apuntando al issuer público del PocketID existente; discovery, intercambio de código y JWKS se resuelven contra esa URL. Alternativa descartada: mantener PocketID en el compose; duplicaría el IdP que ya corre en el VPS y fragmentaría los passkeys.

### Sin nginx intermedio

Al servir el propio binario el SPA y la API en `:3000`, desaparece `nginx.conf` y con él el problema de `X-Forwarded-Proto`: ya no hay proxy interno que pise el esquema. Las cookies son `Secure` de por sí y el callback es relativo, así que no hace falta reconstruir URLs absolutas.

## Risks / Trade-offs

- **El backend debe alcanzar `AUTH_ISSUER_URL`** → al ser el issuer público, el contenedor sale por la red del host; si el dominio resuelve al VPS, funciona (hairpin NAT). Verificar en el despliegue.
- **`AUTH_ISSUER_URL` distinto del `iss` de PocketID** → provoca fallo de validación; la documentación recalca que deben coincidir.
- **Nombre/red de Traefik distinta en el VPS** → `TRAEFIK_NETWORK` configurable; el error de red ausente se documenta.
- **La imagen monolítica corre como root** (el `Dockerfile` no declara `USER`) → se acepta: es la imagen que ya se publica y usa el dev; cambiarla es otro cambio.
- **Retirar `ports:` rompe el acceso directo por IP** → intencional; el único ingress es Traefik. Rollback: revertir el compose.
- **Pérdida de `nginx.conf`** → no hay pérdida funcional: el `ServeDir` con fallback a `index.html` cubre las rutas de cliente de React Router y el streaming/SSE lo sirve Axum directamente.

## Migration Plan

1. Reescribir `docker-compose.prod.yml` (servicio único) y eliminar `Dockerfile.backend`, `Dockerfile.frontend`, `nginx.conf`; actualizar docs; validar con `podman compose -f docker-compose.prod.yml config`.
2. En el VPS: definir `.env` (`APP_HOST`, `AUTH_ISSUER_URL`, `AUTH_CLIENT_ID`, `AUTH_CLIENT_SECRET`, `AUTH_REDIRECT_URL`, `AUTH_POST_LOGOUT_REDIRECT_URL`, `JWT_SECRET`, `TRAEFIK_NETWORK`, `TRAEFIK_CERT_RESOLVER`), DNS y cliente OIDC en el PocketID existente (Redirect URI `https://${APP_HOST}/api/auth/callback`, Post-logout `https://${APP_HOST}`).
3. `podman compose -f docker-compose.prod.yml up -d --build`; verificar la tarea 6.4 (login → `me` → rutas → logout SSO).
4. Rollback: revertir el compose (volver a publicar puertos) o `AUTH_ENABLED=false` para desactivar el enforcement sin revertir la imagen.

## Open Questions

- Nombre del entrypoint HTTPS de Traefik (por defecto `websecure`): configurable, no cambia specs ni tareas.
