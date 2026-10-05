# Tasks

## 1. Contratos y dependencias (RED)

- [x] 1.1 Añadir a `Cargo.toml` `jsonwebtoken` y `axum-extra` con feature `cookie`; verificar con `cargo check` que compilan en el árbol actual (y que `reqwest` cubre el cliente OIDC).
- [x] 1.2 Escribir los tests unitarios RED de sesión en `src/auth.rs`: firma válida, expiración y manipulación del token; verificar que fallan con el stub actual.
- [x] 1.3 Escribir los tests unitarios RED de `state`/`nonce` (generación única, consumo de un solo uso) y de validación del ID token (`iss`, `aud`, `exp`, `nonce`, firma) con `wiremock`; verificar que fallan.
- [x] 1.4 Escribir los tests de integración RED de `tests/api/auth.rs` para `/api/auth/me` (401 sin cookie, 200 con claims), enforcement y exención de `/api/health` y `/api/auth/*`; verificar que fallan.
- [x] 1.5 Escribir los tests unitarios RED del logout: con ID token la URL devuelta incluye `id_token_hint` (y `post_logout_redirect_uri` cuando está configurado); sin ID token solo se cierra la sesión local; verificar que fallan.

## 2. Backend — sesión y flujo OIDC

- [x] 2.1 Implementar en `src/auth.rs` la emisión/validación de la cookie de sesión HS256 (claims `sub`/`email`/`name`/`exp`) y hacer pasar los tests de 1.2.
- [x] 2.2 Parsear `AUTH_POST_LOGOUT_REDIRECT_URL` en `Config`/`AuthConfig` (default: la raíz pública de la app) y exponerlo al flujo de logout.
- [x] 2.3 Implementar generación/consumo de `state` y `nonce` y la validación del ID token (discovery + JWKS + `jsonwebtoken`) haciendo pasar los tests de 1.3.
- [x] 2.4 Implementar en `src/routes/auth.rs` `login` (302 con `state`/`nonce`) y `callback` (validación, intercambio, cookie, 302), registrando el ID token del proveedor como claim `id_token` de la sesión emitida; registrar `routes()` en `src/lib.rs`.
- [x] 2.5 Implementar `me` y `logout`, construyendo la URL de cierre SSO `{end_session_endpoint}?id_token_hint=<id_token>&post_logout_redirect_uri=<...>` a partir del ID token retenido, y verificar `/api/auth/me` en los tests de 1.4.

## 3. Backend — enforcement y CORS

- [x] 3.1 Implementar el middleware `from_fn_with_state` de sesión con exenciones y 401, y aplicarlo al router de `/api/*`; verificar los tests de enforcement y exención de 1.4.
- [x] 3.2 Implementar el modo dev (`auth.enabled=false`) como usuario por defecto y verificar que los tests existentes siguen en verde con `AUTH_ENABLED=false`.
- [x] 3.3 Sustituir `allow_origin(Any)` por un origen explícito con credenciales derivado de la configuración y verificar el test de CORS (`tests/api/cors.rs`).
- [x] 3.4 Hacer pasar los tests de logout de 1.5: la URL incluye `id_token_hint` (y `post_logout_redirect_uri` si está configurado) y sin ID token solo se cierra la sesión local.

## 4. Frontend

