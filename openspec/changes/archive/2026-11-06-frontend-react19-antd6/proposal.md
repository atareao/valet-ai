# Proposal

## Why

Dependabot mantiene cinco PRs abiertos sobre `frontend/`, pero cuatro de ellos son una sola migración: `antd 6` y `react-leaflet 5` exigen React 19, así que no se pueden mergear por separado sin dejar el árbol en una combinación de peer dependencies que no construye. Además el repo ya declara `@ant-design/icons@6` junto a `antd@5`, combinación que la documentación de antd 6 declara no soportada. Es la primera migración mayor del stack de UI del proyecto y conviene hacerla con el árbol y la producción verificados, no por acumulación de PRs automáticos.

## What Changes

- **BREAKING** (dependencias): `antd` 5 → 6 y `@ant-design/icons` alineado a la serie `6` (`^6.0.0`), subidos en el mismo commit.
- **BREAKING** (dependencias): `react` y `react-dom` 18 → 19, junto con `@types/react` y `@types/react-dom`.
- **BREAKING** (dependencias): `react-leaflet` 4 → 5 (exige React 19) y `chart.js` 4.5.1 → 4.6.1.
- Barrido de las APIs que antd 6 marca deprecadas y el repo usa: `Space`/`Space.Compact` `direction` → `orientation`, `Tabs` `tabPosition` → `tabPlacement`, `Modal` `destroyOnClose` → `destroyOnHidden`, la retirada de `List` en favor de `Listy` y `maskClosable` → `mask.closable`.
- Adaptación del único uso del namespace global `JSX` al nuevo `React.JSX` y actualización de los tests que fijan atributos renombrados.
- `README.md` deja de anunciar «React 18 + Ant Design 5».
- Sin cambios de comportamiento, de diseño visual ni de API del backend. Sin cambios en `docker-compose.prod.yml`.
- `Descriptions` conserva `bordered`: en antd 6.7.0 no está deprecado y `variant` no existe en `DescriptionsProps`.

## Capabilities

### New Capabilities

Ninguna.

### Modified Capabilities

- `frontend`: se añaden tres requisitos sobre el stack de UI soportado, sobre el uso exclusivo de API vigente de antd y sobre la conservación de las invariantes de presentación y comportamiento durante la migración.

## Impact

- **Código:** `frontend/package.json`, `frontend/package-lock.json`, `frontend/src/theme.ts`, los componentes que usan las props renombradas, los tests que las fijan, `frontend/src/components/widgets/LocationWidget.tsx` y sus dos mocks, `README.md`.
- **Dependencias:** `react`, `react-dom`, `@types/react`, `@types/react-dom`, `antd`, `@ant-design/icons`, `react-leaflet`, `chart.js`.
- **Sistemas:** la suite de tests del frontend, el job `frontend` del CI, el build del `Dockerfile` y el control de bundle inicial de `frontend/scripts/check-initial-bundle.mjs`.
- **Producción:** dos despliegues en `co1` (uno por etapa) con verificación visual; el backend Rust no se toca.
- **PRs de Dependabot resueltos:** `#280`, `#281`, `#284`, `#311` quedan obsoletos al absorber la migración; `#310` (chart.js) entra en la etapa B.
