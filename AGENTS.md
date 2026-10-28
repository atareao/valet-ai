# AGENT DIRECTIVES: OPENSPEC (SDD) + TDD WORKFLOW

## ⚠️ REGLA DE ORO — LEER ANTES DE ACTUAR

**ANTES de escribir o editar CUALQUIER archivo de código fuente (Rust, TypeScript, JSX, CSS, etc.),
debes ejecutar `just check-spec` para confirmar que existe un change proposal aprobado.**

Si `just check-spec` falla:
1. DETENTE inmediatamente.
2. Informa al usuario que no hay un change proposal activo.
3. Pregunta si quiere crear uno con `openspec new change <feature>`.
4. NO escribas código hasta recibir aprobación explícita.

**SALTARSE ESTE PASO ES VIOLACIÓN DEL PROTOCOLO.**

---

## I. CORE PRINCIPLES & GOALS

- **Phase 0 — Legacy Support:** If modifying existing code without specs or tests, establish a baseline spec and characterization tests before introducing changes.
- **Phase 1 — SDD (OpenSpec):** No new code or tests may be written before a spec change proposal exists in `openspec/changes/<feature>/` and is approved by the user.
- **Phase 2 — TDD (Red-Green-Refactor):** Once the spec is approved, code MUST be developed strictly test-first using terminal commands.
- **Strict Verification:** Always run CLI test suites using terminal tools. Never assume code or tests pass/fail without CLI confirmation.

---

## II. EXECUTION WORKFLOW

### Phase 0: Legacy Code Preparation (Conditional)

*Execute this phase ONLY if modifying an existing module/file that lacks OpenSpec documentation or tests.*

1. **Characterization Spec (As-Is):**
   - Inspect the target file/module.
   - Generate a baseline spec in `openspec/specs/<module>/spec.md` reflecting current behavior.
2. **Characterization Tests:**
   - Write Rust (`#[test]`) or React/TS (`vitest` / `@testing-library/react`) tests matching current behavior.
   - Run tests via CLI (`cargo test` or `npx vitest run`) to confirm all pass in **GREEN**.

### Phase 1: SDD Protocol (OpenSpec)

When the user requests a new feature, bug fix, or refactor:

1. **Create the Change Proposal:**
   - Execute CLI command: `openspec new change <feature-name>`
2. **Draft Specifications:**
   - Populate `openspec/changes/<feature-name>/proposal.md` with intent, scope, and impact.
   - Create spec deltas in `openspec/changes/<feature-name>/specs/<module>/spec.md`.
   - Ensure the spec includes:
     - **Contracts:** Rust types/structs/enums, TypeScript interfaces/props, API endpoints, or function signatures.
     - **Scenarios (BDD style):** Detailed `Given / When / Then` clauses for happy path, error cases, and edge cases.
   - Populate `openspec/changes/<feature-name>/tasks.md` with the TDD task checklist.
3. **STOP & WAIT FOR APPROVAL:**
   - Present the created specification to the user.
   - **DO NOT** write application code or new tests until the user explicitly approves the spec.

### Phase 2: TDD Protocol (Red-Green-Refactor)

Once the user approves the spec (e.g., "Approved", "Looks good", "Proceed with TDD"):

1. **RED (Write Failing Tests):**
   - Read the `Given / When / Then` scenarios in `openspec/changes/<feature-name>/specs/`.
   - Write tests in Rust or React/TypeScript corresponding to those scenarios.
   - Execute CLI tests (`cargo test` or `npx vitest run`).
   - **Verify:** Confirm test failure for the new functionality while any legacy tests remain **GREEN**.
2. **GREEN (Minimal Implementation):**
   - Write the absolute minimum code necessary to satisfy the failing tests.
   - Execute CLI tests (`cargo test` or `npx vitest run`).
   - Run type checks (`cargo check` or `npx tsc --noEmit`).
   - **Verify:** Confirm all tests pass (100% green) and no compilation/type errors exist.
3. **REFACTOR (Clean & Consolidate):**
   - Clean up code formatting, types, and structure without altering behavior.
   - Run linters (`cargo clippy -- -D warnings` / `npm run lint`).
   - Re-run test suites via CLI to guarantee no regressions.
