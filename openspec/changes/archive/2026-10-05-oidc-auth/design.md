# Design

## Context

Hoy el backend arranca con `AppState.auth_config: Option<AuthConfig>` construido desde el entorno, pero nunca se lee. `src/auth.rs` solo contiene un stub de fase RED que decodifica el payload de un JWT sin verificar la firma, y `src/lib.rs` aplica únicamente un `CorsLayer` con `allow_origin(Any)`. Las dependencias del crate no incluyen ninguna librería JWT, de cookies ni de cliente OIDC. En el frontend, `api/client.ts` no envía credenciales, `App.tsx` no tiene guard y `AppLayout.tsx` no tiene logout. El stack de producción (`docker-compose.prod.yml`) referencia `Dockerfile.backend` y `Dockerfile.frontend` que no existen y no hay `nginx.conf`.

La motivación y el alcance están en `proposal.md`; el comportamiento exigido, en `specs/`. Este documento sólo decide el "cómo".

## Goals / Non-Goals

**Goals:**
- Implementar el RP OIDC server-side con Authorization Code, validación de ID token y sesión propia.
- Reutilizar `AppState.auth_config` como única fuente de configuración de auth.
- Mantener intacto el modelo single-user: la identidad autenticada protege la puerta, no particiona datos.
- Dejar el stack de producción reproducible con Podman/Docker.

**Non-Goals:**
- Multi-usuario, `user_id` por fila o migraciones de esquema.
- Refresh tokens, rotación de sesión o revocación centralizada.
- Roles/permisos granulares: quien se autentica ve toda la instancia.
- Cambiar `Dockerfile` (mono-servicio de desarrollo) ni `docker-compose.yml`.

## Decisions

### Dependencias Rust

- **Sesión propia**: `jsonwebtoken` (HS256) para emitir y validar la cookie de sesión. Alternativa descartada: construir la firma a mano; evita errores de criptografía y aporta validación de `exp`/`iat` de serie.
- **Cookies**: `axum-extra` con la feature `cookie`, que ofrece `CookieJar` y `PrivateCookieJar` integrados con Axum 0.8. Alternativa descartada: `tower-cookies`; funciona, pero `axum-extra` encaja mejor con los extractores y evita una capa extra.
- **Cliente OIDC**: implementación manual con el `reqwest` ya presente (discovery en `/.well-known/openid-configuration`, intercambio del código en el token endpoint y JWKS en `jwks_uri`) en lugar del crate `openidconnect`. Racional: `openidconnect` arrastra `oauth2`, una API asíncrona más pesada y una superficie mayor para un único proveedor (PocketID); una implementación manual reutiliza `reqwest`/`serde` ya en el árbol y es testeable con `wiremock` (ya en dev-dependencies). La firma del ID token se valida seleccionando del JWKS la clave cuyo `kid` coincide con el header y verificándola con `jsonwebtoken` (`DecodingKey::from_rsa_components` o `from_jwk`).
- Caché de discovery/JWKS: `tokio::sync::RwLock`/`OnceCell` en el estado para no pedir el discovery en cada callback.

### Módulos

- `src/auth.rs`: reescritura del módulo. Conserva `AuthConfig` y `Claims`, y añade: emisión/validación de la cookie de sesión, generación de `state`/`nonce`, discovery/JWKS, intercambio de código y validación del ID token. Se elimina `extract_user_id` (stub sin verificación).
- `src/routes/auth.rs`: handlers `login`, `callback`, `logout`, `me`, exportando `routes()` para encadenarse como el resto de módulos de rutas.
- `src/middleware/auth.rs` (o `src/auth/middleware.rs`): extractor/middleware `from_fn_with_state` que resuelve la sesión desde la cookie y, si `auth.enabled`, corta con 401.
- `src/lib.rs`: registrar `.merge(routes::auth::routes())`, sustituir el `CorsLayer` por uno con origen explícito y credenciales, y aplicar el middleware de auth.

