# HANDOFF

**Fecha:** 2026-11-12

## Estado

**COMPLETADO.** El change `oximeal-search-semantics` (issue #361) está implementado, auditado,
desplegado como **v0.37.0** en `co1` y verificado E2E contra OxiMeal real. `openspec list` sin
changes activos.

## Qué se ha hecho (búsqueda de recetas de OxiMeal)

- **Código** (PR #363 → `development`, CI verde): `search_recipes` parte la consulta en términos
  (`split_whitespace`, dedup case-insensitive, tope `MAX_SEARCH_TERMS = 6`), hace **una petición
  por término en paralelo** (`futures::future::join_all`, mismo `category`) y combina en Valet:
  primero las recetas que coinciden con **todos** los términos, si no por **nº de coincidencias**
  descendente; dedup por `id` (la puntuación cuenta términos, no apariciones), máx 10. El esquema
  de `query` documenta la semántica (subcadena contigua) y el aviso accionable cuando `count == 0`.
  Migración `20261112000001_oximeal_search_semantics.sql` actualiza `SKILL_MENUS_PROMPT` con
  guarda byte a byte.
- **Release v0.37.0** (PR #364 → `main`, tag `v0.37.0`, imagen publicada). Hubo conflicto de
  versión en el PR de release (`development` estaba en 0.36.0, `main` en 0.36.1); resuelto a
  0.37.0.
- **Despliegue** en `co1` (Dockge, `~/docker/dockge/stacks/valet`):
  `docker compose pull valet && up -d valet`; `/api/health` → `{"version":"0.37.0"}` (verificado
  por IP del contenedor, porque desde el host el dominio público da 403).
- **E2E real contra OxiMeal** (token M2M desde la BD de producción, una petición por término,
  combinación idéntica a Valet): `tomate aguacate mozzarella` → «Ensalada de tomate, aguacate y
  mozzarella» **primera con 3/3 términos** (antes daba 0).
- **Cierre documental** (PR #365 → `development`): change archivado
  (`2026-11-12-oximeal-search-semantics`), deltas fusionados en `oximeal-service`/`tools/registry`/
  `oximeal-api`, `docs/oximeal-integration.md` actualizado, `plans/PLAN-028-ARCHIVED.md`, fila
  nueva en el índice §VI de `AGENTS.md`, issue **#361** cerrado.
- Auditoría `@rust-reviewer` sin bloqueantes; hallazgo aplicado (dedup por término).

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
  responde 200. Para E2E desde `co1`, usar la IP del contenedor o un contenedor en su red.
- **Duplicados en el catálogo de OxiMeal** (Merluza/Crema/Lentejas ×2, sin instrucciones):
  limpieza aparte, fuera de alcance.

## Restricciones

- Flujo SDD + TDD: aprobación de la spec antes de escribir código.
- Gitflow: PR a `development`/`main`, commits con gitmoji. No commits directos a `main`/`development`.
- No relajar aserciones de tests.
