# PLAN-003 — Selección por skills y otros temas

**Proyecto:** Valet (Rust/Axum + React + SQLite)
**Estado:** 🟢 Activo
**Tema 1:** ✅ Cerrado (PR #155, merge `d109f8d`)
**Tema 2:** ✅ Cerrado (PR #159, merge `bd2892e`)
**Tema 3:** ✅ Cerrado (PR #163, merge `46b3e0d`)
**Tema 4:** 📝 Spec en revisión (change OpenSpec `strava-diagnostics`)
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

**Estado:** ✅ Completado en el PR #155 (merge `d109f8d`). Implementado y archivado en OpenSpec como `2026-10-09-skill-selection`.

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

- [x] Crear el change proposal (`skill-selection`): proposal + specs (contratos + escenarios Given/When/Then).
- [x] Escenarios cubiertos: skill deshabilitada no enruta ni expone tools **con router on y off**; umbral per-skill; `GET /api/skills` con `enabled`.
- [x] Poblar `tasks.md` con el checklist TDD.
- [x] **STOP** y esperar aprobación explícita del usuario.

### F1 — Backend (rojo→verde→refactor)

- [x] Config del router: `read_router_config` con `enabled` por skill y umbral per-skill **sin global**.
- [x] `effective_threshold` sin `ROUTER_THRESHOLD`; fallback al `threshold` compilado del catálogo.
- [x] Filtrado absoluto de skills deshabilitadas (no enruta, no expone tools, no inyecta fragmento) con `ROUTER_ENABLED` on y off.
- [x] `compose_skill_fragments` salta las skills deshabilitadas.
- [x] `GET /api/skills` expone `enabled` por skill (`SkillView`).
- [x] Retirada del toggle de tools (handler, ruta, repo, modelo, registro, cableado en `main.rs`).
- [x] Migración nueva (seed `ROUTER_SKILL_<ID>_ENABLED`, materialización de umbrales, retirada de `ROUTER_THRESHOLD`, `DROP COLUMN enabled`).
- [x] Tests unitarios en `skill_router.rs`/`skills.rs` y de handler.

### F2 — Frontend (rojo→verde→refactor)

- [x] Pestaña top-level `Skills` con una pestaña por skill (switch + umbral + 4 textos).
- [x] Pestaña `Enrutador de skills` sin umbral global ni lista de tools (solo activar enrutado + modelo de decisiones).
- [x] `RouterControl` depurado; helpers en `skillRouter.ts` (clave `enabled`, umbral per-skill).
- [x] Eliminar `ToolsTab` y `useTools`; limpiar `getTools`/`toggleTool` del cliente.
- [x] Tipos: añadir `enabled` a las skills en `types/index.ts` y `useSkills.ts`.
- [x] Tests de `SettingsDialog`/componentes con vitest + testing-library.

### F3 — Limpieza en cascada

- [x] `clippy -D warnings` en verde (0 warnings).
- [x] Eliminar código muerto (rutas/handlers/repo/modelo/registro/tests).
- [x] `cargo fmt`.
- [x] `tsc --noEmit`.
- [x] Lint de frontend.
- [x] `openspec validate --all --strict`.

### F4 — Cierre

- [x] Archivar el change.
- [x] Actualizar `AGENTS.md § V` si procede.

---

## Definición de Hecho (DoD)

- [x] Skill deshabilitada nunca enruta ni expone sus tools ni inyecta su fragmento, con `ROUTER_ENABLED` on y off (test que lo verifica).
- [x] Umbral por skill funcional sin `ROUTER_THRESHOLD`; fallback al default del catálogo.
- [x] `GET /api/skills` expone `enabled`.
- [x] Toggle de tools eliminado (sin código muerto; `clippy` 0 warnings).
- [x] Migración verificada (materializa umbrales, añade claves enabled, retira global, dropea columna).
- [x] UI: `Skills` top-level con pestaña por skill; `Enrutador de skills` sin umbral global; `ToolsTab` fuera.
- [x] `cargo test`, `npx vitest run`, `tsc --noEmit`, lint y `openspec validate --all --strict` en verde.

---

## Riesgos / notas

- Es un cambio **medio-grande** con impacto en el **arnés de medición** del router y en los tests de integridad del catálogo.
- Quitar `ROUTER_THRESHOLD` es una decisión de producto: deja una única fuente de umbral (per-skill).
- Con 0 skills activas solo queda el core: añadir aviso en la UI.

---

## Tema 2 — Conciencia temporal

**Estado:** ✅ Completado en el PR #159 (merge `bd2892e`). Implementado y archivado en OpenSpec como `2026-10-09-temporal-awareness`.

### Objetivo

Que el modelo sepa **qué hora es realmente** en cada turno y **cuándo** se dijo cada mensaje del historial, de modo que resuelva «hoy», «ayer» y «mañana» contra la fecha real y no contra una que leyó turnos atrás y ya caducó.

### Decisiones de diseño (BLOQUEADAS — respétalas tal cual)

1. **Sección temporal siempre presente** en el mensaje de sistema, en cada turno (no solo cuando el navegador manda contexto), y **cierra** el mensaje. Formato: `Fecha y hora actual: 2026-10-09 19:00:00 (viernes). Evalúa 'hoy', 'ayer' y 'mañana' respecto a este timestamp.`
2. **La sección de ubicación manda solo la ubicación**: la antigua sección «browser» pierde la fecha; conserva `Ubicación: …` / `Coordenadas: …` y se omite si no hay coordenadas.
3. **Marca de tiempo por mensaje**: los mensajes `user` y `assistant` (historial y turno) se prefijan con `[YYYY-MM-DD HH:MM]` en la zona efectiva; los `tool` y los vacíos, no. La marca **no se persiste**: solo viaja en la petición al LLM.
4. **Instante y zona efectivos por turno**: instante = `BrowserContext.timestamp` si es válido, si no `Utc::now()`; zona = `BrowserContext.timezone` → `settings.timezone` → `Europe/Madrid`; zona inválida → UTC.

### Mapa arquitectónico

- `src/tools/time_format.rs` — formateadores nuevos (`format_inline_timestamp`, `format_prompt_now`), reutilizando `chrono_tz` y los nombres en español.
- `src/orchestrator/agent.rs` — resolución del instante/zona del turno; sección temporal; sección de ubicación sin fecha; prefijado del historial y del mensaje del turno; `compose_system_message` gana la sección temporal final.

### Specs afectadas

- `orchestrator/agent` (un requisito modificado, uno retirado y cuatro añadidos). Sin cambios de esquema, migraciones, API ni UI.

### Tareas técnicas (TDD — checklist)

- [x] F0 — Change OpenSpec `temporal-awareness` (aprobado antes de codificar).
- [x] F1 — RED: formateadores y ensamblado (sección temporal siempre presente y cierra; ubicación sin fecha; prefijos del historial y del turno).
- [x] F2 — GREEN: `time_format.rs` + `agent.rs`.
- [x] F3 — REFACTOR: `cargo fmt`, `clippy -D warnings`, código muerto del formato antiguo.
- [x] F4 — VERIFY: `cargo test`, review y archivo del change.

---

## Tema 3 — Skill de running (Strava)

**Estado:** ✅ Completado en el PR #163 (merge `46b3e0d`). Implementado y archivado en OpenSpec como `2026-10-10-strava-running`.

### Objetivo

Que el asistente pueda **consultar y analizar tus sesiones de running** desde la API de Strava v3: qué has corrido, cómo fue una sesión concreta, el detalle fino (ritmo/FC/cadencia) y los totales. **Solo lectura**: no se crea ni se modifica nada en Strava.

### Decisiones de diseño (BLOQUEADAS — aprobadas el 2026-10-10)

1. **Solo lectura.** Scopes `read,activity:read_all`. Fuera `activity:write`.
2. **OAuth dentro de la app.** Rutas propias `GET /api/strava/authorize` (redirige a `https://www.strava.com/oauth/authorize` con `state`) y `GET /api/strava/callback` (canjea el `code`), más `GET /api/strava/status` y `POST /api/strava/disconnect`. Los tokens viven en `settings`, como el resto de claves.
3. **Rotación del refresh token.** Cada respuesta de `POST /oauth/token` trae un refresh token **nuevo** que invalida el anterior: hay que **persistir el nuevo en cada refresco** y hacerlo en **un único punto de refresco por atleta** (lock), o dos turnos simultáneos se pisan y la integración queda muerta hasta reconectar.
4. **Consulta en vivo con caché corta.** Nada de sincronizar a una tabla local. Se respeta `X-RateLimit-*` y se degrada con un mensaje claro ante `429`.
5. **Skill nueva `running`** (7ª del catálogo) con 4 tools. Los canarios de integridad del catálogo (6 skills / 13 tools) suben a **7 / 17** en el mismo cambio. El fragmento `SKILL_RUNNING_PROMPT` se siembra por migración.
6. **Sin widget** de gráficas en esta entrega (segunda iteración).
7. **No se guardan actividades**; solo los tokens.

### Tools de la skill (4)

| Tool | Endpoint | Para qué |
|:--|:--|:--|
| `strava_recent_activities` | `GET /athlete/activities` (`before`, `after`, `page`, `per_page`) | listado con filtro de deporte (Run/TrailRun/VirtualRun) |
| `strava_activity_detail` | `GET /activities/{id}` | una sesión con vueltas y splits |
| `strava_activity_streams` | `GET /activities/{id}/streams` (`keys`, `key_by_type`) | series de ritmo/FC/cadencia/altitud |
| `strava_athlete_stats` | `GET /athlete` + `GET /athletes/{id}/stats` | perfil y totales (año/recientes) |

Prerrequisito del usuario: registrar la app en `strava.com/settings/api` y fijar el *Authorization Callback Domain* al dominio público de la instancia (`localhost`/`127.0.0.1` valen en local). Las apps nuevas nacen en **Single Player Mode** (capacidad de 1 atleta).

### Claves en `settings` (con respaldo ENV)

`strava_client_id` (`STRAVA_CLIENT_ID`), `strava_client_secret` (`STRAVA_CLIENT_SECRET`), `strava_refresh_token`, y el estado derivado `strava_access_token` / `strava_expires_at` / `strava_athlete_id` / `strava_scope`.

### Mapa arquitectónico

Backend:

- `src/tools/strava.rs` (o `src/tools/strava/`) — cliente HTTP + las 4 tools.
- `src/services/strava.rs` (o `src/integrations/strava.rs`) — OAuth, refresco con rotación y lock, lectura de credenciales.
- `src/routes/strava.rs` + `src/handlers/strava.rs` — authorize / callback / status / disconnect.
- `src/orchestrator/skills.rs` — variante `Skill::Running` + `SkillSpec` + actualización de canarios.
- `src/lib.rs` — registrar las 4 tools en `build_tool_registry` (13 → 17).
- `src/config.rs` — `STRAVA_CLIENT_ID` / `STRAVA_CLIENT_SECRET` (respaldo).
- `src/db/repos/settings.rs` — seed de las claves nuevas.
- Migración nueva — seed de claves + `SKILL_RUNNING_PROMPT`.

Frontend:

- `SettingsDialog.tsx` — sección «Integraciones» (o sub-pestaña en Skills) con el botón oficial **«Connect with Strava»**, estado y desconectar.
- `api/client.ts`, `types/index.ts`, hook `useStrava` — estado de la conexión.
- La 7ª skill aparece sola en `SkillsTab` (el catálogo lo sirve la API).

### Specs afectadas

- **Nueva** `openspec/specs/strava/spec.md` (OAuth, tokens, rotación, tools, errores, límites).
- `orchestrator/skill-router` (MODIFIED: catálogo de 7 skills + fragmento + umbral).
- `tools` (MODIFIED: las 4 tools nuevas).
- `frontend` (MODIFIED: UI de conexión), si procede.

### Tareas técnicas (TDD — checklist)

- [x] F0 — Change OpenSpec `strava-running` (proposal + specs + tasks) → **STOP** y aprobación.
- [x] F1 — RED: contrato de las tools (args/errores) con el cliente HTTP mockeado; OAuth (state, callback, persistencia); **rotación** del refresh token con dos refrescos concurrentes; catálogo de 7 skills.
- [x] F2 — GREEN: servicio Strava + tools + rutas + catálogo + migración.
- [x] F3 — REFACTOR: `cargo fmt`, `clippy -D warnings`, canarios actualizados.
- [x] F4 — VERIFY: `cargo test`, review, archivo del change; UI con `vitest`.
- [x] F5 — Cierre: actualizar este plan y `AGENTS.md §V` si procede.

### DoD

- [x] `cargo test`, `npx vitest run`, `tsc --noEmit`, lint y `openspec validate --all --strict` en verde.
- [x] Las 4 tools se anuncian con la skill activa y no sin ella.
- [x] El refresco persiste el refresh token nuevo y sobrevive a dos refrescos concurrentes (test).
- [x] Un `429` de Strava devuelve un mensaje claro, no un error crudo.
- [x] `openspec archive strava-running`.

### Riesgos / notas

- **La rotación del refresh token** es la fuente #1 de fallos; el test de concurrencia es obligatorio.
- El `redirect_uri` debe colgar del dominio registrado; en producción, la URL pública tras Traefik.
- Rate limits: 100 req/15 min y 1000/día (no-upload) → caché y nada de polling.
- Los tokens quedan **en claro** en `settings`, como el resto de claves; el refresh token es el más sensible (lectura completa de la cuenta).
- El botón de conexión debe ser el oficial «Connect with Strava» (condiciones de la API).

---

### Notas de cierre

- **Requisito previo del usuario**: registrar la app en `strava.com/settings/api` (Single Player Mode, un atleta), fijar el *Authorization Callback Domain* y pegar `client_id`/`client_secret` en la sección «Integraciones» (o definirlos por `STRAVA_CLIENT_ID`/`STRAVA_CLIENT_SECRET`).
- **Los tokens OAuth no viajan al navegador**: `/api/settings` omite `strava_access_token` y `strava_refresh_token` y los ignora en escritura (`SENSITIVE_KEYS`).
- **Refuerzos tras la revisión**: el `refresh_token` rotado se confirma en su propia transacción (un fallo posterior no lo pierde); la caché purga lo caducado; `before`/`after` aceptan ISO 8601 o epoch.
- **Segunda iteración**: el widget de gráficas (ritmo/volumen semanal).

## Tema 4 — Diagnóstico de la conexión con Strava

**Estado:** 📝 Change OpenSpec `strava-diagnostics` creado; pendiente de aprobación.

### Objetivo

Que Valet **diga por qué** falla la integración de Strava. El 2026-10-10 una instancia en producción
devolvía `403` en todas las llamadas con la app de Strava **inactiva** (su propietario sin suscripción,
requisito de Strava desde el 1 de julio de 2026) y hubo que diagnosticarlo fuera de Valet —sacando el
token de la base de datos con `sqlite3` y preguntando con `curl`— porque la aplicación descartaba el
cuerpo de la respuesta y mostraba «Conectada como \<atleta\>» mientras todo fallaba.

### Decisiones de diseño (BLOQUEADAS)

1. **El cuerpo del error de Strava viaja al usuario**: `message` + `errors[]` (`field`/`code`).
2. **`Application/Status/Inactive` tiene error propio y mensaje accionable** (suscripción de la cuenta
   propietaria + `https://www.strava.com/settings/api`).
3. **`GET /api/strava/check` responde siempre `200`** con `{ok, athlete_id, athlete_name, error}`;
   consulta `/athlete` **sin caché**.
4. **Desconectar limpia siempre** y **avisa** cuando la revocación no se confirma.
5. **El `scope` concedido se muestra** en «Integraciones», con aviso si falta `activity:read_all`.
6. Fuera de alcance: `approval_prompt=force`, persistir el último error, widget de gráficas, MCP.

### Mapa arquitectónico

- `src/services/strava.rs` — `StravaError::ApplicationInactive`, parseo del cuerpo, `Strava::check`, `disconnect` con aviso.
- `src/handlers/strava.rs` + `src/routes/strava.rs` — `GET /api/strava/check` y desconexión con aviso.
- `frontend/src/components/StravaIntegration.tsx`, `hooks/useStrava.ts`, `api/client.ts`, `types/index.ts`.

### Specs afectadas

- `tools/strava` (MODIFIED: estado/desconexión + fallos accionables).
- `strava-ui` (MODIFIED: sección «Integraciones»).

### Tareas técnicas (TDD — checklist)

- [ ] F0 — Change OpenSpec `strava-diagnostics` (proposal + design + specs + tasks) → **STOP** y aprobación.
- [ ] F1 — RED: mapeo del cuerpo de error, `check` sin caché, desconexión con aviso, handler, UI.
- [ ] F2 — GREEN: servicio + rutas + frontend.
- [ ] F3 — REFACTOR: `cargo fmt`, `clippy -D warnings`, `tsc`, lint.
- [ ] F4 — VERIFY: `cargo test`, `vitest`, `openspec validate --strict`, reviews.
- [ ] F5 — Cierre: archivar el change, actualizar este plan y `AGENTS.md § V` si procede.

### DoD

- [ ] Un `403` de aplicación inactiva produce un mensaje que nombra la suscripción y la URL de reactivación (test).
- [ ] Un `403` genérico incorpora el `message` y el `field`/`code` de Strava (test).
- [ ] `GET /api/strava/check` responde `200` con `ok:false` + mensaje cuando la app está inactiva (test).
- [ ] La desconexión borra los tokens siempre y avisa cuando la revocación no se confirma (test).
- [ ] `cargo test`, `npx vitest run`, `tsc --noEmit`, lint y `openspec validate --all --strict` en verde.
- [ ] `openspec archive strava-diagnostics`.

### Riesgos / notas

- El estado `Application/Status/Inactive` **no** es un fallo de Valet: solo se puede resolver en Strava.
- `GET /api/strava/check` sale a la red por diseño: no debe usarse en bucle.
- No se persiste estado nuevo: el diagnóstico no añade claves a `settings`.

## Temas pendientes

Temas 1, 2 y 3 cerrados (PR #155, #159 y #163). Tema 4 en spec (`strava-diagnostics`).
