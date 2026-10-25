# Design: Adoptar jsdom 30.1.3 y alinear los tests con su comportamiento correcto

## Context

`jsdom` es el entorno DOM de la suite de vitest del frontend. Dependabot propone 30.1.1 → 30.1.3 (PR #248). Verificado en local (`npm ci` por versión y ejecución de los dos ficheros, 3 veces cada uno):

| Versión de jsdom | `SettingsDialog.test.tsx` | `CalendarView.test.tsx` |
|---|---|---|
| 30.1.1 | pasa | pasa |
| 30.1.3 | **falla** — `user.clear`: «The element to be cleared could not be focused» | **falla** — `toHaveStyle({ background })` |

El changelog de jsdom identifica la causa: 30.1.2 dejó de enfocar elementos ocultos por `display` (el test tecleaba en el panel oculto «Archivist Prompt»), y 30.1.3 dejó de devolver shorthands de `getComputedStyle()` de forma inconsistente (el test asertaba `background` en lugar de `background-color`). Son dos correcciones de jsdom, no regresiones.

## Goals / Non-Goals

**Goals**

- Adoptar jsdom 30.1.3.
- Que los dos tests ejerciten de verdad la interfaz visible, sin apoyarse en bugs del entorno.

**Non-Goals**

- Cambiar el comportamiento de la aplicación.
- Tocar componentes de producción.
- Modificar cualquier otra dependencia.
- `docker-compose.prod.yml` ni producción.

## Decisions

### D1 — Se adopta jsdom 30.1.3

`jsdom` SHALL pasar a `^30.1.3` en `frontend/package.json` y quedar reflejado en `frontend/package-lock.json`. Las dos correcciones que nos afectan son intencionadas; quedarse en 30.1.1 equivaldría a mantener dos tests que dependen de bugs del entorno.

### D2 — El test de `SettingsDialog` activa la sub-pestaña antes de escribir

`SettingsDialog.test.tsx` SHALL activar la sub-pestaña correspondiente antes de hacer `clear`/`type` sobre cada prompt, de modo que el campo esté visible y sea enfocable. La pestaña se localiza por su etiqueta (p. ej. `screen.getByText("Archivist")`), siguiendo la convención del propio fichero de test. El test SHALL re-consultar el campo con `getByLabelText` **después** de activar la pestaña, y SHALL NOT operar sobre paneles ocultos por `display: none`.

### D3 — El test de `CalendarView` aserta el longhand `background-color`

`CalendarView.test.tsx` SHALL asertar la propiedad `background-color` (longhand) con el color de la categoría, en lugar del shorthand `background`. jsdom 30.1.3 resuelve los shorthands de forma consistente y el longhand es la forma estable y precisa de comprobarlo.

### D4 — Sin delta de spec (`skip_specs: true`)

El cambio no altera comportamiento observable (es un bump de dependencia de desarrollo con adaptación de tests), así que no se modifica ninguna spec. El change se marca `skip_specs: true` en `.openspec.yaml`. No se inventa un requirement solo para validar.

## Risks / Trade-offs

- `jsdom` ≥ 30.0.0 exige Node `^22.22.2 || ^24.15.0 || >=26`. El CI usa `node-version: 22` (última 22.x), que lo cumple: en el run de #248 el `npm ci` y el arranque de vitest funcionaron y solo fallaron los dos tests descritos.
- Al hacerse los tests más estrictos, podrían destapar otros usos débiles del entorno. Se corrió la suite completa (567 tests) con jsdom 30.1.3 y **solo** estos dos fallaban.
- Localizar la sub-pestaña con precisión es necesario para no repetir el patrón de operar sobre elementos ocultos.