4. **CONSOLIDATE & ARCHIVE:**
   - Mark completed items in `tasks.md`.
   - Once all scenarios pass, run `openspec archive <feature-name>` to merge the delta into `openspec/specs/`.

### Practical Lessons Learned (SDD + TDD)

#### Archive requires exact header matching
`openspec archive` busca el header exacto del delta en la spec destino. Si el header del delta es `"### Requirement: Pipeline evaluation order (WAF first)"` pero la spec tiene `"### Requirement: Pipeline evaluation order"`, el archive falla. **Los headers del delta deben copiar EXACTAMENTE los de la spec destino.**

#### Si reescribes la spec directamente, no intentes archivar
Si modificaste `openspec/specs/<module>/spec.md` a mano (fuera del mecanismo de archive), el change proposal correspondiente queda huérfano. No se puede archivar porque los headers ya no coinciden. **Solución: eliminar el directorio del change proposal** (`rm -rf openspec/changes/<feature>/`).

#### Cambios en cascada
Eliminar una entidad (ej. Whitelist/Blacklist) puede dejar código muerto en otras partes (ej. `AppError::Conflict`, tests de Conflict). El REFACTOR phase debe incluir la limpieza de estos artefactos. **Siempre ejecutar `cargo clippy -- -D warnings` tras el GREEN phase para detectar código/ variantes no usados.**

#### `openspec archive --yes` no bypassa validación de headers
La flag `--yes` salta la comprobación de tareas incompletas, pero NO la validación de que los headers del delta existan en la spec destino. Si los headers no matchean, el archive igual falla.

#### Mantén openspec artifacts sincronizados con el código
Si implementas un cambio en código pero no actualizas los artifacts de openspec (tasks, proposal), el change proposal queda "stuck" — no se puede archivar ni continuar. **Antes de empezar un nuevo cambio, verifica que no haya cambios activos huerfanos con `openspec list`.**

---

## III. PROJECT CONFIGURATION & CONVENTIONS

### Stack Commands

#### Backend: Rust
- **Test Runner:** `cargo test` (or `cargo nextest run` if available).
- **Type Checking & Linting:** `cargo check` and `cargo clippy -- -D warnings` (enforce zero warnings).
- **Formatting:** `cargo fmt --check`
- **Conventions:**
  - Structs and types placed in domain modules or `src/models/`.
  - Unit tests placed in the same file under `#[cfg(test)]`.
  - Integration and API tests placed in `tests/`.

#### Frontend: React + TypeScript
- **Test Runner:** `npx vitest run` or `npm test -- --watch=false` (single-pass execution).
- **Type Checking:** `npx tsc --noEmit` (mandatory during GREEN/REFACTOR steps).
- **Linting & Formatting:** `npm run lint` / `npx eslint .`
- **Conventions:**
  - Components in `src/components/`, hooks in `src/hooks/`.
  - Component tests colocated as `Component.test.tsx` using `@testing-library/react`.
  - User-centric testing behavior using `@testing-library/user-event` instead of implementation details.

### Custom Repository Rules

#### Desarrollo: Podman
El proyecto usa **Podman** como runtime de contenedores para desarrollo local.
- `just dev` → `podman compose up -d --build` (reconstruye imagen + arranca)
- `just dev-docker` → alternativa con Docker
- El binario de Podman está en `/usr/bin/podman`
- Las imágenes se construyen con `podman compose build`

#### Entorno de producción
- `docker-compose.prod.yml` despliega con frontend separado (nginx) + PocketID
- `docker-compose.yml` es para desarrollo con frontend embebido

---

## IV. RESPONSE FORMAT & STATUS MESSAGES

Always prefix your progress updates with the current status tag:

```text
[LEGACY - INSPECT] Creating baseline spec & characterization tests.
[OPENSPEC - DRAFT] Generating change proposal in openspec/changes/...
[OPENSPEC - WAITING] Spec generated. Awaiting user review and approval.
[TDD - RED] Creating tests for scenario <Name> -> Running CLI tests.
[TDD - GREEN] Implementing minimal code -> Running CLI tests & type checks.
[TDD - REFACTOR] Refactoring code -> Running Clippy/ESLint & tests.
[OPENSPEC - ARCHIVE] Archiving change into openspec/specs/.
```