### Configuración

- Se añade `AUTH_POST_LOGOUT_REDIRECT_URL` (default: la raíz pública de la app) al entorno y su campo correspondiente en `AuthConfig`/`Config`; se usa como `post_logout_redirect_uri` del logout SSO.

### Flujo de cookie

1. `login`: `state` y `nonce` aleatorios (CSPRNG, base64url) se guardan en cookies temporales firmadas (o en una cookie de flujo `HttpOnly` con la misma firma) y se redirige a la authorization endpoint.
2. `callback`: se recuperan `state`/`nonce` de esas cookies de flujo, se borran (un solo uso), se valida el `state`, se intercambia el `code` (con `client_secret`), se valida el ID token y se emite la cookie de sesión `valet_session` (`jsonwebtoken` HS256 con `JWT_SECRET`), `HttpOnly; Secure; SameSite=Lax; Path=/; Max-Age=...`, con `sub`, `email`, `name`, `exp` y el claim `id_token` con el ID token del proveedor. Finalmente, `302` al SPA.
3. `me`: lee la cookie de sesión, la valida y devuelve los claims, o `401`.
4. `logout`: elimina la cookie de sesión y construye la URL de cierre SSO `{end_session_endpoint}?id_token_hint=<id_token>&post_logout_redirect_uri=<...>` a partir del ID token retenido en la sesión.

La cookie de sesión es la única credencial que viaja al navegador; el access token del proveedor no se persiste en el cliente. El ID token sí se retiene dentro del JWT de sesión (claim no accesible desde JavaScript) porque `end_session_endpoint` de PocketID exige `id_token_hint`; se acepta este trade-off porque la cookie es `HttpOnly`+`Secure` y el ID token caduca pronto.

**Tamaño de cookie:** un ID token puede ser grande y los navegadores limitan cada cookie a ~4KB. Plan B si el JWT de sesión con el claim `id_token` excede ese límite: mover el ID token a una segunda cookie `HttpOnly`+`Secure` dedicada (`valet_id_token`), emitida y borrada junto con `valet_session`, en lugar de incrustarlo en el JWT.

### Orden del middleware Axum

Aplicar `from_fn_with_state(state.clone(), auth_middleware)` como capa del router de la API, no del fallback estático. El middleware resuelve la exención por ruta (`/api/health`, `/api/auth/*`) antes de exigir sesión. Ubicación recomendada: envolver únicamente las rutas `/api/*` en un `Router` con `.layer(...)` y después `.merge` del `fallback_service` de assets, para que nginx/SPA y health no pasen por la comprobación. El orden de capas importa: CORS por fuera, middleware de auth por dentro, de modo que un preflight `OPTIONS` no sea rechazado con 401.

### CORS

`allow_credentials(true)` con `allow_origin` construido desde el origen del `AUTH_REDIRECT_URL` (explícito). No puede combinarse con `Any`. En modo dev (`auth.enabled=false`) puede mantenerse un origen permisivo pero nunca `credentials + Any`.

### Frontend

- `src/api/client.ts`: añadir `credentials: "include"` en `request` y en las llamadas de streaming (`EventSource` con cookies o `fetch` con credenciales).
- Nuevo `AuthContext`/`useAuth`: al montar llama a `/api/auth/me`; expone `user`, `loading` y `logout`. `App.tsx` decide entre spinner, login y aplicación.
- Nuevo componente `LoginPage` que navega a `/api/auth/login`.
- Detección global de `401`: en el manejo de `ApiError` (status 401) disparar el estado no autenticado/redirección a login.
- `AppLayout.tsx`: botón de logout que llama a `POST /api/auth/logout` y redirige al `end_session_endpoint` devuelto.

### nginx y compose

