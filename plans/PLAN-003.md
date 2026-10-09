# PLAN-003 — Selección por skills y otros temas

**Proyecto:** Valet (Rust/Axum + React + SQLite)
**Estado:** 🟢 Activo
**Fecha:** 2026-10-09
**Metodología:** OpenSpec (SDD) + TDD (Red-Green-Refactor)

---

## Contexto (lo que hay hoy)

- Las *skills* son el catálogo cerrado de 6 dominios definido en código (`src/orchestrator/skills.rs`): `agenda`, `pendientes`, `recuerdos`, `entorno`, `web`, `widgets`. Cada `SkillSpec` tiene `id`, `instructions` (pregunta), `criteria_true`, `criteria_false`, `threshold` (default compilado), `tools`, `prompt_key`, `prompt_heading`.
- Herramientas *core* (`CORE_TOOLS`): `get_current_time`, `get_current_location` — siempre expuestas, nunca enrutadas.
- Un test de integridad garantiza que **toda tool registrada pertenece al core o a una skill** (el catálogo cubre el 100 % de las tools → la selección por tool es redundante).
- El *router* (`src/orchestrator/skill_router.rs`) lee de `settings`: `ROUTER_ENABLED` (on/off), `ROUTER_THRESHOLD` (umbral global, default 0.10), `ROUTER_MODEL` (modelo de decisiones), `ROUTER_THRESHOLD_<ID>` (override por skill; hoy solo `ROUTER_THRESHOLD_WIDGETS=0.20`). Funciones clave: `read_router_config`, `effective_threshold`, `effective_field`, `compose_skill_fragments`.
- UI actual en `frontend/src/components/SettingsDialog.tsx`: pestaña **«Herramientas»** = `RouterControl` (activar enrutador, umbral global, modelo, lista de umbrales por skill) **+ `ToolsTab`** (lista de tools con switch habilitar/deshabilitar). Y dentro de **Prompts > sub-pestaña «Skills»** (`SkillPromptFields.tsx`) están los 4 textos por skill (pregunta, criterio SÍ, criterio NO, fragmento de prompt).
- El toggle de tool NO es solo UI: `PUT /api/tools/{id}/toggle` (`src/handlers/tools.rs`) hace `UPDATE tools SET enabled = ...`, y luego sincroniza el registro en memoria con `registry.set_disabled(...)` (`src/tools/registry.rs`), de modo que una tool desactivada se oculta del prompt y se rechaza al ejecutarse. Columna en `migrations/20260925000001_initial.sql`: `enabled INTEGER NOT NULL DEFAULT 1`.

---

## Tema 1 — Selección por skills

### Objetivo

Que la unidad seleccionable sea la **skill**, no la tool. El usuario podrá **habilitar/deshabilitar cada skill** a voluntad, de forma que una skill deshabilitada **nunca entre en el enrutado**. Se retira por completo la selección de tools (redundante).

### Decisiones de diseño (BLOQUEADAS — respétalas tal cual)

1. **Skill deshabilitada = filtro absoluto**: nunca se enruta, no expone sus tools y no inyecta su fragmento de prompt, **aunque `ROUTER_ENABLED` esté apagado** (hoy, con el router apagado, se exponen todas las tools sin filtrar: esto debe cambiar para respetar el filtro).
2. **Umbral 100 % per-skill**: se **elimina** `ROUTER_THRESHOLD` (global). Cada skill usa su `ROUTER_THRESHOLD_<ID>`; si no hay valor, **fallback** = `threshold` compilado del catálogo.
3. **Se elimina del todo el toggle de tools**: endpoint, repo, `set_disabled` del registro, columna `enabled` y sus tests. El estado `enabled` previo **se descarta** (no se traduce a skills).
4. **Core** (`get_current_time`, `get_current_location`) sigue siempre expuesto.
5. Persistencia nueva: clave `ROUTER_SKILL_<ID>_ENABLED` (`true` por defecto; ausencia = habilitada). `GET /api/skills` expone `enabled` por skill.

