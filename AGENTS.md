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

### Aplazado por decisión del usuario — no ejecutar sin nueva orden

**1. `docker-compose.prod.yml` no se toca.**

Se mantiene tal cual (backend + frontend nginx independiente + PocketID). El despliegue de trabajo se hace con `docker-compose.yml` sobre Podman. Cambiarlo requiere orden explícita.

### Cerrado — no volver a abrir

**2. Autoarranque tras reinicio del host.** Descartado por decisión del usuario (2026-10-09): **no se habilita**. El contenedor no sobrevive a un reinicio del host y se levanta a mano si hace falta.

**3. Los ficheros sueltos de la raíz.**

`assets.svg` y `temporal.svg` llevaban sin trackear desde el 2026-09-29 y los vieron tres sesiones seguidas, que se limitaron a clasificarlos como "preexistentes y ajenos" sin decidir nada. El 2026-10-01 quedaron movidos a `~/.local/share/valet-scratch/`, con un `NOTAS.md` allí que explica qué son. Eran restos inválidos de la generación de iconos (uno de ellos ni siquiera es un SVG: es un fragmento pegado sin etiqueta `<svg>`), nunca estuvieron en git, y los iconos reales y versionados están en `frontend/src/assets/valet-icon.svg`, `assets/linux/hicolor/scalable/apps/valet.svg` y `assets/icono.svg`. Si vuelven a aparecer en la raíz, es que alguien los ha traído de vuelta a propósito.

**4. Los tokens OAuth de Strava no se exponen por la API de ajustes.** Decidido el 2026-10-10 al cerrar el Tema 3 del PLAN-003: `GET /api/settings` omite `strava_access_token` y `strava_refresh_token`, y `PUT /api/settings` los ignora. Son material de gestión interna, no ajustes editables. La lista vive en `SENSITIVE_KEYS` (`src/db/repos/settings.rs`).


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
| `plans/PLAN-003.md` | Selección por skills y otros temas (Temas 1–3 ✅: PR #155, #159, #163) | 🟢 Activo |
| `plans/PLAN-004-PENDING.md` | Roadmap v2 (`v2-ROADMAP.md`): atareao.es, bitácora, project log, observabilidad | ⏸️ Pending |

> Los planes completados conservan notas que pueden haber quedado desactualizadas (p. ej. si
> un fichero estaba «sin trackear»). Se dejan tal cual, como registro histórico.
