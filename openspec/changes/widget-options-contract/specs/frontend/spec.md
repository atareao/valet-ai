## MODIFIED Requirements

### Requirement: El widget `QuickForm` SHALL presentar campos y enviar los valores introducidos

`QuickFormWidget` SHALL renderizar, a partir de `QuickFormData`, un título y una lista de campos
(antd), y un botón de envío. Al enviar, SHALL llamar a `onAction("submit", valores)` con un objeto que
mapee el `name` de cada campo a su valor. Los tipos de campo admitidos son `text`, `textarea`, `number`,
`select`, `checkbox`, `slider` y `radio`; un campo con un `type` desconocido SHALL tratarse como texto.

#### Scenario: Envío de un campo de texto

**Given** un `QuickFormData` con un campo `ciudad` de tipo `text`
**When** el usuario escribe "Madrid" y envía el formulario
**Then** se invoca `onAction("submit", { "ciudad": "Madrid" })`

#### Scenario: Envío de varios tipos de campo

**Given** un `QuickFormData` con un campo de texto, uno de selección y otro de casilla
**When** el usuario completa los campos y envía
**Then** `onAction` recibe `submit` con el valor de cada campo bajo su `name`

#### Scenario: Envío de un campo numérico

**Given** un `QuickFormData` con un campo `presupuesto` de tipo `number`
**When** el usuario introduce "1500" y envía el formulario
**Then** se invoca `onAction("submit", { "presupuesto": 1500 })`

#### Scenario: Envío de un campo de texto largo

**Given** un `QuickFormData` con un campo `notas` de tipo `textarea`
**When** el usuario escribe un texto y envía el formulario
**Then** se invoca `onAction("submit", { "notas": "<texto>" })`

#### Scenario: Un campo de tipo `radio` muestra las alternativas a la vista
**Given** un `QuickFormData` con un campo `choice` de tipo `radio` y tres alternativas con su texto completo
**When** el widget se renderiza
**Then** las tres alternativas SHALL verse en la pantalla, sin desplegar ningún control
**And** la etiqueta del campo SHALL mostrarse como la pregunta

#### Scenario: Envío de un campo de tipo `radio`
**Given** un `QuickFormData` con un campo `choice` de tipo `radio`
**When** el usuario elige la segunda alternativa y envía el formulario
**Then** se invoca `onAction("submit", { "choice": "<texto de la segunda alternativa>" })`