## V. CURRENT PROJECT STATE

Lo **decidido**, esté aplazado o resuelto. Lo que está en marcha no se apunta aquí: vive en `openspec/specs/` y en el historial de git.

### Entorno de producción real (`co1`) — verificado el 2026-10-26

El despliegue de producción **no** usa el `docker-compose.prod.yml` del repo: lo gestiona **Dockge** en el host `co1` (`ssh co1`).

- **Stack:** `~/docker/dockge/stacks/valet/compose.yaml` (más su `.env`). Ese directorio se monta en el contenedor de Dockge como `/opt/stacks` (`DOCKGE_STACKS_DIR=/opt/stacks`), así que los labels del contenedor dicen `/opt/stacks/valet` **aunque esa ruta no exista en el host** — no confundirlo con un compose perdido. Para comprobar que un `compose.yaml` concreto gobierna el contenedor: el `com.docker.compose.config-hash` del contenedor debe coincidir con `docker compose config --hash` de ese fichero.
- **Servicio:** `valet` (`container_name: valet`), imagen `ghcr.io/atareao/valet-ai` (tag `latest`), volumen `valet_valet_data` montado en `/app/data/valet.db`.
- **Ruta pública:** `https://valet.territoriolinux.es` tras Traefik (red externa `proxy`), con `AUTH_ENABLED=true` contra el PocketID ya existente. `/api/health` responde `200` sin sesión; `/api/events` exige sesión (`401`).
- **Desplegar un release:** publicar la imagen (tag `vX.Y.Z` sobre `main`, workflow `image.yml`) y **recrear el stack desde Dockge** (equivalente a `docker compose pull && docker compose up -d` en ese directorio). Sin ese paso, el contenedor sigue con la imagen antigua.

### Aplazado por decisión del usuario — no ejecutar sin nueva orden

**1. `docker-compose.prod.yml` no se toca.**

Se mantiene tal cual (backend + frontend nginx independiente + PocketID). El despliegue de trabajo se hace con `docker-compose.yml` sobre Podman. Cambiarlo requiere orden explícita.

### Cerrado — no volver a abrir

**2. Autoarranque tras reinicio del host.** Descartado por decisión del usuario (2026-10-09): **no se habilita**. El contenedor no sobrevive a un reinicio del host y se levanta a mano si hace falta.

**3. Los ficheros sueltos de la raíz.**

`assets.svg` y `temporal.svg` llevaban sin trackear desde el 2026-09-29 y los vieron tres sesiones seguidas, que se limitaron a clasificarlos como "preexistentes y ajenos" sin decidir nada. El 2026-10-01 quedaron movidos a `~/.local/share/valet-scratch/`, con un `NOTAS.md` allí que explica qué son. Eran restos inválidos de la generación de iconos (uno de ellos ni siquiera es un SVG: es un fragmento pegado sin etiqueta `<svg>`), nunca estuvieron en git, y los iconos reales y versionados están en `frontend/src/assets/valet-icon.svg`, `assets/linux/hicolor/scalable/apps/valet.svg` y `assets/icono.svg`. Si vuelven a aparecer en la raíz, es que alguien los ha traído de vuelta a propósito.

**4. Los tokens OAuth de Strava no se exponen por la API de ajustes.** Decidido el 2026-10-10 al cerrar el Tema 3 del PLAN-003: `GET /api/settings` omite `strava_access_token` y `strava_refresh_token`, y `PUT /api/settings` los ignora. Son material de gestión interna, no ajustes editables. La lista vive en `SENSITIVE_KEYS` (`src/db/repos/settings.rs`).

**5. La granularidad de las skills y las políticas de corte de la selección (top-k, presupuesto de bytes) no se tocan.** Medido el 2026-10-11 con el arnés (`valet-route-eval`) y un medidor desechable (`/tmp/opencode/budget-eval`, fuera del repo) sobre 40 turnos reales de producción (120 evaluaciones):

