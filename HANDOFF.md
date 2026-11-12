# HANDOFF

**Fecha:** 2026-11-09

## Estado

**COMPLETADO.** El change `blog-check-reply-route` (issue #340) está implementado, auditado,
desplegado como **v0.36.0** en `co1` y verificado E2E. No queda ninguna tarea crítica.

## Qué se ha hecho (sonda del blog)

- **Código** (PR #346 → `development`, CI verde): `check_connection` sondea en dos fases —
  `GET /wp-json/wp/v2/users/me` y `GET /wp-json/` autenticado exigiendo
  `routes["/wp/v2/comments"].methods` con `POST`. Variante nueva
  `WordpressError::ReplyRouteUnavailable` y detalle accionable en `GET /api/blog/check`. Sin
  efectos secundarios: ninguna petición a la colección de comentarios.
- **Release v0.36.0** (PR #347 → `main`, tag `v0.36.0`, imagen publicada) y back-merge a
  `development` (PR #348).
- **Despliegue** en `co1` (Dockge): `/api/health` → `{"version":"0.36.0"}`.
- **E2E** (instancia local con copia de la BD de producción, `AUTH_ENABLED=false`):
  `GET /api/blog/status` → `{configured:true,...}` y `GET /api/blog/check` →
  `{ok:true,"detail":"conexión correcta"}` en 1,13 s. El índice del sitio declara
  `/wp/v2/comments` con `["POST"]`.
- **Cierre documental** (rama `chore/blog-check-reply-route-closeout`): change archivado
  (`2026-11-09-blog-check-reply-route`), `docs/wordpress-integration.md` actualizado,
  `plans/PLAN-026-ARCHIVED.md`, AGENTS §V entrada 25 + índice §VI, issue **#340** cerrado.
- Auditoría `@rust-reviewer` sin bloqueantes; los 4 hallazgos aplicados (el más importante:
  anclar la autenticación del índice en el mock).
- **Documentación** (PR #350 → `development`): `docs/android-http-shortcuts.md`, la receta para
  lanzar avisos a Valet desde la app Android «Atajos HTTP» (clave de ingesta, atajo, azulejo de
  ajustes rápidos, errores y límites), enlazada desde la sección `## 🔌 API` del `README.md`.

## Tarea pendiente / siguiente paso

- **Ninguna tarea abierta.** `valet-ai` sin issues ni PRs, `openspec list` sin changes activos.
- Candidatas (sin asignar, por orden de tamaño):
  1. **`frontend/src/test/dependency-stack.test.ts` solo cubre la etapa A** de React 19 (etapa B:
     React 19, react-leaflet 5, chart.js 4.6.1 sin cubrir). Alcance cerrado: un fichero de test, sin
     código de producción. Endurecerlo haría fallar el CI si alguien baja esas versiones.
  2. **atareao.es #175**: aprovisionar `atareao_valet_reply_user` desde la UI del plugin (hoy paso
     manual de WP-CLI).
  3. **«Token de chat» para el móvil** (solo si se pide): hoy solo `POST /api/notifications` admite
     clave; leer la bandeja o mandar una frase al agente exige cookie OIDC de 8 h. Es superficie
     nueva de autenticación: necesita change con spec, no un parche.
- **Aparcado por decisión del usuario, no tocar sin orden:** `PLAN-004-PENDING` (roadmap v2) y
  `PLAN-010-PENDING` (TTL de sesión de 8 h).

## Follow-ups conocidos

- **Bundle inicial al límite:** 69.98/70.00 kB. Los métodos por integración **no** van en
  `api/client.ts` (grafo inicial); van en su propio `api/<X>.ts` importado por un hook lazy.
  No subir el límite del control de bundle.
- **Archive deja el `Purpose` en placeholder** para capabilities nuevas: reescribirlo a mano y
  volver a pasar `openspec validate --all --strict`.
- **Pedir el dominio público desde `co1` da `403 Ko`** (bloqueo de IP origen/WAF); desde fuera
  responde 200.

## Restricciones

- Flujo SDD + TDD: aprobación de la spec antes de escribir código.
- Gitflow: PR a `development`/`main`, commits con gitmoji. No commits directos a `main`/`development`.
- No relajar aserciones de tests.
