# Spec Delta: tools/registry

## MODIFIED Requirements

### Requirement: La definición de `render_widget` SHALL documentar el esquema de `data` de cada widget

La definición de la herramienta `render_widget` (`parameters()`) SHALL documentar, para cada widget
permitido, la forma esperada de `data`, de modo que el modelo emita las claves correctas. Para
`QuickForm` SHALL documentar `{ title, fields: [{ name, label, type, options?, min?, max? }],
submit_label? }` con los tipos de campo admitidos `text`, `textarea`, `number`, `select`, `checkbox` y
`slider`. Para `Checklist` SHALL documentar `{ title, items: [{ id, label }] }`. Para `LocationWidget`
SHALL documentar `{ title, description?, latitude, longitude, address? }` con `latitude` y `longitude`
numéricas.

#### Scenario: La definición documenta el esquema de QuickForm

**Given** la definición de la tool `render_widget`
**When** se inspecciona el esquema de su parámetro `data`
**Then** menciona `fields` con `name`, `label` y `type`
**And** enumera los tipos de campo `text`, `textarea`, `number`, `select`, `checkbox` y `slider`

#### Scenario: La definición documenta el esquema de Checklist

**Given** la definición de la tool `render_widget`
**When** se inspecciona el esquema de su parámetro `data`
**Then** menciona `items` con `id` y `label`

#### Scenario: La definición documenta el esquema de LocationWidget

**Given** la definición de la tool `render_widget`
**When** se inspecciona el esquema de su parámetro `data`
**Then** menciona `LocationWidget` con `latitude` y `longitude`
**And** el enum de `widget_name` incluye `LocationWidget`
