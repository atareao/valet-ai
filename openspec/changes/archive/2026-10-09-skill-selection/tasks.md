# Tasks

## 0. Change proposal y aprobación

- [x] 0.1 Presentar este change (proposal + specs + tasks) y **esperar aprobación explícita** antes de escribir código.

## 1. RED: fijar el contrato

- [x] 1.1 Tests que fallan en `skill_router`: una skill con `ROUTER_SKILL_<ID>_ENABLED=false` no genera pregunta, no se selecciona, no expone sus herramientas ni inyecta su fragmento, con `ROUTER_ENABLED` on **y** off.
- [x] 1.2 Tests que fallan: el umbral efectivo de una skill es su `ROUTER_THRESHOLD_<ID>` y, si falta, el compilado; `ROUTER_THRESHOLD` no se lee.
- [x] 1.3 Tests que fallan en `/api/skills`: cada skill expone `enabled`.
- [x] 1.4 Tests que fallan (frontend): pestaña «Skills» con una pestaña por skill (interruptor, umbral, 4 textos); «Enrutador de skills» sin umbral global; ausencia de la lista de tools.

## 2. GREEN — Backend

- [x] 2.1 `read_router_config`: leer `ROUTER_SKILL_<ID>_ENABLED` y `ROUTER_THRESHOLD_<ID>`; retirar `ROUTER_THRESHOLD`.
- [x] 2.2 `effective_threshold`: override por skill o compilado; sin global.
- [x] 2.3 Filtro absoluto: excluir las skills deshabilitadas de las preguntas, la selección, el conjunto expuesto y los fragmentos.
- [x] 2.4 Fallo abierto acotado por el filtro de skills deshabilitadas.
- [x] 2.5 `handlers/skills.rs`: `SkillView.enabled` + `GET /api/skills`.
- [x] 2.6 Retirada del toggle de tools: `handlers/tools.rs`, `routes/tools.rs`, `db/repos/tools.rs`, `models/tool.rs`, `tools/registry.rs` (`set_disabled`), cableado en `main.rs`; `GET /api/tools` si queda sin uso.
- [x] 2.7 Migración: materializar los umbrales efectivos, sembrar `ROUTER_SKILL_<ID>_ENABLED`, retirar `ROUTER_THRESHOLD`, `ALTER TABLE tools DROP COLUMN enabled`.

## 3. GREEN — Frontend

- [x] 3.1 Nueva pestaña `Skills` (top-level) con una pestaña por skill: interruptor + umbral + pregunta + criterio SÍ + criterio NO + fragmento; guardado propio.
- [x] 3.2 `Enrutador de skills`: solo activar enrutado + modelo; sin umbral global ni lista de tools.
- [x] 3.3 Eliminar `ToolsTab` y `useTools`; limpiar `getTools`/`toggleTool` del cliente y los tipos.
- [x] 3.4 Helpers en `skillRouter.ts` (clave `enabled`, umbral per-skill).

## 4. REFACTOR / limpieza en cascada

- [x] 4.1 `cargo fmt` + `clippy --all-targets -D warnings` (0 warnings).
- [x] 4.2 Código muerto eliminado (rutas/handlers/repo/modelo/registro/tests).
- [x] 4.3 `tsc --noEmit` + lint frontend.

## 5. VERIFY / cierre

- [x] 5.1 `cargo test`, `npx vitest run`, `openspec validate --all --strict` en verde.
- [x] 5.2 Reviews `@rust-reviewer` y `@react-reviewer`; hallazgos aplicados.
- [x] 5.3 `openspec archive skill-selection` y actualizar `AGENTS.md § V` si procede.