- **Atomizar las skills: descartado.** El router activa *todas* las skills que superan su umbral, no una ganadora; partir una en dos suele hacer que ambas partes lo superen (los criterios se solapan) y expone **más** herramientas, no menos. El catálogo ya es atómico: 10 skills / 28 herramientas.
- **top-k: descartado.** La curva cobertura↔coste es casi lineal, sin rodilla. `topk(6)` baja la cobertura al 95 % (baseline 97,5 %) ahorrando solo un 6 % de bytes; `topk(4)` ahorra un 25 % de bytes y pierde 11 puntos de cobertura. El baseline ya expone 14 de 28 herramientas con 97,5 % de cobertura: no hay margen limpio.
- **Presupuesto de bytes: descartado.** Más agresivo que el top-k y sin ganancia a igual cobertura (`bytes(8000)` → 89,2 %).
- **Causa de fondo:** cuatro skills superan su umbral casi siempre (`recuerdos` 97,5 %, `pendientes` 79 %, `timeline` 77 %, `agenda` 70 %) y la cobertura exige que *todas* las herramientas usadas estén expuestas, con 1,77 usos por turno.
- Queda **sin medir** la calibración de umbrales por skill: solo pagaría en coste, y el coste no es un problema (histórico de `chat`: 0,1318 $, con el 53 % del prefijo ya cacheado), así que no se considera trabajo pendiente.

**6. El router falla abierto y su margen de timeout es del 70 %.** `ROUTER_TIMEOUT_MS=800` y, al expirar, el router expone todas las skills habilitadas. En producción no ha ocurrido nunca (44 llamadas, todas `success`, media 349 ms, **máximo 567 ms**), pero si la latencia sube esos turnos pagan el catálogo entero. Dato anotado para `llm-worker-resilience`, que se implementó y **archivó el 2026-10-18**, sin tocar este margen: el dato sigue vigente.

**7. «Marcar todas» (`notifications-inbox`): medido con un turno real (2026-10-18).** El arreglo estaba verificado por tests desde el 2026-10-14, pero nunca se había medido en un despliegue. Medición sobre la instancia levantada con la **v0.15.0** (el despliegue de trabajo, `docker-compose.yml`): tres avisos metidos por la API de ingesta y un turno real del agente pidiéndole «marca todas las notificaciones como leídas». Resultado: **una sola invocación de `mark_read` sin `ids`**, un solo `UPDATE notifications SET read_at = datetime('now') WHERE read_at IS NULL` con `rows_affected=3`, sin aprobaciones, sin repeticiones y con cero errores o reintentos en los logs (2 iteraciones del bucle ReAct de un tope de 5). El bucle de reintentos que motivó el arreglo **no aparece**. Referencia del «antes»: los 3 avisos con `read_at = NULL` de los turnos del 2026-10-13 19:12 y 2026-10-14 05:04. Dejó además una nota útil: el patrón sano es `list_unread` + `mark_read` sin `ids` **en la misma respuesta del modelo** (el agente usó el `total=3` del listado para responder), no `listar → marcar con los ids`.

**8. «Abrir» en Android: verificado en dispositivo real.** La prueba de la tarea 6.3 de `location-widget-open-action` (2026-10-18) salió bien: `geo:` **sin fijar paquete** abre la aplicación de mapas en un Android real y el pin cae donde debe. La decisión de diseño queda confirmada en la práctica y el `intent://` con `S.browser_fallback_url` que se descartó al diseñar **no hace falta**. No se reabre salvo que aparezca un dispositivo donde falle.

