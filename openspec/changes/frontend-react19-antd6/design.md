# Design

## Context

El frontend es React + TypeScript con Vite, 59 importaciones de `antd`, 42 ficheros `*.test.tsx` y 33 `*.test.ts`, `vitest` con `jsdom`, y el CI y el `Dockerfile` corriendo `tsc --noEmit`, `eslint --max-warnings 0`, `vitest run` y `npm run build` sobre Node 22. El tema oscuro vive entero en `theme.ts` con overrides de tokens por componente, y no tiene cobertura de tipos: antd no valida los nombres de los tokens.

## Goals / Non-Goals

**Goals:** dejar el frontend sobre React 19, antd 6, react-leaflet 5 y chart.js 4.6.1; eliminar las APIs deprecadas de antd; conservar el tema y el comportamiento; pasar la suite sin relajar aserciones.

**Non-Goals:** cambiar comportamiento o diseño; refactorizar `forwardRef`; tocar el backend Rust; tocar `docker-compose.prod.yml`; podar `AGENTS.md`.

## Decisions

### Decision: dos etapas, cada una con su propio PR y release

antd 6 declara soportar React **>= 18**, no 19. Eso permite subir antd primero y React después.

- **Etapa A:** `antd 5 → 6` + `@ant-design/icons` alineado, sobre React 18.
- **Etapa B:** `react`/`react-dom` y tipos `18 → 19`, `react-leaflet 4 → 5`, `chart.js 4.5.1 → 4.6.1`.

**Rationale:** antd 6 reescribe el DOM interno de componentes y activa variables CSS por defecto; React 19 cambia el modelo de tipos y retira APIs. Son dos causas raíz distintas. Con un único PR, un cambio de color en producción no permite saber cuál de las dos lo causó, y el rollback arrastra las cinco dependencias. Con dos etapas, cada fallo tiene un culpable único y el rollback es de un solo salto.

**Alternatives considered:** un único PR con las cinco dependencias (rechazado: mezcla causas raíz y hace ambiguo el rollback); actualizar solo antd 5 al último parche y quedarse ahí (rechazado: deja la cola de Dependabot viva y no resuelve la convivencia `icons@6` + `antd@5`).

### Decision: `@ant-design/icons` sube en el mismo commit que `antd`

La documentación de antd 6 declara que `@ant-design/icons@6` **no es compatible con `antd@5`** y que ambas deben subir juntas. El repo ya está hoy en esa combinación no soportada (`icons@^6.3.4` con `antd@^5.25.0`), así que el commit de la etapa A corrige el desajuste y no lo prolonga.

### Decision: enumerar las APIs deprecadas con `@ant-design/cli`, no a mano

`npx @ant-design/cli lint frontend/src` enumera las APIs deprecadas contra el antd instalado, y `antd token <Componente>` devuelve los tokens vigentes de la versión. Una lista escrita a mano se queda corta en cuanto antd renombra algo entre parches —el propio repo declara `^5.25.0` mientras el lock resuelve `5.29.3`—. La comparación de la salida de `antd token` antes y después de subir antd es lo que demuestra que ningún token de `theme.ts` quedó huérfano.

### Decision: `forwardRef` no se toca

React 19 permite pasar `ref` como prop, pero `forwardRef` sigue funcionando. Hay un único uso (`MessageInput.tsx`). Migrarlo mezcla un refactor sin relación con una migración de dependencias y ensucia el diff del PR que más fácil debe ser de revisar.

### Decision: los mocks de `matchMedia` se dejan fichero a fichero

`src/test/setup.ts` solo importa `@testing-library/jest-dom/vitest`; ~25 ficheros mockean `matchMedia` por su cuenta. Un mock central reduciría el ruido si antd 6 cambia sus requisitos de entorno, pero también ocultaría qué fichero fallaba. Se mantiene el reparto actual salvo necesidad demostrada, para que cada fallo siga apuntando a su sitio.

## Risks / Trade-offs

- **Token renombrado sin error de compilación:** el riesgo principal, silencioso por naturaleza. Mitigación: comparar `antd token` antes/después y revisar visualmente `Layout`, `Menu`, `Table` y `Card`.
- **`Tag` pierde el margen final en v6:** cambio aceptado por antd. Se revisan las listas de tags y, si el hueco importa, se reintroduce por contenedor antes que con un override global de `ConfigProvider`.
- **`matchMedia` duplicado en ~25 tests:** un cambio de entorno de antd 6 puede hacer fallar varios a la vez. Se acepta el coste de diagnóstico a cambio de localización.
- **Bundle inicial:** si antd 6 reorganiza subpaquetes, los regex de `vendor-antd` de `vite.config.ts` dejan de casar y el control de `postbuild` falla. Es un fallo del PR, no un umbral que subir.
- **Estado intermedio `icons@6` + `antd@5`:** existe hoy; la etapa A lo cierra. No se prolonga más de lo necesario.

## Migration Plan

1. Etapa A: rama desde `development`, `antd` + `@ant-design/icons`, barrido de deprecadas, tests ajustados, `just check-all` verde, auditoría, PR, release y despliegue en `co1`.
2. Etapa B: rama desde `development`, React 19 + tipos, `React.JSX.Element`, `react-leaflet 5`, `chart.js 4.6.1`, `just check-all` verde, auditoría, PR, release y despliegue en `co1`.
3. Cierre: archivar el change, renombrar el plan, actualizar `AGENTS.md` y cerrar los PRs de Dependabot absorbidos.

**Rollback:** cada etapa es un PR y un release independientes; revertir la etapa B deja antd 6 sobre React 18, que es una combinación soportada. Revertir la etapa A devuelve el árbol a la combinación actual.

## Open Questions

Ninguna pendiente de resolver antes de implementar. La única decisión abierta es de proceso, no de diseño: si el usuario prefiere un único PR y un único release para ambas etapas en lugar de dos, el diseño no cambia, solo se funden los pasos de release.
