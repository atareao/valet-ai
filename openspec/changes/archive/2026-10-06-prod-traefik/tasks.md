# Tasks

## 1. Compose de producción con un único Dockerfile

- [x] 1.1 Reescribir `docker-compose.prod.yml`: **un único servicio `valet`** con `build: { context: ., dockerfile: Dockerfile }`, `container_name: valet_valet`, `restart: unless-stopped`, volumen `valet_data` y **sin `ports:`**. Verificar con `podman compose -f docker-compose.prod.yml config` que hay un solo servicio, que NO existen `backend`/`frontend`/`pocketid` y que no hay ningún `published`.
- [x] 1.2 Unir `valet` a la red **externa** `TRAEFIK_NETWORK` (por defecto `traefik`, `external: true`) y declarar las labels de Traefik (`Host(${APP_HOST})`, entrypoint HTTPS, `tls.certresolver=${TRAEFIK_CERT_RESOLVER}`, `loadbalancer.server.port=3000`). Verificar con `podman compose config` que las labels aparecen con los valores de entorno.
- [x] 1.3 Propagar al servicio las variables de auth (`AUTH_ENABLED`, `AUTH_ISSUER_URL`, `AUTH_CLIENT_ID`, `AUTH_CLIENT_SECRET`, `AUTH_REDIRECT_URL`, `AUTH_POST_LOGOUT_REDIRECT_URL`, `JWT_SECRET`) y `DATABASE_URL` apuntando a la ruta del volumen. Verificar con `podman compose config`.
- [x] 1.4 **Eliminar** `Dockerfile.backend`, `Dockerfile.frontend` y `nginx.conf`. Verificar que no queda ninguna referencia (`rg 'Dockerfile\.(backend|frontend)|nginx\.conf'` solo puede encontrar openspec y la propia documentación histórica).
- [x] 1.5 Actualizar la cabecera comentada de `docker-compose.prod.yml`: un solo servicio (`Dockerfile` monolítico), topología Traefik, `APP_HOST` **sin esquema**, `AUTH_ISSUER_URL` al PocketID externo y las redirect/post-logout URIs. Verificar que el ejemplo de `APP_HOST` no lleva `https://`.

## 2. Documentación de despliegue

- [x] 2.1 Actualizar `.env.example` y `.env.j2`: describir el stack como **un único servicio** y conservar `APP_HOST`, `TRAEFIK_NETWORK` y `TRAEFIK_CERT_RESOLVER`; eliminar cualquier mención a `frontend`, `backend` o nginx como servicios. Verificar con `grep`.
- [x] 2.3 Crear `.env.prod.sample`: plantilla con cada variable que consume `docker-compose.prod.yml` (APP_HOST, TRAEFIK_*, AUTH_*, JWT_SECRET, LLM/embeddings y APIs externas), comentada, con la pista `openssl rand -hex 32` para `JWT_SECRET`. Verificar con `rg -o '\$\{[A-Z_]+' docker-compose.prod.yml | sort -u` que no falta ninguna.
- [x] 2.2 Reescribir la sección de Producción de `README.md` y `README.es.md`: un solo servicio (`valet`, `Dockerfile` monolítico) tras un Traefik existente y **PocketID externo ya desplegado** (solo registrar las URIs del cliente OIDC y fijar `AUTH_ISSUER_URL`). Verificar que ambas secciones ya no mencionan `Dockerfile.backend`/`Dockerfile.frontend`/nginx y sí la red de Traefik y las dos URIs.

## 3. Verificación de integración

- [x] 3.1 Crear la red externa si no existe (`podman network create traefik`) y validar el stack con `podman compose -f docker-compose.prod.yml config`; confirmar que no hay errores ni variables sin resolver.
- [x] 3.2 Ejecutar `openspec validate prod-traefik` (verde) y comprobar que el change no toca código Rust ni frontend, por lo que `just check-all` no debe presentar regresiones.