**9. MCP no se implanta. Descartado por decisión del usuario (2026-10-18).** Se estudió el ecosistema y se llegó a proponer un plan de tres changes (añadir, habilitar/deshabilitar y borrar servidores MCP, con importación por pegado), pero el hilo queda **cerrado**: no se implementa soporte de MCP y no se reabre sin orden explícita. Lo que conviene no volver a investigar si alguien lo replantea: la imagen de producción es `alpine:3.21` con solo `ca-certificates` —nada de Node ni Python—, así que los servidores MCP por **stdio no pueden correr ahí** y una v1 tendría que ser forzosamente **HTTP remoto**; los formatos de importación de facto son `mcpServers` (Claude Desktop y Cursor), `servers` (VS Code) y `mcp` (OpenCode); el router de skills **falla abierto** ante un timeout, así que expondría de golpe las herramientas de todos los servidores conectados; y el punto de equilibrio entre una plataforma MCP genérica y seguir con integraciones nativas estaba en torno a 7-8 integraciones, con un inventario hoy de 29 herramientas y 10 skills. La alternativa que se venía usando —integración nativa por herramienta— sigue siendo la vigente.

**10. El límite de llamadas a herramientas del bucle ReAct ya no es un contador único.** Decidido el 2026-10-25 (change `react-tool-guards`, archivado). El antiguo `MAX_TOOL_RETRIES = 5` por nombre de herramienta era **ciego a los argumentos y al resultado**: no distinguía el *progreso* (marcar 10 correos distintos) del *bucle* (la misma llamada reintentada), así que cortaba trabajo legítimo. Se sustituye por **tres guardas ortogonales** —huella de llamada idéntica, presupuesto por `tool::operation` (techo alto de éxitos / umbral bajo de fallos **consecutivos**) y tope global del turno— con sus umbrales en `settings` (`REACT_TOOL_FINGERPRINT_LIMIT`=2, `REACT_TOOL_FAILURE_LIMIT`=3, `REACT_TOOL_SUCCESS_LIMIT`=50, `REACT_TOOL_TOTAL_LIMIT`=100), editables en Ajustes y leídos **en cada turno**. Subir el número a secas se descartó: con la clave antigua un bucle real quemaba el presupuesto y todo el corte recaía en `max_iterations`. No se reabre salvo que aparezca un patrón real que las tres capas no cubran.

**11. El rango del calendario se consulta en hora local y con intervalo semiabierto `[start, end)`.** Decidido el 2026-10-25 al archivar el change `calendar-range-boundaries`. El frontend calcula los límites con `dayjs` en hora local del navegador y los envía como RFC 3339 con offset; el backend decide la inclusión comparando **instantes** (`DateTime<FixedOffset>`), nunca cadenas. El límite superior es el inicio del día siguiente al último visible. **No se migran los timestamps existentes con offset `+02:00`** (decisión del usuario): el arreglo funciona sobre datos mixtos sin migración. Los límites aceptan RFC 3339 o `YYYY-MM-DD` (día UTC completo); un límite no parseable en `GET /api/events` devuelve 422. Las escrituras (`create_event`/`update_event`, API y tool) normalizan a UTC canónico. Verificado extremo a extremo el 2026-10-26 contra la **BD real de producción** (copia de `co1`) y desplegado en `co1` como **v0.20.2**: el rango de octubre devuelve 16 eventos / 16 IDs únicos, incluidos los dos del 31-oct que antes desaparecían. No se reabre salvo que aparezca un fallo real de visibilidad.

**12. En ZenNotes, `Valet` es un *subpath*, no una carpeta; `folder` solo admite `inbox`, `quick`, `archive`.** Decidido el 2026-10-26 al cerrar el change `zennotes-notes`. El E2E contra el servidor real destapó que mandar `folder="Valet"` devuelve HTTP 500: las carpetas válidas del servidor son `inbox`, `quick`, `archive` y `trash`, y `Valet` es un subpath de `inbox` (el usuario ya lo tenía). La tool `zennotes` crea por defecto con `folder="inbox"` y `subpath` = ajuste `zennotes_folder` (por defecto `Valet`), de modo que la nota nace en `inbox/Valet/`; el servidor **autocrea** subpaths que no existan. `trash` reubica la nota a `trash/<path>`, así que `delete`/`restore` deben usar la ruta con el prefijo `trash/` (no son idempotentes sobre el path original). Verificado a través de la propia tool contra el servidor real: `list` recorta (6.273 → 5), `create` sin destino nace en `inbox/Valet/`, y el ciclo `trash`+`delete` deja la nota en `NotFound` sin residuo. No se reabre salvo que la API de ZenNotes cambie su modelo de carpetas.