### Estructura de la UI resultante

- Pestaña **`Skills`** (nivel superior de Settings, ya NO dentro de Prompts): **una pestaña por skill** (6), cada una con: **switch habilitar/deshabilitar · umbral · pregunta · criterio SÍ · criterio NO · prompt**. Guardado propio (un solo dueño de esas claves).
- Pestaña **`Enrutador de skills`** (renombra a la actual «Herramientas»): solo lo global → **activar enrutado** (switch; renómbralo de «Enrutador de skills» a «Activar enrutado» para no chocar con el nombre de la pestaña) + **modelo de decisiones** (`ROUTER_MODEL`). Sin umbral global ni lista de tools.
- Se elimina `ToolsTab` y su hook `useTools`.

### Migración (decidida)

- **Fleco 1 → B**: al quitar el global, **materializar el umbral efectivo actual** de cada skill en `ROUTER_THRESHOLD_<ID>` (si el global era, p. ej., 0.15, las cinco de dominio pasan a 0.15; `widgets` conserva 0.20). Si el global estaba en su default y la skill no tenía override, no hace falta fila: el fallback del catálogo ya da el valor correcto.
- **Fleco 2 → A**: descartar el estado `enabled` de tools; `ALTER TABLE tools DROP COLUMN enabled` (SQLite ≥ 3.35) o dejar la columna inerte — recomienda DROP COLUMN y justifícalo.
- Eliminar/neutralizar la clave `ROUTER_THRESHOLD` (el router deja de leerla).

### Mapa arquitectónico (ficheros afectados — verifica los nombres con el repo, pero son estos)

Backend:

- `src/orchestrator/skills.rs` — catálogo (posiblemente sin cambios; es la fuente de verdad).
- `src/orchestrator/skill_router.rs` — `read_router_config`, `effective_threshold` (sin global), filtrado de skills deshabilitadas, `compose_skill_fragments` (saltar deshabilitadas).
- `src/handlers/skills.rs` — `GET /api/skills` + `SkillView` (añadir `enabled`).
- `src/handlers/tools.rs` + `src/routes/tools.rs` — eliminar `toggle_tool` (y su ruta); `list_tools` si ya no se usa.
- `src/db/repos/tools.rs` — eliminar `ToolsRepo`/`toggle_enabled`/`disabled_names` (o podar).
- `src/models/tool.rs` — quitar el campo `enabled`.
- `src/tools/registry.rs` — quitar `set_disabled`.
- `src/main.rs` — quitar el cableado inicial de `set_disabled`.

Migración nueva (SQL idempotente, mismo idioma que `20261008000001_skill_router_tuning.sql`): seed `ROUTER_SKILL_<ID>_ENABLED`, materialización de umbrales, retirada de `ROUTER_THRESHOLD`, `DROP COLUMN enabled`.

Frontend:

- `frontend/src/components/SettingsDialog.tsx` — nueva pestaña top-level `Skills`; renombrar `Herramientas`→`Enrutador de skills`; quitar `ToolsTab`.
- `frontend/src/components/SkillPromptFields.tsx` — reconvertir a pestañas por skill con switch+umbral+4 textos (o nuevo componente `SkillsPanel`).
- `frontend/src/components/RouterControl.tsx` — quitar umbral global y lista de umbrales por skill.
- `frontend/src/components/skillRouter.ts` — helpers (clave `enabled`, umbral siempre per-skill).
- `frontend/src/components/ToolsTab.tsx` y `frontend/src/hooks/useTools.ts` — eliminar.
- `frontend/src/hooks/useSkills.ts`, `frontend/src/types/index.ts`, `frontend/src/api/client.ts` — tipos/hook/cliente (añadir `enabled`; quitar `getTools`/`toggleTool`).

---

## Tareas técnicas (TDD — checklist)

### F0 — Change OpenSpec (`skill-selection`)

