# Guarda de versiones del stack de UI del frontend

## Qué es

`frontend/src/test/dependency-stack.test.ts` es la guarda de contrato sobre
`frontend/package.json`: impide que alguien baje de serie el stack de UI sin que el CI lo
note. Nació en la etapa A del change `frontend-react19-antd6` (antd 6 sobre React 18) y hasta
el 2026-11-13 solo vigilaba esa mitad, aunque la etapa B llevaba cerrada desde el 2026-11-06
(v0.31.0) y la spec ya exigía el stack completo. Se podía volver a React 18, a
`react-leaflet` 4 o bajar `chart.js` con el CI en verde.

- Change: `dependency-stack-etapa-b` (archivado).
- Issue: [#368](https://github.com/atareao/valet-ai/issues/368). PR: [#371](https://github.com/atareao/valet-ai/pull/371).
- Plan: `plans/PLAN-030-ARCHIVED.md `.

## Ficheros implicados

- `frontend/src/test/dependency-stack.test.ts` — **el único fichero de código** que cambia.
- `frontend/package.json` y `frontend/package-lock.json` — **no se tocan**: solo se leen.
- `openspec/specs/frontend/spec.md` — declara el requirement; **no cambia**.

## Cómo funciona

El test lee `frontend/package.json` con `readFileSync` (vitest fija el cwd en `frontend/`) y
afirma cada versión declarada. No ejecuta `npm`, así que corre en milisegundos y no depende
del lockfile.

Ayudantes: `minVersion(range)` extrae `major.minor.patch` del rango declarado, `major(range)`
devuelve la serie y `compareVersions(a, b)` compara dos versiones semánticas.

### Lo que vigila

| Clave | Dónde | Aserción |
|---|---|---|
| `react`, `react-dom` | `dependencies` | serie **19** |
| `@types/react`, `@types/react-dom` | `devDependencies` | serie **19** |
| `antd` | `dependencies` | serie **6** |
| `@ant-design/icons` | `dependencies` | mínima **≥ 6.0.0** |
| `react-leaflet` | `dependencies` | serie **5** |
| `chart.js` | `dependencies` | major **4** y minor **≥ 6** |
| `@ant-design/v5-patch-for-react-19` | ambas | **no** debe declararse |

`chart.js` se afirma como serie (major 4 y minor ≥ 6) y no como mínimo suelto: la spec dice
«serie `4.6`». Un mínimo `4.6.1` dejaba pasar un salto a `^5.0.0` —lo detectó la auditoría
del PR— y la serie `4` a secas dejaba pasar `4.5.x`.

### Lo que NO vigila

La cadena de herramientas: `jsdom`, `vitest`, `vite` y `eslint`. Cambia a otro ritmo y su
contrato ya lo ejerce el CI al correr la suite. Mezclarlos haría que la guarda fallara por un
bump de herramienta en lugar de por una regresión del stack de UI.

## Variables/config

Ninguna. Este change no introduce ajustes, endpoints, migraciones ni variables de entorno.

## Cómo habilitar/desplegar

**No hay despliegue.** Es un test: corre en el job `frontend` del CI con el resto de la suite.
No requiere release ni recrear el stack en `co1`.

Para comprobar la guarda a mano:

```bash
cd frontend && npx vitest run src/test/dependency-stack.test.ts
```

## Decisiones

- **La guarda vigila el stack de UI, no la cadena de herramientas.** Ver arriba.
- **`chart.js` se afirma como serie, no como mínimo.** Cierra el hueco que detectó la
  auditoría del PR.
- **`skip_specs: true`:** el requirement ya existía en `openspec/specs/frontend/spec.md` y no
  cambia; lo que faltaba era su test. No se inventa un requirement para satisfacer la
  validación.
- **La guarda se verificó en rojo (8/8).** Un test que solo se ha visto en verde no prueba que
  detecte nada: se bajó temporalmente cada versión en `package.json` y se comprobó que falla
  exactamente el test que le corresponde, restaurando `package.json` byte a byte.

## Follow-ups

- **`@ant-design/icons` admite un salto de major.** Su aserción es `≥ 6.0.0`, así que un
  `^7.0.0` pasaría. Es de la etapa A y quedó fuera de este change, que añade aserciones de la
  etapa B y conserva las de la etapa A intactas. Candidato a endurecer si se aplica el mismo
  criterio a toda la tabla.
- **`minVersion` exige `major.minor.patch`.** Un rango como `>=19`, `19.x` o `workspace:*`
  lanzaría excepción en vez de afirmar. Falla en cerrado —no deja pasar una bajada— y el repo
  solo usa `^x.y.z`, así que hoy no hay riesgo real.
- **El gate local de bundle depende del Node del runtime.** No es de este change; está anotado
  en `docs/deps-queue-2026-11.md`.