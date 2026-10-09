# Tasks

## 0. Change proposal y aprobación

- [ ] 0.1 Presentar este change (proposal + specs + tasks) y **esperar aprobación explícita** antes de escribir código.

## 1. RED: fijar el contrato

- [ ] 1.1 Tests que fallan en `skill_router`: una skill con `ROUTER_SKILL_<ID>_ENABLED=false` no genera pregunta, no se selecciona, no expone sus herramientas ni inyecta su fragmento, con `ROUTER_ENABLED` on **y** off.
- [ ] 1.2 Tests que fallan: el umbral efectivo de una skill es su `ROUTER_THRESHOLD_<ID>` y, si falta, el compilado; `ROUTER_THRESHOLD` no se lee.
- [ ] 1.3 Tests que fallan en `/api/skills`: cada skill expone `enabled`.
- [ ] 1.4 Tests que fallan (frontend): pestaña «Skills» con una pestaña por skill (interruptor, umbral, 4 textos); «Enrutador de skills» sin umbral global; ausencia de la lista de tools.

## 2. GREEN — Backend

- [ ] 2.1 `read_router_config`: leer `ROUTER_SKILL_<ID>_ENABLED` y `ROUTER_THRESHOLD_<ID>`; retirar `ROUTER_THRESHOLD`.
- [ ] 2.2 `effective_threshold`: override por skill o compilado; sin global.
- [ ] 2.3 Filtro absoluto: excluir las skills deshabilitadas de las preguntas, la selección, el conjunto expuesto y los fragmentos.
- [ ] 2.4 Fallo abierto acotado por el filtro de skills deshabilitadas.
- [ ] 2.5 `handlers/skills.rs`: `SkillView.enabled` + `GET /api/skills`.
- [ ] 2.6 Retirada del toggle de tools: `handlers/tools.rs`, `routes/tools.rs`, `db/repos/tools.rs`, `models/tool.rs`, `tools/registry.rs` (`set_disabled`), cableado en `main.rs`; `GET /api/tools` si queda sin uso.
- [ ] 2.7 Migración: materializar los umbrales efectivos, sembrar `ROUTER_SKILL_<ID>_ENABLED`, retirar `ROUTER_THRESHOLD`, `ALTER TABLE tools DROP COLUMN enabled`.

## 3. GREEN — Frontend

- [ ] 3.1 Nueva pestaña `Skills` (top-level) con una pestaña por skill: interruptor + umbral + pregunta + criterio SÍ + criterio NO + fragmento; guardado propio.
- [ ] 3.2 `Enrutador de skills`: solo activar enrutado + modelo; sin umbral global ni lista de tools.
- [ ] 3.3 Eliminar `ToolsTab` y `useTools`; limpiar `getTools`/`toggleTool` del cliente y los tipos.
- [ ] 3.4 Helpers en `skillRouter.ts` (clave `enabled`, umbral per-skill).

## 4. REFACTOR / limpieza en cascada

- [ ] 4.1 `cargo fmt` + `clippy --all-targets -D warnings` (0 warnings).
- [ ] 4.2 Código muerto eliminado (rutas/handlers/repo/modelo/registro/tests).
- [ ] 4.3 `tsc --noEmit` + lint frontend.

## 5. VERIFY / cierre

- [ ] 5.1 `cargo test`, `npx vitest run`, `openspec validate --all --strict` en verde.
- [ ] 5.2 Reviews `@rust-reviewer` y `@react-reviewer`; hallazgos aplicados.
- [ ] 5.3 `openspec archive skill-selection` y actualizar `AGENTS.md § V` si procede.