**13. El enrutado de la skill `zennotes` está medido y su umbral no se toca.** Decidido el 2026-10-26 al cerrar el change `zennotes-notes`. Medición con el arnés `valet-route-eval` sobre turnos sintéticos (vault vs. store interna) y el modelo de producción `typesafe/jev-1.13`: cobertura del 100 %, y R3 (confusión `zennotes` ↔ `recuerdos`) **descartado** — nunca roba turnos a `recuerdos`. La sobre-activación en turnos internos («apúntame una idea») se corrigió **endureciendo las `criteria`** de la skill (caída de 0.78 → 0.07), no el umbral, que se mantiene en 0.10. Queda un turno ambiguo («¿Qué te conté ayer…?») en 0.10–0.11, sin impacto porque `recuerdos` siempre entra. No se sube el umbral.

**14. `zennotes_token` es un ajuste sensible.** Decidido el 2026-10-26 al cerrar el change `zennotes-notes`. `zennotes_token` figura en `SENSITIVE_KEYS`, así que `GET /api/settings` lo **omite** y `PUT /api/settings` lo **ignora**: la única puerta para guardarlo es el endpoint dedicado `PUT /api/zennotes/token` (`400` si va vacío, `204` si se guarda) y el estado se consulta con `GET /api/zennotes/status` (`{configured, base_url, folder}`, **nunca** el token). El campo del token en el frontend (`ZennotesSection.tsx`) arranca **siempre vacío** y no relee el valor guardado. `zennotes_base_url` (def `https://zennotes.territoriolinux.es`) y `zennotes_folder` (def `Valet`) no son sensibles.

