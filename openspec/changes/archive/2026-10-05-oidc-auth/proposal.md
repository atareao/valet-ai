# Proposal

## Why

Valet expone hoy toda su API sin autenticación: la variable `AUTH_ENABLED` existe pero no tiene ningún efecto y el stub de `src/auth.rs` decodifica el JWT sin verificar la firma. Desplegado en producción, cualquiera con acceso a la URL puede leer y modificar todos los datos de la instancia. Se necesita una puerta de entrada OIDC real (PocketID) antes de exponer el servicio.

## What Changes

- Sustituir el stub de autenticación por un flujo OIDC completo (Authorization Code + cookie HttpOnly firmada con JWT HS256); el token del proveedor nunca llega a JavaScript.
- Retener el ID token emitido por el proveedor dentro de la sesión para construir el logout SSO con `id_token_hint`, y añadir la variable `AUTH_POST_LOGOUT_REDIRECT_URL` para el `post_logout_redirect_uri`.
- Añadir las rutas `GET /api/auth/login`, `GET /api/auth/callback`, `POST /api/auth/logout` y `GET /api/auth/me`.
- Añadir middleware de sesión: con `AUTH_ENABLED=true`, todas las rutas `/api/*` salvo `/api/health` y `/api/auth/*` exigen sesión válida (401); con `AUTH_ENABLED=false` se permite un usuario dev por defecto.
- Endurecer CORS: al usar credenciales se usan orígenes explícitos y ya no `allow_origin(Any)`.
- Frontend: pantalla de login que redirige a `/api/auth/login`, credenciales incluidas en todas las peticiones, redirección global a login ante un 401 y botón de logout SSO en el header.
- **BREAKING**: sustituir `allow_origin(Any)` por orígenes explícitos rompe los clientes cross-origin que hasta ahora hacían peticiones sin credenciales.
- Infra de producción: crear `Dockerfile.backend` y `Dockerfile.frontend` (hoy referenciados por `docker-compose.prod.yml` pero inexistentes), añadir `nginx.conf` y propagar `AUTH_REDIRECT_URL` en el stack de producción.

## Capabilities

### New Capabilities
- `oidc-auth`: flujo de inicio de sesión OIDC, sesión transportada en cookie firmada, endpoints de autenticación y enforcement de sesión sobre la API.

### Modified Capabilities
- `infra`: stack de producción con servicios separados (backend + frontend nginx + PocketID), los Dockerfiles que faltan, `nginx.conf` y las variables de entorno de autenticación.
- `frontend`: puerta de entrada (login/logout), credenciales en las peticiones y manejo global de la sesión expirada.

## Impact

- `src/auth.rs` (reescritura), nuevo módulo de rutas/middleware de auth, `src/lib.rs` (CORS y capas) y `AppState.auth_config`, que pasa a leerse.
- `Cargo.toml`: nuevas dependencias (`jsonwebtoken`, cookies de `axum-extra`, cliente OIDC/JWKS vía `reqwest`).
- Frontend: `App.tsx`, `api/client.ts`, `AppLayout.tsx` y nuevos componentes/contexto de sesión.
- Infra: `docker-compose.prod.yml`, `Dockerfile.backend`, `Dockerfile.frontend`, `nginx.conf` y documentación de variables de entorno (`AUTH_POST_LOGOUT_REDIRECT_URL`, default la raíz pública de la app, leída en `AuthConfig`/`Config`).
- No cambia el esquema de datos: el modelo sigue siendo single-user (sin `user_id` en las tablas).
