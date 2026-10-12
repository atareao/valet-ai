# Tasks

Todas las casillas empiezan sin marcar. **No se escribe ni una línea de código ni un solo test antes de que el usuario apruebe este change.**

## 0. Aprobación (punto de parada del protocolo)

- [x] 0.1 Presentar este change (proposal + delta de `notifications` + design + tasks) y **esperar aprobación explícita** antes de tocar código.

## 1. RED — aserciones de cuerpo y de `Content-Type`

- [x] 1.1 RED — en `tests/api/notifications.rs`, añadir a los cuatro tests de `401` (`ingest_without_authorization_returns_401_and_no_row`, `ingest_with_malformed_authorization_returns_401`, `ingest_with_unknown_token_returns_401`, `ingest_with_revoked_key_returns_401`) la aserción de que el `Content-Type` es `application/json` y el cuerpo es `{"error":"unauthorized"}`. Debe fallar (hoy el cuerpo va vacío). Comando en rojo: `cargo test --test notifications`. **Pegar la salida en rojo en el informe.**
- [x] 1.2 RED — test nuevo que asevera que una clave **revocada** y un token **desconocido** producen respuestas **idénticas** (mismo estado y mismo cuerpo), de modo que un observador externo no puede distinguirlas. Comando: `cargo test --test notifications`.

## 2. GREEN — cuerpo JSON y registro del motivo

- [x] 2.1 GREEN — en `src/middleware/auth.rs`, sustituir los tres `StatusCode::UNAUTHORIZED.into_response()` de `require_ingest_key` por una respuesta `401` con `Content-Type: application/json` y cuerpo `{"error":"unauthorized"}`, y registrar en cada caso el **motivo** con `tracing::warn!`: `missing_authorization` (sin cabecera), `malformed_authorization` (cabecera sin esquema `Bearer`) y `unknown_or_revoked_key` (token sin clave). **Nunca registrar el token ni su prefijo.** Confirmar en verde con `cargo test --test notifications`.
- [x] 2.2 GREEN — `cargo check` y `cargo test --test notifications` sin errores.

## 3. Verificación y cierre

- [x] 3.1 REFACTOR — `cargo fmt`, `cargo clippy -- -D warnings` y `cargo test` (suite completa) en verde. Confirmar que los tests de `tests/api/auth.rs` de sesión OIDC siguen pasando sin cambios.
- [x] 3.2 Frontend sin cambios: no aplica `vitest` ni `tsc`; dejar constancia de que no se toca el frontend.
- [x] 3.3 `openspec validate --all` en verde.
- [ ] 3.4 PR a `development` con merge commit, revisado por `rust-reviewer`. Archive del change en PR aparte tras el merge.
