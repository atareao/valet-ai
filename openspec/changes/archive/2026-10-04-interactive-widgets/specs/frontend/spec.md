# Spec Delta: frontend

## ADDED Requirements

### Requirement: El frontend SHALL renderizar widgets a partir de un registry de componentes

El frontend SHALL exponer un `WIDGET_REGISTRY` que mapee el nombre del widget a su componente React, y
un `WidgetRenderer` que, dado un `WidgetInstance { id, name, data }`, renderice el componente
correspondiente pasándole `data` y un callback `onAction`. Con un `name` que no exista en el registry,
`WidgetRenderer` SHALL mostrar un aviso visible y SHALL NOT lanzar una excepción.

#### Scenario: Widget conocido

**Given** un `WidgetInstance` con `name: "Checklist"` y `data` válido
**When** se renderiza con `WidgetRenderer`
**Then** se pinta el componente `Checklist` dentro de un contenedor del mensaje

#### Scenario: Widget desconocido

**Given** un `WidgetInstance` con `name: "SystemMonitor"`
**When** se renderiza con `WidgetRenderer`
**Then** se muestra un aviso con el nombre desconocido
**And** el chat sigue funcionando

### Requirement: El evento SSE `widget` SHALL asociarse al mensaje del asistente en curso

`useSSE` SHALL reconocer los eventos con `type: "widget"` y entregarlos por un callback con `id`,
`name` y `data`. `useMainChat` SHALL acumular los `WidgetInstance` recibidos en el mapa de widgets del
mensaje del asistente que se está generando, y SHALL exponerlos a la vista para que se rendericen tras
el contenido Markdown. Un evento `widget` SHALL NOT interrumpir el resto del stream.

#### Scenario: Llega un widget durante el stream

**Given** un stream en curso del mensaje del asistente
**When** llega un evento `{"type":"widget","id":"w1","name":"QuickForm","data":{}}`
**Then** el mensaje del asistente en curso queda con un widget `QuickForm` de id `w1`

#### Scenario: El stream continúa tras el widget

**Given** un stream que ya emitió un evento `widget`
**When** llegan nuevos eventos `chunk` y `done`
**Then** el texto se sigue acumulando y el mensaje se cierra con normalidad

### Requirement: Las acciones de un widget SHALL devolverse al backend como un turno de usuario

El frontend SHALL exponer una función `sendWidgetAction(widget, action, payload)` que construya un
contenido mediante `formatWidgetAction(name, id, action, payload)` con el formato
`[widget:<name>#<id>] <action> <json-payload>` y lo envíe como mensaje de usuario por
`POST /api/chat/stream`, reutilizando el pipeline existente. No SHALL crearse un endpoint nuevo para
las acciones.

#### Scenario: Submit de QuickForm

**Given** un widget `QuickForm` con id `abc`
**When** el usuario envía el formulario con `{ "ciudad": "Madrid" }`
**Then** se envía un turno de usuario cuyo contenido es `[widget:QuickForm#abc] submit {"ciudad":"Madrid"}`

#### Scenario: Submit de Checklist

**Given** un widget `Checklist` con id `def`
**When** el usuario envía la selección con `["a","c"]`
**Then** se envía un turno de usuario cuya acción es `submit` y cuyo payload contiene `["a","c"]`

### Requirement: El widget `QuickForm` SHALL presentar campos y enviar los valores introducidos

`QuickFormWidget` SHALL renderizar, a partir de `QuickFormData`, un título y una lista de campos
(antd), y un botón de envío. Al enviar, SHALL llamar a `onAction("submit", valores)` con un objeto que
mapee el `name` de cada campo a su valor. Un campo con un `type` desconocido SHALL tratarse como texto.

#### Scenario: Envío de un campo de texto

**Given** un `QuickFormData` con un campo `ciudad` de tipo `text`
**When** el usuario escribe "Madrid" y envía el formulario
**Then** se invoca `onAction("submit", { "ciudad": "Madrid" })`

#### Scenario: Envío de varios tipos de campo

**Given** un `QuickFormData` con un campo de texto, uno de selección y otro de casilla
**When** el usuario completa los campos y envía
**Then** `onAction` recibe `submit` con el valor de cada campo bajo su `name`

### Requirement: El widget `Checklist` SHALL permitir marcar ítems y enviar la selección

`ChecklistWidget` SHALL renderizar, a partir de `ChecklistData`, un título y una casilla por ítem, y un
botón de envío. Al enviar, SHALL llamar a `onAction("submit", { checkedIds: [...] })` con los
identificadores marcados.

#### Scenario: Envío de elementos marcados

**Given** un `ChecklistData` con los ítems `a`, `b` y `c`
**When** el usuario marca `a` y `c` y envía
**Then** se invoca `onAction("submit", { "checkedIds": ["a", "c"] })`

#### Scenario: Sin elementos marcados

**Given** un `ChecklistData` con ítems y ninguno marcado
**When** el usuario envía
**Then** se invoca `onAction("submit", { "checkedIds": [] })`