- [x] 4.1 Añadir `credentials: "include"` en `frontend/src/api/client.ts` (y en streaming) y verificar con un test que las peticiones incluyen credenciales.
- [x] 4.2 Crear `AuthContext`/`useAuth` que resuelve `/api/auth/me` y expone `user`/`loading`/`logout`; verificar con test que `me` en curso muestra estado de carga.
- [x] 4.3 Crear `LoginPage` que navega a `/api/auth/login` y el guard en `App.tsx` (spinner → login → aplicación); verificar con tests de render.
- [x] 4.4 Añadir la detección global de 401 que redirige a login y verificarla con un test que simula un 401.
- [x] 4.5 Añadir el botón de logout en `AppLayout.tsx` que llama a `POST /api/auth/logout` y redirige al `end_session_endpoint`; verificar con test de interacción.
- [x] 4.6 Ejecutar `npx tsc --noEmit`, `npm run lint` y `npx vitest run` en `frontend/` y confirmar 100% verde.
- [x] 4.7 Alinear el contrato de logout al backend (`end_session_url` en `api/client.ts` y `AuthProvider`); actualizar el test de `AppLayout` y añadir un test de contrato estricto (`auth.logout.contract.test.ts`).
- [x] 4.8 Un 401 en streaming (`useSSE`) emite `valet:unauthorized` además de `onError`; cubierto con test.
- [x] 4.9 `AuthUser.email`/`name` admiten `null` como los serializa el backend (`MeResponse`).
- [x] 4.10 Test de cadena completa 401 → `valet:unauthorized` → `AuthProvider` → `LoginPage` (`auth.fullchain.test.tsx`).
- [x] 4.11 Logout valida destino `https:` (anti open redirect) y usa `import.meta.env.BASE_URL` como fallback; tests de `null`/URL insegura (`auth.logout.test.tsx`).
- [x] 4.12 Evitar el doble `GET /api/auth/me` bajo `StrictMode` cacheando la promesa en un ref.
- [x] 4.13 `useMemo` para el valor del contexto y estado `loggingOut` que deshabilita el botón de logout.
- [x] 4.14 Comentarios de tests RED→GREEN y documentado el `useContext` directo en `AppLayout`.
- [x] 4.15 Fallback de logout usa `import.meta.env.BASE_URL || "/"` (bug de `base: ''`), con test que fija el destino `/`.
- [x] 4.16 `AuthProvider` usa la constante exportada `UNAUTHORIZED_EVENT`; test de un solo `me` bajo `StrictMode`.

## 5. Infra de producción

- [x] 5.1 Crear `Dockerfile.backend` (Rust musl + runtime, puerto 3000) y verificar que `docker build` construye la imagen.
- [x] 5.2 Crear `Dockerfile.frontend` con stage de build Vite + stage nginx y verificar que la imagen construye y contiene el `dist`.
- [x] 5.3 Crear `nginx.conf` (fallback SPA a `index.html`, proxy `/api` con SSE sin buffering) y verificar el proxy a `/api/health`.
- [x] 5.4 Actualizar `docker-compose.prod.yml` para propagar `AUTH_REDIRECT_URL` y `AUTH_POST_LOGOUT_REDIRECT_URL` y verificar con `docker compose -f docker-compose.prod.yml config`.
- [x] 5.5 Levantar el stack y verificar que `/api/health` responde `200` `status ok` a través de nginx.

## 6. Verificación e integración

- [ ] 6.1 Ejecutar `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings` y `cargo test` y corregir cualquier fallo.
- [ ] 6.2 Ejecutar `just check-all` y confirmar que cubre formato, clippy, tests Rust, lint, tests y build del frontend en verde.
- [ ] 6.3 Ejecutar `openspec validate oidc-auth` y confirmar que el change valida sin errores.
- [ ] 6.4 Verificar manualmente el flujo completo end-to-end (login → `me` → rutas protegidas → logout SSO) contra PocketID y documentar el resultado, comprobando explícitamente que tras el logout el SSO queda cerrado: al volver a entrar el proveedor pide credenciales de nuevo.

## 7. Cierre y archivado

- [x] 7.1 Actualizar la documentación de variables de entorno (`AUTH_REDIRECT_URL`, `AUTH_POST_LOGOUT_REDIRECT_URL`, `JWT_SECRET`, `AUTH_*`) y verificar que un lector puede configurar el stack solo con ella.
- [ ] 7.2 Marcar las tareas completadas en `tasks.md` y, con las specs validadas, ejecutar `openspec archive oidc-auth` comprobando que los headers del delta coinciden con `openspec/specs/`.

## 8. Hardening de seguridad (auditoría REFACTOR)