**15. OpenTodo es el backend de tareas del agente, con doble backend y fallback.** Decidido el 2026-10-27 al cerrar el change `opentodo-integration` (fase 1, archivada; PR #265). Con `opentodo_token` configurado, la herramienta `tasks` va **entera** a OpenTodo (`https://todo.territoriolinux.es`); sin token conserva exactamente el comportamiento local sobre la tabla `tasks`, de modo que desplegar no cambia nada hasta que se pega el token. Las decisiones que no se reabren sin orden explícita:

- **Estados y ámbitos son etiquetas sin `@`**: `doing`/`waiting` (grupo `state`) y `shared`/`personal` (grupo `scope`). El modelo nunca maneja ids de etiqueta: `set_label` recibe `group` + `value` y el servicio compone el array completo, porque `PATCH labels` **reemplaza** el array entero.
- **Prioridad 1:1 y siempre explícita**: `critical→1`, `high→2`, `medium→3`, `low→4`. Sin prioridad se envía `4`, que es el valor por defecto del servidor: **no existe «sin prioridad»**.
- **Solo el espacio de trabajo por defecto**: no hay ajuste `opentodo_workspace` ni parámetro `workspace`, y **no se filtra al leer** (el servidor ignora `?workspace_id=`). Una tarea creada a mano en otro espacio sigue apareciendo en los listados.
- **No se crean vistas guardadas**: las etiquetas ya aparecen solas en el lateral de OpenTodo.
- **`opentodo_token` es sensible** (mismo molde que el 14): `GET /api/settings` lo omite y `PUT /api/settings` lo ignora; la única puerta es `PUT /api/opentodo/token`. `opentodo_base_url` **no** es sensible y viaja en `PUT /api/settings`.
- **`GET /api/opentodo/check` es la sonda real** (consulta `GET /api/v1/me` y devuelve siempre `200` con `{ok, detail}`, nunca el token); `GET /api/opentodo/status` solo lee ajustes y daba falsos positivos. Por eso «Probar conexión» usa `/check`, no `/status`.
- **Toda escritura lleva `Idempotency-Key`** con un UUID nuevo por petición; un `409` significa «ya aplicado», no se reintenta y la herramienta lo dice en el mensaje.
- **Las vistas se recalculan en Rust** (`inbox`, `today`, `upcoming`, `anytime`, `someday`, `doing`, `waiting`, `all`) porque el servidor no filtra. `Next` queda fuera del alcance.

Queda **pendiente** la verificación E2E contra la instancia real (tarea 9.3 de `tasks.md`), aplazada al despliegue por decisión del usuario (el token ya está rotado y se pegará en Ajustes → Integraciones → OpenTodo para verificarlo desde la UI). La **fase 2** (`remove-local-tasks`) retirará la tabla/FTS/API/tool/vista locales y migrará las 3 tareas, con acceso directo a `https://todo.territoriolinux.es` en su lugar.

---

## VI. GESTIÓN DE PLANES (`plans/`)

Todos los planes del proyecto viven en `plans/`. Ninguno se queda suelto en la raíz.

### Nomenclatura

- **Plan nuevo:** `plans/PLAN-XXX.md`, con `XXX` de tres dígitos y cero a la izquierda
  (`001`, `002`, …). El número se asigna en orden creciente y **no se reutiliza** mientras
  el plan siga existiendo.
- **Excepción — reordenación:** un número puede volver a usarse si el plan que lo tenía
  **nunca llegó a ejecutarse** y pasó a `-PENDING`/`-COMPLETED` dejando libre el ordinal.
  Precedente: el `003` se reutilizó al reorganizar `plans/` y aparcar el roadmap v2 como
  `PLAN-004-PENDING`.
- **Plan completado:** se renombra a `plans/PLAN-XXX-COMPLETED.md`. El cuerpo del plan
  **no se reescribe**: es un registro histórico de lo que se decidió e hizo.
- **Plan parado / backlog:** se renombra a `plans/PLAN-XXX-PENDING.md`. No está activo ni se
  trabaja en él ahora: queda aparcado, para retomarlo o cerrarlo más adelante.
- Un plan completado **no se reabre**. El trabajo nuevo —aunque sea un ajuste sobre algo ya
  cerrado— genera un `plans/PLAN-XXX.md` nuevo.

### Índice

| Fichero | Origen | Estado |
|:---|:---|:---:|
| `plans/PLAN-001-COMPLETED.md` | Plan maestro original (`PLAN.md`): fases F0–F4 | ✅ Completado |
| `plans/PLAN-002-COMPLETED.md` | OIDC (PocketID), producción tras Traefik y cierres (`PLAN-PENDIENTE.md`) | ✅ Completado |
| `plans/PLAN-003-COMPLETED.md` | Selección por skills y otros temas (Temas 1–4 ✅: PR #155, #159, #163, #166/#167; Tema 5 aparcado) | ✅ Completado |
| `plans/PLAN-004-PENDING.md` | Roadmap v2 (`v2-ROADMAP.md`): atareao.es, bitácora, project log | ⏸️ Pending |
| `plans/PLAN-005-COMPLETED.md` | Activity Timeline & Life Journal (línea temporal de hechos) (PR #169) | ✅ Completado |
| `plans/PLAN-006-COMPLETED.md` | `LocationWidget`: de «Guardar» a «Abrir» (change `location-widget-open-action`) | ✅ Completado |
| `plans/PLAN-007-COMPLETED.md` | Bitácora autónoma: la app escribe el diario por su cuenta (PRs #215 y #218) | ✅ Completado |
| `plans/PLAN-008-COMPLETED.md` | Sincronización del chat entre dispositivos (change `chat-cross-device-sync`) | ✅ Completado |
| `plans/PLAN-009-COMPLETED.md` | Integración de ZenNotes: el agente lee y escribe notas del vault vía API (change `zennotes-notes`) | ✅ Completado |
| `plans/PLAN-010-PENDING.md` | Diagnóstico del TTL de sesión (8 h fijo) y opciones A–D | ⏸️ Pending |
| `plans/PLAN-011.md` | Integrar OpenTodo y retirar el TODO local (change `opentodo-integration`, PR #265). Fase 1 ✅ archivada; fase 2 pendiente | 🔄 En curso |

> Los planes completados conservan notas que pueden haber quedado desactualizadas (p. ej. si
> un fichero estaba «sin trackear»). Se dejan tal cual, como registro histórico.
