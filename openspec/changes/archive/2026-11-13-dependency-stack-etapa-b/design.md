# Design

## Decisión 1 — la guarda vigila el stack de UI, no la cadena de herramientas

El fichero pasa a afirmar exactamente lo que enumera el requirement de `openspec/specs/frontend/spec.md`: `react`, `react-dom`, `@types/react`, `@types/react-dom`, `antd`, `@ant-design/icons`, `react-leaflet` y `chart.js`.

Deliberadamente **no** afirma `jsdom`, `vitest`, `vite` ni `eslint`: eso es la cadena de herramientas, cambia a otro ritmo y su contrato ya lo ejerce el CI al correr la suite. Mezclarlos haría que la guarda fallara por un bump de herramienta en lugar de por una regresión del stack de UI, que es lo que debe proteger.

## Decisión 2 — la aserción es de serie o de mínimo, según lo que la spec declara

- `react`, `react-dom`, `@types/react`, `@types/react-dom` → **serie** (major `19`), como ya se hace con `antd` en la etapa A.
- `react-leaflet` → **serie** `5`. Su peer exige React 19, así que bajar a la 4 es precisamente lo que hay que impedir.
- `chart.js` → **major `4` y minor ≥ `6`**. La spec dice «serie `4.6`» y el valor declarado es `^4.6.1`. Afirmar solo el mínimo `4.6.1` dejaba pasar un salto a `^5.0.0` (lo detectó la auditoría) y afirmar la serie `4` a secas dejaba pasar `4.5.x`; la conjunción cubre los dos casos reales.

## Decisión 3 — no se cambia la spec

El requirement ya declara las versiones y **no cambia**; lo que cambia es la cobertura de test. Marcar `skip_specs: true` es la lectura disciplinada de la regla de OpenSpec («specs describe behavior, so if behavior does not change, no spec should change either») y evita duplicar en la spec lo que el test ya dice. Si algún día se quiere que la spec nombre al guardián, es un change aparte.

## Decisión 4 — la guarda se verifica en rojo

Un test que solo se ha visto en verde no prueba que detecte nada. La verificación incluye bajar temporalmente `react` a `^18.3.1`, `react-leaflet` a `^4.2.1` y `chart.js` a `^4.5.1` en `frontend/package.json` y comprobar que el test **falla** en cada caso, revirtiendo cada manipulación. Es la única forma de saber que la guarda no es decorativa.