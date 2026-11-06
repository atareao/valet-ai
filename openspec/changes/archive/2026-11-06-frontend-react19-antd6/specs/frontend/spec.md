# Delta: frontend

## ADDED Requirements

### Requirement: El frontend SHALL declarar React 19 y antd 6 como versiones de su stack de UI

`frontend/package.json` SHALL declarar `react` y `react-dom` en la serie `19`, `@types/react` y `@types/react-dom` en la serie `19`, `antd` en la serie `6`, `@ant-design/icons` en la serie `6` (`^6.0.0`), `react-leaflet` en la serie `5` y `chart.js` en la serie `4.6`. `frontend/package-lock.json` SHALL quedar sincronizado con esas declaraciones y SHALL NOT declararse `@ant-design/v5-patch-for-react-19`.

#### Scenario: Instalación limpia y reproducible

**Given** un clon limpio del repositorio
**When** se ejecuta `npm ci` en `frontend/`
**Then** la instalación SHALL terminar con código de salida 0
**And** SHALL NOT aparecer conflictos ni avisos de peer dependencies

#### Scenario: Las versiones declaradas son las acordadas

**Given** el `frontend/package.json`
**When** se leen las dependencias del stack de UI
**Then** `react`, `react-dom`, `@types/react` y `@types/react-dom` SHALL estar en la serie `19`
**And** `antd` SHALL estar en la serie `6`
**And** `@ant-design/icons` SHALL estar en la serie `6` (rango `^6.0.0`, sin saltos de major)
**And** `react-leaflet` SHALL estar en la serie `5`
**And** `chart.js` SHALL estar en la serie `4.6`
**And** SHALL NOT declararse `@ant-design/v5-patch-for-react-19`

### Requirement: El frontend SHALL usar solo la API vigente de antd 6

El código de `frontend/src` SHALL usar la API vigente de antd 6 en los componentes afectados: `orientation` en lugar de `direction` en `Space` y `Space.Compact`, `tabPlacement` en lugar de `tabPosition` en `Tabs`, `destroyOnHidden` en lugar de `destroyOnClose` en los componentes que lo aceptan, `mask.closable` en lugar de `maskClosable` en los superpuestos, y el enumerado de `size` unificado (`medium` en `Table` y `Divider`, `medium`/`large` en `Descriptions`). En `Descriptions` NO SHALL introducirse `variant`: en antd 6.7.0 `DescriptionsProps` no declara esa propiedad y `bordered` no está deprecado, así que se conserva `bordered` como API vigente. Ni el arranque de la aplicación ni la suite de tests SHALL emitir avisos de API deprecada de antd.

#### Scenario: El barrido no deja residuos

**Given** el árbol `frontend/src`
**When** se enumera su uso de antd con `npx @ant-design/cli lint frontend/src`
**Then** SHALL NOT reportarse ninguna API deprecada de antd
**And** SHALL NOT quedar ningún `direction` en `Space`, ningún `tabPosition` en `Tabs`, ningún `destroyOnClose` ni ningún `maskClosable`

#### Scenario: El arranque no avisa

**Given** la aplicación arrancada en local
**When** se recorren las vistas que montan `Modal`, `Drawer`, `Tabs`, `Descriptions`, `Space` y `Table`
**Then** la consola del navegador SHALL NOT mostrar avisos de API deprecada de antd

#### Scenario: La lista migrada conserva su semántica

**Given** las vistas `NotesView`, `JournalView` y `NotificationsView`, que ya no usan `List` de antd sino `Listy`
**When** se renderizan con al menos un elemento
**Then** cada fila SHALL exponerse con `role="listitem"`
**And** SHALL existir un contenedor ancestro con `role="list"` que las agrupe

#### Scenario: Las pestañas laterales siguen en vertical

**Given** el diálogo de ajustes, que monta `Tabs` con las pestañas en lateral
**When** se abre con ancho de escritorio
**Then** las pestañas SHALL seguir mostrándose en vertical a la izquierda
**And** el test que fija ese comportamiento SHALL comprobar la prop vigente de antd 6

### Requirement: La migración SHALL conservar las invariantes de presentación y comportamiento del frontend

La migración SHALL ser de dependencias: el tema declarado en `frontend/src/theme.ts`, la composición de la interfaz y el comportamiento observable SHALL permanecer invariantes. La suite de tests SHALL pasar sin haber eliminado ni relajado aserciones, y el reparto de code-splitting de `vite.config.ts` SHALL conservar los grupos `vendor` existentes.

#### Scenario: La suite pasa sin relajar aserciones

**Given** `frontend/` con las dependencias migradas
**When** se ejecuta `npx vitest run`
**Then** todos los ficheros de test SHALL pasar
**And** SHALL NOT haberse eliminado ni relajado ninguna aserción para conseguirlo

#### Scenario: El tema oscuro conserva sus tokens

**Given** la aplicación arrancada con `valetTheme`
**When** se revisan el `Layout`, el `Menu` lateral, las `Table` y las `Card`
**Then** los colores SHALL ser los declarados en `theme.ts`
**And** SHALL NOT quedar ningún token de `theme.ts` sin efecto por haber cambiado de nombre en antd 6

#### Scenario: El tipo, el lint y el build siguen en verde

**Given** `frontend/` migrado
**When** se ejecuta `just check-all`
**Then** `tsc --noEmit`, `eslint --max-warnings 0`, `vitest run` y `npm run build` SHALL completarse sin errores
**And** el control de bundle inicial SHALL pasar sin subir su umbral

#### Scenario: El code-splitting conserva sus grupos

**Given** la configuración de `vite.config.ts`
**When** se construye el bundle de producción
**Then** los grupos `vendor-react`, `vendor-antd`, `vendor-antd-icons`, `vendor-charts`, `vendor-leaflet` y `vendor-markdown` SHALL seguir generándose como chunks separados
