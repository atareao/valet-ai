# Spec Delta: frontend

## MODIFIED Requirements

### Requirement: El widget `QuickForm` SHALL presentar campos y enviar los valores introducidos

`QuickFormWidget` SHALL renderizar, a partir de `QuickFormData`, un título y una lista de campos
(antd), y un botón de envío. Al enviar, SHALL llamar a `onAction("submit", valores)` con un objeto que
mapee el `name` de cada campo a su valor. Los tipos de campo admitidos son `text`, `textarea`, `number`,
`select`, `checkbox` y `slider`; un campo con un `type` desconocido SHALL tratarse como texto.

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

## ADDED Requirements

### Requirement: Los widgets SHALL normalizar variantes de `data` antes de renderizar

`QuickFormWidget` SHALL usar `description` como título cuando falte `title`. `ChecklistWidget` SHALL
aceptar cada ítem con `label` o con `text`, y SHALL sintetizar un `id` cuando falte (a partir de su
posición). Ninguna de estas variantes SHALL provocar un error de render.

#### Scenario: `QuickForm` sin `title` usa `description`

**Given** un `QuickFormData` sin `title` pero con `description: "Elige una opción"`
**When** se renderiza el widget
**Then** se muestra "Elige una opción" como título

#### Scenario: `Checklist` con ítems sin `id` y con `text`

**Given** un `ChecklistData` con ítems `{ text: "Paso uno" }` y `{ text: "Paso dos" }` (sin `id`)
**When** el usuario marca "Paso uno" y envía
**Then** se invoca `onAction("submit", { "checkedIds": ["item-0"] })`