- [x] 8.1 **Fail-closed de configuración (crítico):** `AuthConfig::validate()` aborta el arranque si `auth.enabled` y `jwt_secret`/`client_id`/`client_secret`/`redirect_url`/`issuer_url` están vacíos. `AppState::new_with_orchestrator` valida ANTES de cualquier efecto secundario. Tests: `test_new_with_orchestrator_fails_closed_when_auth_incomplete`, `test_new_with_orchestrator_ok_with_complete_auth_config`, `test_auth_config_validate_*`.
- [x] 8.2 **Cookie de flujo firmada + `StateStore`:** el `state`/`nonce` viaja en una única cookie `valet_oauth_flow` firmada (HS256 con `jwt_secret`, `FlowClaims`), verificada en el callback; el `state` se consume una sola vez vía `StateStore` (ya no es código muerto) ANTES del intercambio de código. Impide plantar un `state` desde otro subdominio. Tests: `test_flow_token_*`, `callback_reusing_state_returns_401`, `callback_with_mismatched_state_returns_401`.
- [x] 8.3 **CORS con credenciales:** se elimina `AllowOrigin::mirror_request()`; se usa una lista explícita (`AllowOrigin::list`): orígenes derivados de la config si auth está activo, o `http://localhost:5173`/`http://localhost:3000` en dev. Lista vacía ⇒ sin ACAO. Tests reforzados en `tests/api/cors.rs` (sin comodín, sin reflexión arbitraria, preflight OPTIONS).
- [x] 8.4 **Cliente HTTP compartido + caché + límite de cuerpo:** `http_client()` (`OnceLock`, timeout 10s, pool) usado por todas las rutas; discovery y JWKS cacheados con TTL en `tokio::sync::RwLock`; `read_limited_json` rechaza cuerpos > 64 KiB. Test: `test_discover_rejects_oversized_body`.
- [x] 8.5 **Validación de `jwks_uri`:** se exige mismo origen que el `issuer` antes de usar el JWKS (`same_origin`). Test: `test_validate_id_token_rejects_jwks_on_another_origin`.
- [x] 8.6 **Config fuente única:** `AuthConfig::from_config(&Config)` sustituye las relecturas de env en `new_with_orchestrator`; `Config` sigue siendo la única fuente. `Config::from_env` conserva su firma y defaults.
- [x] 8.7 **Tests añadidos:** callback end-to-end (discovery+JWKS+token con `wiremock`), `state` no coincidente/reutilizado → 401 sin cookie, y `alg=none`/confusión HS256.
- [x] 8.8 **`leeway`:** sesión y cookie de flujo sin margen (`0`); ID token con margen de 5s (`ID_TOKEN_LEEWAY_SECS`).
- [x] 8.9 **`StateStore` con TTL, cota y mutex a prueba de veneno:** almacén `Mutex<HashMap<String,(String,Instant)>>`; `issue()` purga entradas con `elapsed >= FLOW_TTL` (600s) y, si tras purgar `len() >= MAX_PENDING_STATES` (1024), expulsa la más antigua antes de insertar; `consume()` rechaza estados expirados y sigue siendo de un solo uso; el mutex se recupera con `unwrap_or_else(|poisoned| poisoned.into_inner())` (sin `expect`). Tests: `state_expires_after_ttl`, `state_within_ttl_is_consumed_once`, `state_store_enforces_cap` (helpers `issue_with_now`/`consume_with_now`, sin `sleep`).
- [x] 8.10 **`http_client()` conserva el timeout en el fallback:** el reintento sin `pool_max_idle_per_host` mantiene `.timeout(HTTP_TIMEOUT_SECS)`, registra el fallo con `tracing::warn!` y solo usa `Client::new()` (sin timeout) como último recurso.
- [x] 8.11 **Defaults fail-closed de `AUTH_REDIRECT_URL`/`AUTH_POST_LOGOUT_REDIRECT_URL`:** `Config::from_env` usa `unwrap_or_default()` y `AuthConfig::default()` deja ambos en `String::new()`, de modo que con `AUTH_ENABLED=true` y sin configurarlos `validate()` aborta el arranque. Tests actualizados: `test_config_defaults`, `test_auth_config_default`, `test_production_state_initialization` (fija `AUTH_REDIRECT_URL` explícita).
- [x] 8.12 **`AUTH_ISSUER_URL` fail-closed:** mismo criterio para el issuer: `Config::from_env` pasa a `unwrap_or_default()` y `AuthConfig` deriva `Default` (issuer vacío), de modo que habilitar auth sin definir `AUTH_ISSUER_URL` aborta el arranque en vez de confiar en `http://localhost:8080`. `validate()` sin cambios (ya exige issuer no vacío). Tests: `test_config_defaults` y `test_auth_config_default` esperan `""`; se refuerza `test_auth_config_validate_fails_closed_when_enabled_and_incomplete` con los ejes `client_secret` y `redirect_url` vacíos; `test_production_state_initialization` fija `AUTH_ISSUER_URL` explícita.

Notas de no aplicabilidad: ninguna. Los 9 hallazgos se han aplicado; el de `jwks_uri` (RFC 8414) se implementa como comprobación estricta de mismo origen, aceptando el trade-off de rechazar proveedores que sirvan el JWKS en otro host (PocketID lo sirve en el mismo origen).
