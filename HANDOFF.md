# HANDOFF

**Fecha:** 2026-11-13

## Estado

**COMPLETADO.** Los tres pasos del relevo anterior (0 → 1 → 2) están cerrados. `valet-ai` sin
issues ni PRs abiertos, `openspec list` sin changes activos.

## Qué se ha hecho

### Paso 0 — el relevo entra en `development`

- PR **#366** (`Actualiza HANDOFF tras v0.37.0`) mergeado con merge commit (`6cdc1fa`), rama
  `docs/handoff-v0.37.0` borrada.

### Paso 1 — la cola de Dependabot entra, salvo `vite` 8.3.7

- Issue **#367**, change `deps-queue-2026-11`, `plans/PLAN-029-ARCHIVED.md`.
- **Código** (PR **#369** → `development`, CI verde): cinco bumps en una sola rama.
  `uuid` 1.29.0 → **1.29.2** (solo `Cargo.lock`), `react-router-dom` 7.18.4 → **7.18.5`,
  `vitest` 5.0.5 → **5.1.0**, `eslint` 10.11.0 → **10.14.0** y `jsdom` 30.1.3 → **31.0.0** (major).
- **`vite` 8.3.7 NO entra.** Rebasado sobre `development` y con el CI relanzado (run
  `43368520339`) deja el grafo inicial en **70.01 kB** sobre un umbral de **70.00 kB**; el margen
  real es de 0.02 kB porque 65.95 de esos 70.00 kB son `react` + `react-dom`. El umbral **no se
  sube** (lo prohíbe `openspec/specs/frontend/spec.md`). PR **#353** cerrado con ese motivo.
- **`jsdom` 31 no rompió nada:** 738 tests verdes **sin adaptar ninguno** (a diferencia del
  precedente `adopt-jsdom-30-1-3`). Su peer `@napi-rs/canvas` es opcional y no se instala; su
  motor CSS nuevo se lleva ~300 líneas del lockfile.
- Los cuatro bumps del frontend aportan **0 bytes** al grafo inicial (69.98 kB, byte-idéntico).
- **Cierre documental** (PR **#370**): change archivado, `docs/deps-queue-2026-11.md`, apartado
  **26** de `AGENTS.md`, fila del índice, issue **#367** cerrado. Cerrados también #352, #354,
  #355, #356 y #357 como absorbidos.
- Auditoría `@react-reviewer` sin bloqueantes.

### Paso 2 — la guarda de versiones cubre la etapa B

- Issue **#368**, change `dependency-stack-etapa-b`, `plans/PLAN-030-ARCHIVED.md`.
- **Reencuadrado al escribir el delta:** la spec **ya declaraba** el stack completo de la etapa B
  (`openspec/specs/frontend/spec.md`, «El frontend SHALL declarar React 19 y antd 6…»), así que no
  faltaba spec: **faltaba el test que la spec ya exigía**.
- **Código** (PR **#371** → `development`, CI verde): `frontend/src/test/dependency-stack.test.ts`
  pasa de 3 a 9 aserciones. Nuevas: `react`/`react-dom` (serie 19),
  `@types/react`/`@types/react-dom` (serie 19), `react-leaflet` (serie 5) y `chart.js`
  (**major 4 y minor ≥ 6**). Las tres de la etapa A quedan intactas. `describe` → «stack de
  dependencias de UI del frontend».
- **`chart.js` se afirma como serie, no como mínimo:** la auditoría del PR detectó que un mínimo
  `4.6.1` dejaba pasar un salto a `^5.0.0`. Se corrigió antes de mergear.
- **Verificado en rojo (8/8):** bajar cada versión en `package.json` hace fallar exactamente el test
  que le corresponde. `package.json` y el lock no se tocan (solo se leen).
- **Cierre documental** (PR **#372**): change archivado, `docs/dependency-stack-guard.md`, apartado
  **27** de `AGENTS.md`, fila del índice, issue **#368** cerrado.

## Tarea pendiente / siguiente paso

- **Ninguna tarea abierta.** `valet-ai` sin issues ni PRs, `openspec list` sin changes activos.
- Candidatas (sin asignar, por orden de tamaño):
  1. **`@ant-design/icons` admite un salto de major** en la guarda de versiones (su aserción es
     `≥ 6.0.0`, así que un `^7.0.0` pasaría). Es de la etapa A y quedó fuera del paso 2. Mismo
     criterio que `chart.js` si se quiere endurecer toda la tabla.
  2. **atareao.es #175**: aprovisionar `atareao_valet_reply_user` desde la UI del plugin (hoy paso
     manual de WP-CLI).
  3. **«Token de chat» para el móvil** (solo si se pide): hoy solo `POST /api/notifications` admite
     clave; leer la bandeja o mandar una frase al agente exige cookie OIDC de 8 h. Es superficie
     nueva de autenticación: necesita change con spec, no un parche.
- **Aparcado por decisión del usuario, no tocar sin orden:** `PLAN-004-PENDING` (roadmap v2) y
  `PLAN-010-PENDING` (TTL de sesión de 8 h).

## Follow-ups conocidos

- **Política del umbral de bundle (abierto).** El margen es de 0.02 kB. Hay que decidir si se sube
  el umbral de forma deliberada —una vez, con criterio— o si se asume que los patches de `vite`
  quedan congelados. Dependabot **ya ha vuelto a proponer `vite` 8.3.7** (rama
  `dependabot/npm_and_yarn/frontend/development/vite-8.3.7`), como estaba previsto: volverá a estar
  rojo.
- **El gate local de bundle depende del Node del runtime.** Con **Node 24** (el `node` por defecto
  en esta máquina) el mismo árbol mide **70.02 kB** y `just check-all` sale rojo en `frontend-check`;
  con **Node 22** (el que fija `.github/workflows/ci.yml`) mide **69.98 kB** y pasa. Es el `zlib`
  del runtime, no el código. Para verificar en local: `fnm exec --using=22.23.4 -- just check-all`.
- **La licencia MIT está en `main` pero no en `development`.** `a02135a` (📝 docs: añado licencia
  MIT) entró directo en `main`; `development` no lo tiene. Si se publica un release desde
  `development`, el `LICENSE` no viajará. Conviene decidir si se sincroniza.
- **Archive deja el `Purpose` en placeholder** para capabilities nuevas: reescribirlo a mano y
  volver a pasar `openspec validate --all --strict`.
- **Pedir el dominio público desde `co1` da `403 Ko`** (bloqueo de IP origen/WAF); desde fuera
  responde 200. Para E2E desde `co1`, usar la IP del contenedor o un contenedor en su red.
- **Duplicados en el catálogo de OxiMeal** (Merluza/Crema/Lentejas ×2, sin instrucciones):
  limpieza aparte, fuera de alcance.
- **La ruta de reflexión graba `kind='chat'` con `model="default"`** y ensucia las estadísticas de
  chat con llamadas que no son conversación del usuario (anotado desde `tool-usage-stats`).

## Restricciones

- Flujo SDD + TDD: aprobación de la spec antes de escribir código.
- Gitflow: PR a `development`/`main`, commits con gitmoji. No commits directos a `main`/`development`.
- No relajar aserciones de tests.
- `docker-compose.prod.yml` no se toca.