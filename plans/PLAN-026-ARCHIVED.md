# PLAN-026 — Endurecer `GET /api/blog/check`: detectar la vía de respuesta muerta

**Issue:** [atareao/valet-ai#340](https://github.com/atareao/valet-ai/issues/340)
**Change OpenSpec:** `blog-check-reply-route`
**Rama:** `feature/blog-check-reply-route` (desde `development`)
**Estado:** ✅ Completado — v0.36.0 desplegada en `co1`; E2E `GET /api/blog/check` → `{ok:true}`.

## 1. Contexto

`GET /api/blog/check` sondea hoy una sola cosa: que la Application Password sirva, con
`GET <base>/wp-json/wp/v2/users/me`. Eso valida **autenticación**, no **capacidad de escritura**.

El 2026-11-09, con credenciales perfectamente válidas, la ruta de colección
`POST /wp-json/wp/v2/comments` —la que usa `blog_comment_reply`— devolvía `404 rest_no_route`
porque el plugin del sitio solo la expone al «usuario de servicio». `check` decía «conexión
correcta» y el fallo no apareció hasta que el usuario intentó publicar una respuesta real.

## 2. Objetivo

Que la sonda distinga cinco desenlaces:

1. credenciales válidas **y** vía de respuesta disponible → `ok:true`;
2. credenciales válidas pero **vía de respuesta no disponible** → `ok:false` con detalle
   específico y accionable;
3. credenciales inválidas → como hoy;
4. sitio caído → como hoy;
5. sin credenciales → como hoy (sin salir a la red, sin cambios).

## 3. Diseño acordado

- **Mecanismo:** tras un `users/me` con `200`, la sonda consulta el índice REST
  `GET <base>/wp-json/` con la misma autenticación básica. El índice publica el mapa de rutas
  **ya filtrado por la política del sitio para esa identidad**; basta comprobar que existe la
  clave `/wp/v2/comments` y que `methods` contiene `POST`.
- **Sin efectos secundarios:** la sonda **no** crea, modifica ni enumera comentarios.
- **Contrato HTTP intacto:** `GET /api/blog/check` sigue respondiendo siempre `200` con
  `{ok, detail}`, y el detalle nunca contiene la contraseña.
- **Detalle de éxito sin cambios:** "conexión correcta".
- **Nuevo detalle de fallo:**
  "las credenciales son válidas, pero la ruta de respuesta del blog no está disponible: revisa la política REST del plugin".
- **Nueva variante:** `WordpressError::ReplyRouteUnavailable`.
- **Clasificación del índice REST:** red caída o `5xx` → `Unavailable`; cualquier otro no-`200` o
  cuerpo sin `routes` → `ReplyRouteUnavailable`; ruta ausente o sin `POST` → `ReplyRouteUnavailable`.
- **Frontend sin cambios:** `BlogSection` pinta el `detail` tal cual.

## 4. Fuera de alcance

- Sondear la vía dentro de `blog_comment_reply` o `blog_comment_delete`.
- Endpoints, ajustes o migraciones nuevos.
- Cambiar el texto del éxito o el formato de la respuesta.

## 5. Tareas (TDD)

### 5.1 RED
- [x] Test (`wiremock`) `users/me` 200 + índice con `/wp/v2/comments` y `POST` → `Ok(())`.
- [x] Test: índice sin `/wp/v2/comments` → `ReplyRouteUnavailable`.
- [x] Test: índice con la ruta pero sin `POST` en `methods` → `ReplyRouteUnavailable`.
- [x] Test: índice `404`/`403` → `ReplyRouteUnavailable`.
- [x] Test: índice `5xx` y fallo de red → `Unavailable`.
- [x] Test: cuerpo del índice ilegible o sin `routes` → `ReplyRouteUnavailable`.
- [x] Test: 0 peticiones a `/wp-json/wp/v2/comments` y ningún `POST`.
- [x] Test (`src/handlers/blog.rs`): índice sin la ruta → `ok:false` con el detalle específico y sin contraseña.
- [x] `cargo test` → RED.

### 5.2 GREEN
- [x] Añadir `WordpressError::ReplyRouteUnavailable` con su `Display`.
- [x] Implementar la segunda fase en `Wordpress::check_connection` (helper privado del índice).
- [x] Mapear la variante nueva en `check_detail`.
- [x] `cargo test` y `cargo check` verdes.

### 5.3 REFACTOR
- [x] `cargo fmt --check` y `cargo clippy -- -D warnings` limpios.
- [x] `openspec validate --all --strict` limpio.
- [x] Actualizar `docs/wordpress-integration.md`.
- [x] `npx vitest run` en `frontend/`.

### 5.4 Cierre
- [x] Auditoría `@rust-reviewer` sin bloqueantes.
- [x] PR a `development`, release `v0.36.0` a `main`, despliegue en `co1` y E2E `{ok:true}`.
- [x] `openspec archive blog-check-reply-route`, `docs/`, índice de `plans/`, issue #340 cerrado.

## 6. Riesgos

- **Falso negativo por índice restringido:** se acepta; el veredicto es «no puedo confirmar la vía
  de respuesta», accionable, nunca un falso positivo.
- **Doble petición:** el check pasa de 1 a 2 llamadas (hasta 15 s cada una). Sin caché.