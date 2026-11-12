# Tasks: blog-check-reply-route

Issue: [#340](https://github.com/atareao/valet-ai/issues/340)

## 1. RED (tests que fallan)

### 1.1 Servicio WordPress (`src/services/wordpress.rs`)
- [x] 1.1.1 Test (`wiremock`): `users/me` 200 + índice con `/wp/v2/comments` y `POST` → `Ok(())`.
- [x] 1.1.2 Test: índice sin la clave `/wp/v2/comments` → `ReplyRouteUnavailable`.
- [x] 1.1.3 Test: índice con `/wp/v2/comments` pero `methods` sin `POST` → `ReplyRouteUnavailable`.
- [x] 1.1.4 Test: índice `404` → `ReplyRouteUnavailable`.
- [x] 1.1.5 Test: índice `403` → `ReplyRouteUnavailable`.
- [x] 1.1.6 Test: índice `503` → `Unavailable`.
- [x] 1.1.7 Test: fallo de red en el índice → `Unavailable`.
- [x] 1.1.8 Test: índice `200` con cuerpo ilegible o sin `routes` → `ReplyRouteUnavailable`.
- [x] 1.1.9 Test: sin peticiones a `/wp-json/wp/v2/comments` y sin `POST` (wiremock).
- [x] 1.1.10 Test: `Display` de `ReplyRouteUnavailable` sin contraseña ni `Authorization`.
- [x] 1.1.11 `cargo test` → RED.

### 1.2 Handler (`src/handlers/blog.rs`)
- [x] 1.2.1 Test: `users/me` 200 + índice sin la ruta → `200` con `ok:false` y el detalle específico.
- [x] 1.2.2 Test: ese detalle no contiene la contraseña.
- [x] 1.2.3 Test: el caso sano sigue devolviendo "conexión correcta" (sin regresión).
- [x] 1.2.4 `cargo test` → RED.

## 2. GREEN (implementación mínima)
- [x] 2.1 Añadir `WordpressError::ReplyRouteUnavailable` con su `Display`.
- [x] 2.2 Implementar la segunda fase de `Wordpress::check_connection` (helper privado del índice).
- [x] 2.3 Mapear la variante nueva en `check_detail`.
- [x] 2.4 `cargo test` verde y `cargo check` sin errores.

## 3. REFACTOR
- [x] 3.1 `cargo fmt --check` y `cargo clippy -- -D warnings` limpios.
- [x] 3.2 Comentarios de módulo y `Purpose` coherentes.
- [x] 3.3 `npx vitest run` en `frontend/` (sin cambios esperados).
- [x] 3.4 `openspec validate --all --strict` limpio.

## 4. CIERRE
- [x] 4.1 Actualizar `docs/wordpress-integration.md`.
- [x] 4.2 Auditoría `@rust-reviewer` sin bloqueantes.
- [x] 4.3 PR a `development` (gitmoji) y release `v0.36.0` a `main`.
- [x] 4.4 Despliegue en `co1` y E2E: `GET /api/blog/check` → `{ok:true}`.
- [ ] 4.5 `openspec archive blog-check-reply-route`, índice de `plans/`, issue #340 cerrado.