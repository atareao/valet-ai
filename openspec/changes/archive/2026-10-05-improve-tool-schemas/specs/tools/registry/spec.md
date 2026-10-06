# Spec Delta

## ADDED Requirements

### Requirement: Las definiciones de herramientas SHALL estar en español y documentar los obligatorios por operación

Todas las tools integradas SHALL exponer `description()` y las descripciones de sus parámetros en español. Las tools que despachan por un parámetro `operation` SHALL documentar en la descripción de `operation` qué campos son obligatorios para cada acción, de modo que el modelo no invoque una operación sin sus argumentos. `render_widget` SHALL mantener `data` como opcional.

#### Scenario: Las tools en inglés se traducen al español

- **Given** las definiciones de `web_search` y `search_places`
- **When** se inspeccionan `description()` y las descripciones de sus parámetros
- **Then** todas están redactadas en español

#### Scenario: La descripción de operation enumera los obligatorios

- **Given** las definiciones de `notes`, `tasks` y `reminders`
- **When** se inspecciona la descripción del parámetro `operation`
- **Then** menciona qué campos exige cada acción (por ejemplo `id` para borrar o actualizar y `content` al crear)

#### Scenario: Las tools de geolocalización y hora se describen en español

- **Given** las definiciones de `geocode`, `reverse_geocode`, `get_current_location`, `get_current_time` y `unified_search`
- **When** se inspeccionan sus descripciones
- **Then** están redactadas en español y describen qué hacen
- **And** la de `unified_search` aclara que la consulta es en lenguaje natural o palabras clave

#### Scenario: La descripción de weather guía a geocode

- **Given** la definición de la tool `weather`
- **When** se inspecciona su descripción
- **Then** indica que, si solo se dispone del nombre de la ciudad, debe resolverse antes con `geocode`
- **And** `required` sigue siendo `["latitude", "longitude"]`

## MODIFIED Requirements

### Requirement: La definición de `render_widget` SHALL documentar el esquema de `data` de cada widget

La definición de la herramienta `render_widget` (`parameters()`) SHALL documentar, para cada widget permitido, la forma esperada de `data`, y SHALL refinar el esquema de `data` con una unión discriminada por `widget_name` (`oneOf` o `anyOf`), de modo que el modelo emita las claves correctas. `data` SHALL permanecer opcional y NO SHALL figurar en `required`. Para `QuickForm` SHALL documentar `{ title, fields: [{ name, label, type, options?, min?, max? }], submit_label? }` con los tipos de campo admitidos `text`, `textarea`, `number`, `select`, `checkbox` y `slider`. Para `Checklist` SHALL documentar `{ title, items: [{ id, label }] }`. Para `LocationWidget` SHALL documentar `{ title, description?, latitude, longitude, address? }` con `latitude` y `longitude` numéricas.

#### Scenario: La definición documenta el esquema de QuickForm

- **Given** la definición de la tool `render_widget`
- **When** se inspecciona el esquema de su parámetro `data`
- **Then** menciona `fields` con `name`, `label` y `type`
- **And** enumera los tipos de campo `text`, `textarea`, `number`, `select`, `checkbox` y `slider`

#### Scenario: La definición documenta el esquema de Checklist

- **Given** la definición de la tool `render_widget`
- **When** se inspecciona el esquema de su parámetro `data`
- **Then** menciona `items` con `id` y `label`

#### Scenario: La definición documenta el esquema de LocationWidget

- **Given** la definición de la tool `render_widget`
- **When** se inspecciona el esquema de su parámetro `data`
- **Then** menciona `LocationWidget` con `latitude` y `longitude`
- **And** el enum de `widget_name` incluye `LocationWidget`

#### Scenario: data sigue siendo opcional

- **Given** la definición de la tool `render_widget`
- **When** se inspecciona su `required`
- **Then** contiene `["widget_name"]`
- **And** NO contiene `data`

#### Scenario: El esquema de data usa una unión discriminada

- **Given** la definición de la tool `render_widget`
- **When** se inspecciona la propiedad `data`
- **Then** su esquema incluye `oneOf` o `anyOf` con una alternativa por widget
- **And** cada alternativa corresponde a uno de los valores del enum de `widget_name`