- [ ] Crear el change proposal (`skill-selection`): proposal + specs (contratos + escenarios Given/When/Then).
- [ ] Escenarios cubiertos: skill deshabilitada no enruta ni expone tools **con router on y off**; umbral per-skill; `GET /api/skills` con `enabled`.
- [ ] Poblar `tasks.md` con el checklist TDD.
- [ ] **STOP** y esperar aprobación explícita del usuario.

### F1 — Backend (rojo→verde→refactor)

- [ ] Config del router: `read_router_config` con `enabled` por skill y umbral per-skill **sin global**.
- [ ] `effective_threshold` sin `ROUTER_THRESHOLD`; fallback al `threshold` compilado del catálogo.
- [ ] Filtrado absoluto de skills deshabilitadas (no enruta, no expone tools, no inyecta fragmento) con `ROUTER_ENABLED` on y off.
- [ ] `compose_skill_fragments` salta las skills deshabilitadas.
- [ ] `GET /api/skills` expone `enabled` por skill (`SkillView`).
- [ ] Retirada del toggle de tools (handler, ruta, repo, modelo, registro, cableado en `main.rs`).
- [ ] Migración nueva (seed `ROUTER_SKILL_<ID>_ENABLED`, materialización de umbrales, retirada de `ROUTER_THRESHOLD`, `DROP COLUMN enabled`).
- [ ] Tests unitarios en `skill_router.rs`/`skills.rs` y de handler.

### F2 — Frontend (rojo→verde→refactor)

- [ ] Pestaña top-level `Skills` con una pestaña por skill (switch + umbral + 4 textos).
- [ ] Pestaña `Enrutador de skills` sin umbral global ni lista de tools (solo activar enrutado + modelo de decisiones).
- [ ] `RouterControl` depurado; helpers en `skillRouter.ts` (clave `enabled`, umbral per-skill).
- [ ] Eliminar `ToolsTab` y `useTools`; limpiar `getTools`/`toggleTool` del cliente.
- [ ] Tipos: añadir `enabled` a las skills en `types/index.ts` y `useSkills.ts`.
- [ ] Tests de `SettingsDialog`/componentes con vitest + testing-library.

### F3 — Limpieza en cascada

- [ ] `clippy -D warnings` en verde (0 warnings).
- [ ] Eliminar código muerto (rutas/handlers/repo/modelo/registro/tests).
- [ ] `cargo fmt`.
- [ ] `tsc --noEmit`.
- [ ] Lint de frontend.
- [ ] `openspec validate --all --strict`.

### F4 — Cierre

- [ ] Archivar el change.
- [ ] Actualizar `AGENTS.md § V` si procede.

---

## Definición de Hecho (DoD)

- [ ] Skill deshabilitada nunca enruta ni expone sus tools ni inyecta su fragmento, con `ROUTER_ENABLED` on y off (test que lo verifica).
- [ ] Umbral por skill funcional sin `ROUTER_THRESHOLD`; fallback al default del catálogo.
- [ ] `GET /api/skills` expone `enabled`.
- [ ] Toggle de tools eliminado (sin código muerto; `clippy` 0 warnings).
- [ ] Migración verificada (materializa umbrales, añade claves enabled, retira global, dropea columna).
- [ ] UI: `Skills` top-level con pestaña por skill; `Enrutador de skills` sin umbral global; `ToolsTab` fuera.
- [ ] `cargo test`, `npx vitest run`, `tsc --noEmit`, lint y `openspec validate --all --strict` en verde.

---

## Riesgos / notas

- Es un cambio **medio-grande** con impacto en el **arnés de medición** del router y en los tests de integridad del catálogo.
- Quitar `ROUTER_THRESHOLD` es una decisión de producto: deja una única fuente de umbral (per-skill).
- Con 0 skills activas solo queda el core: añadir aviso en la UI.

---

## Temas pendientes

_Pendiente: se irán añadiendo más piezas a este plan._
