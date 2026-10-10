# Proposal

## Why

La selección de herramientas (el switch por tool de la pestaña «Herramientas») **ya no tiene sentido**: el catálogo de skills cubre el 100 % de las herramientas registradas —hay un test de integridad que lo exige—, de modo que el nivel natural de selección es la **skill**, no la tool. Además:

- No se puede **apagar un dominio** entero (p. ej. los widgets) de un modo que el enrutador respete: apagar la tool `render_widget` no expresa «no quiero widgets».
- El **umbral global** (`ROUTER_THRESHOLD`) obliga a todas las skills a compartir un valor, salvo el override aislado de `widgets`; el control fino por dominio no existe.
- El estado `enabled` por tool no es cosmético: filtra el prompt y rechaza la ejecución, con un andamiaje (`ToolsRepo`, `set_disabled`, endpoint, columna) que ahora sobra.

## What Changes

- **La skill pasa a ser la unidad seleccionable**. Nueva clave `ROUTER_SKILL_<ID>_ENABLED` (por defecto habilitada). Una skill **deshabilitada es un filtro absoluto**: nunca se enruta, no expone sus herramientas ni inyecta su fragmento, **aunque `ROUTER_ENABLED` esté apagado**.
- **Umbral 100 % por skill**: se **elimina** `ROUTER_THRESHOLD` (global). El umbral efectivo de una skill es su `ROUTER_THRESHOLD_<ID>` y, si falta, el valor compilado del catálogo.
- **Se elimina por completo la selección de herramientas**: endpoint `PUT /api/tools/{id}/toggle`, `ToolsRepo`, `set_disabled` del registry, la columna `enabled` de `tools` y sus tests. `GET /api/tools` se retira si queda sin uso.
- **UI**: nueva pestaña **`Skills`** (nivel superior, ya no dentro de «Prompts») con **una pestaña por skill** —interruptor habilitar/deshabilitar, umbral, pregunta, criterio SÍ, criterio NO y prompt—; la pestaña **`Enrutador de skills`** (antes «Herramientas») conserva solo lo global —activar enrutado y modelo de decisiones—; se retira `ToolsTab`.
- **Migración**: materializa el umbral efectivo actual de cada skill (si el global se había personalizado, se copia; `widgets` conserva su `0.20`), siembra las claves `ROUTER_SKILL_<ID>_ENABLED`, retira `ROUTER_THRESHOLD` y dropea la columna `enabled`.

## Capabilities

### New Capabilities
- (ninguna)

### Modified Capabilities
- `orchestrator/skill-router`: la selección se filtra por skills habilitadas (filtro absoluto); se elimina el umbral global en favor del umbral por skill; `GET /api/skills` expone `enabled`.
- `skill-router-ui`: la configuración de skills se muda a una pestaña propia con una pestaña por skill; la pestaña del enrutador pasa a exponer solo lo global.
- `tools/registry`: el registry deja de filtrar por el estado habilitado de las tools (el filtrado por dominio lo hace el enrutador); se retira el andamiaje de habilitación por tool.

## Impact

- `src/orchestrator/skill_router.rs` (configuración sin umbral global, filtro por skill habilitada), `src/orchestrator/skills.rs` (catálogo sin cambios).
- `src/handlers/skills.rs` + `/api/skills` (`enabled`).
- `src/handlers/tools.rs`, `src/routes/tools.rs`, `src/db/repos/tools.rs`, `src/models/tool.rs`, `src/tools/registry.rs` (retirada del toggle).
- Nueva migración de `settings` + `ALTER TABLE tools DROP COLUMN enabled`.
- `frontend/src/components/SettingsDialog.tsx`, `SkillPromptFields.tsx` → nueva pestaña `Skills`, `RouterControl.tsx`, `skillRouter.ts`, `hooks/useSkills.ts`, `types`, `api/client.ts`; se eliminan `ToolsTab.tsx` y `hooks/useTools.ts`.
