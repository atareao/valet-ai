# Design: blog-check-reply-route

## Decisión 1 — Sondear el índice REST en vez de escribir

**Contexto.** Validar la capacidad de respuesta sin efectos secundarios públicos.

**Alternativas descartadas**.
- `POST /wp-json/wp/v2/comments` con un cuerpo de prueba: publicaría un comentario real y exigiría
  un post válido; descartado por la restricción del issue.
- `OPTIONS /wp-json/wp/v2/comments`: algunos sitios lo sirven sin autenticación o sin reflejar la
  política real de la identidad; el índice filtrado es más fiel.
- `HEAD` sobre la colección: no distingue «ruta cerrada» de «método no permitido» de forma fiable.

**Decisión.** `GET <base>/wp-json/` autenticado y comprobación de `routes["/wp/v2/comments"].methods`
contiene `POST`. Es exactamente lo verificado a mano en el diagnóstico del 2026-11-09: la ruta
aparece solo cuando el sitio la expone para esa identidad, y aparece con `["POST"]` (sin `GET`).

## Decisión 2 — Una variante de error nueva, no reutilizar `RouteNotFound`

`RouteNotFound` describe un `404 rest_no_route` al **operar** sobre un comentario concreto. Aquí no
hay operación: hay un veredicto de capacidad. Se añade `ReplyRouteUnavailable`, con detalle propio,
para que el handler pueda dar un mensaje accionable y los tests distingan ambos casos.

## Decisión 3 — Clasificación del índice

- Red caída o `5xx` → `Unavailable` (el sitio no responde; el detalle sigue siendo «el blog no
  responde»).
- Cualquier otro no-`200` (`401`, `403`, `404`, …) o cuerpo sin `routes` → `ReplyRouteUnavailable`.
  Un índice cerrado no permite confirmar la vía: se informa como «no disponible», nunca como éxito.
- `200` sin `/wp/v3/comments` o sin `POST` → `ReplyRouteUnavailable`.

El sesgo es deliberado: **fallar cerrado**. Un falso «no disponible» se resuelve revisando la
política REST; un falso «conexión correcta» es el bug que este change elimina.

## Decisión 4 — El éxito y el formato no cambian

`{"ok": true, "detail": "conexión correcta"}` se mantiene: hay aserciones exactas en
`src/handlers/blog.rs` y en `frontend/src/components/BlogSection.test.tsx`. El endpoint sigue
respondiendo `200` siempre y el frontend sigue pintando `detail` tal cual, así que no hay cambios
de UI.

## Consecuencias

- El check pasa de una a dos peticiones (hasta 15 s cada una). Es una acción manual desde Ajustes;
  no se añade caché.
- `blog_comment_reply` sigue siendo la única vía que descubre un fallo en tiempo de publicación: no
  se instrumenta aquí.
- El arreglo del lado del sitio (`atareao/atareao.es#175`) es independiente; este change endurece
  el consumidor.