- `Dockerfile.backend`: stage builder Rust musl (reutiliza el patrón del `Dockerfile` actual) + runtime mínimo; expone 3000.
- `Dockerfile.frontend`: stage Node que compila Vite + stage nginx que copia `dist` y `nginx.conf`.
- `nginx.conf`: `location /` con `try_files $uri $uri/ /index.html`; `location /api/` con `proxy_pass` al backend, `proxy_http_version 1.1`, `proxy_set_header Connection ""`, y `proxy_buffering off` para SSE.
- `docker-compose.prod.yml`: añadir `AUTH_REDIRECT_URL` y `AUTH_POST_LOGOUT_REDIRECT_URL` al backend y mantener los tres servicios; `frontend` depende de `backend`.

### Estrategia de tests

- **Unit (Rust)**: firma/validación de la cookie de sesión, rechazo por expiración y por manipulación; generación y consumo de `state`/`nonce`; validación del ID token con `wiremock` sirviendo discovery/JWKS falsos (firma válida, `iss`/`aud`/`exp`/`nonce` incorrectos).
- **Integración (Rust, `tests/api/auth.rs`)**: `/api/auth/me` sin cookie → 401 y con cookie válida → 200 con claims; enforcement sobre una ruta protegida y exención de `/api/health` y `/api/auth/*`; con `auth.enabled=false` acceso libre como usuario dev. El callback se prueba contra un proveedor simulado con `wiremock`.
- **Frontend (vitest + Testing Library)**: `LoginPage` navega a `/api/auth/login`; `client` envía `credentials: include`; un 401 lleva a login; el botón de logout llama al endpoint y redirige; estado de carga mientras `me` está en curso.
- **Infra**: `docker compose -f docker-compose.prod.yml config` valida la configuración; el smoke test del CI reutiliza `/api/health`.

## Risks / Trade-offs

- **`credentials + CORS` mal configurado rompe todo el frontend** → fijar origen explícito desde `AUTH_REDIRECT_URL` y cubrirlo con el test de CORS existente.
- **SSE a través de nginx con buffering** → `proxy_buffering off` y `Connection ""`; verificar con un evento incremental.
- **`state`/`nonce` en cookies de flujo**: si el navegador bloquea cookies de terceros o el usuario abre el callback en otro contexto, el callback falla; aceptable para app first-party servida bajo el mismo dominio.
- **Reloj/expiración del ID token y de la sesión** → usar `exp` del proveedor con margen; sesión propia con `Max-Age` corto y re-login.
- **Implementación manual del RP OIDC**: más código que mantener frente a `openidconnect`; se mitiga cubriendo firma, `iss`, `aud` y `nonce` con tests y limitando el alcance a un solo proveedor.
- **`AUTH_REDIRECT_URL` no propagado** → ya se corrige en el compose de producción; sin él el `redirect_uri` del callback no coincide con el registrado en PocketID.
- **`post_logout_redirect_uri` no registrado en el proveedor** → algunos proveedores exigen registrar previamente el post-logout URI; si no coincide con el configurado, el cierre SSO puede rechazar la redirección. El `id_token_hint` ya no es un riesgo abierto: la sesión retiene el ID token específicamente para construirlo.

## Migration Plan

1. Implementar backend (dependencias, `auth.rs`, rutas, middleware, CORS) y sus tests; con `AUTH_ENABLED=false` el comportamiento previo se conserva.
2. Implementar frontend y sus tests.
3. Añadir `Dockerfile.backend`, `Dockerfile.frontend`, `nginx.conf` y actualizar `docker-compose.prod.yml`; validar con `docker compose config` y smoke test.
4. Desplegar en producción con las variables `AUTH_*` y `JWT_SECRET`; comprobar login/logout y que las rutas protegidas devuelven 401 sin sesión.
5. Rollback: volver a `AUTH_ENABLED=false` desactiva el enforcement sin revertir la imagen; el resto es aditivo.

## Open Questions

- Ninguna pendiente que cambie specs, enfoque o desglose de tareas. Queda únicamente la verificación end-to-end real contra PocketID: comprobar que tras el logout el SSO queda cerrado y que volver a entrar exige credenciales.
