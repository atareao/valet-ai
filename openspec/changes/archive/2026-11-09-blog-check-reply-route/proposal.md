# Change: blog-check-reply-route

## Why

`GET /api/blog/check` valida hoy solo que la Application Password autentica
(`GET <base>/wp-json/wp/v2/users/me`). El 2026-11-09, con la credencial válida, la ruta de
escritura `POST /wp-json/wp/v2/comments` —la que usa `blog_comment_reply`— estaba muerta
(`404 rest_no_route`, política REST del plugin) y la sonda decía «conexión correcta». El fallo
solo apareció al intentar publicar una respuesta real. Issue
[#340](https://github.com/atareao/valet-ai/issues/340).

## What Changes

- **Sonda en dos fases** en `Wordpress::check_connection`: (1) `GET /wp-json/wp/v2/users/me`
  como hoy; (2) `GET /wp-json/` (índice REST) con la misma autenticación básica para comprobar
  que la ruta `/wp/v2/comments` existe y declara `POST` entre sus métodos.
- **Nueva variante** `WordpressError::ReplyRouteUnavailable` para «credenciales válidas pero la
  vía de respuesta no está disponible».
- **Nuevo detalle** en `GET /api/blog/check`:
  "las credenciales son válidas, pero la ruta de respuesta del blog no está disponible: revisa la política REST del plugin".
- **Sin efectos secundarios**: no se crea, modifica ni enumera ningún comentario.
- **Sin cambios** en el éxito (`"conexión correcta"`), en el formato `{ok, detail}`, en el `200`
  constante, en los ajustes, en las rutas, en el frontend ni en las migraciones.

## Capabilities

### New Capabilities

_(ninguna)_

### Modified Capabilities

- `blog-api`: la sonda `GET /api/blog/check` pasa a comprobar autenticación **y** disponibilidad
  de la vía de respuesta, sin efectos secundarios y sin filtrar la contraseña.

## Impact

- Código: `src/services/wordpress.rs` (variante de error + segunda fase de la sonda),
  `src/handlers/blog.rs` (mapeo del detalle) y sus tests.
- Documentación: `docs/wordpress-integration.md`.
- Sin migraciones, sin ajustes nuevos, sin cambios de frontend, sin cambios de despliegue.
- Fuera de alcance: sondear la vía dentro de las herramientas del blog y el arreglo del lado del
  sitio (`atareao/atareao.es#175`).
