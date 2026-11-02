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

---

## Dónde está el resto

Las reglas SDD/TDD generales (`I. CORE PRINCIPLES`, `II. EXECUTION WORKFLOW`, `III. Stack Commands`
y `IV. RESPONSE FORMAT`) viven en el `AGENTS.md` global (`~/.config/opencode/AGENTS.md`), que se
inyecta en cada step, y **no se repiten aquí**.

| Tema | Fichero | Qué contiene |
|:---|:---|:---|
| Decisiones cerradas e invariantes de diseño (`V. CURRENT PROJECT STATE`: items 1–16, «Aplazado» y «Cerrado — no volver a abrir») | `docs/decisiones-cerradas.md` | Texto literal de cada decisión, con fechas, PRs y changes archivados |
| Entorno de producción real (`co1`, Dockge, Traefik, despliegue de releases) | `docs/entorno-produccion.md` | Texto literal del item «Entorno de producción real (`co1`)» |
| Índice de planes (`plans/PLAN-*.md`) | `docs/plans-indice.md` | Tabla de planes con origen y estado |

### Cancelas cerradas (no reabrir)

Detalle en `docs/decisiones-cerradas.md`; ninguna se reabre sin orden explícita.

- **1** `docker-compose.prod.yml` no se toca.
- **2** Autoarranque tras reinicio del host: no se habilita.
- **5** Granularidad de skills y políticas de corte (top-k, bytes): no se tocan.
- **8** «Abrir» en Android: cerrado salvo dispositivo donde falle.
- **9** MCP: no se implanta; no se reabre.
- **10** Guardas del bucle ReAct: cerradas salvo patrón real no cubierto.
- **11** Rango `[start, end)` del calendario: cerrado salvo fallo de visibilidad.
- **12** ZenNotes (`folder` ∈ `inbox/quick/archive`): cerrado salvo cambio del modelo.
- **13** Umbral de la skill `zennotes`: no se toca.
- **15** OpenTodo como backend de tareas: no se reabre.
- **16** El TODO local ya no existe: no se reabre.
- **4. Los tokens OAuth de Strava no se exponen por la API de ajustes.** Decidido el 2026-10-10 al cerrar el Tema 3 del PLAN-003: `GET /api/settings` omite `strava_access_token` y `strava_refresh_token`, y `PUT /api/settings` los ignora. Son material de gestión interna, no ajustes editables. La lista vive en `SENSITIVE_KEYS` (`src/db/repos/settings.rs`).
- **14. `zennotes_token` es un ajuste sensible.** Decidido el 2026-10-26 al cerrar el change `zennotes-notes`. `zennotes_token` figura en `SENSITIVE_KEYS`, así que `GET /api/settings` lo **omite** y `PUT /api/settings` lo **ignora**: la única puerta para guardarlo es el endpoint dedicado `PUT /api/zennotes/token` (`400` si va vacío, `204` si se guarda) y el estado se consulta con `GET /api/zennotes/status` (`{configured, base_url, folder}`, **nunca** el token). El campo del token en el frontend (`ZennotesSection.tsx`) arranca **siempre vacío** y no relee el valor guardado. `zennotes_base_url` (def `https://zennotes.territoriolinux.es`) y `zennotes_folder` (def `Valet`) no son sensibles.
