# Spec Delta: frontend

## ADDED Requirements

### Requirement: El panel Herramientas SHALL limitar la altura de la lista y permitir scroll

`ToolsTab` SHALL renderizar la lista de tools dentro de un contenedor con una altura máxima relativa al
viewport (`max-height: 60vh`) y `overflow-y: auto`, de forma que cuando haya más tools de las que caben
en la ventana la lista se desplace verticalmente y todas las filas sigan siendo alcanzables, sin
desbordar el `Modal` de `SettingsDialog`.

**Given** el SettingsDialog abierto en la tab "Herramientas"
**When** el panel se monta con la lista de tools
**Then** la lista queda envuelta en un contenedor con `overflow-y: auto` y una `max-height` definida

#### Scenario: La lista queda contenida y con scroll

**Given** que `GET /api/tools` devuelve más tools de las que caben en el viewport
**When** el usuario abre la pestaña "Herramientas"
**Then** la lista se renderiza dentro de un contenedor con `overflow-y` en `auto` y `max-height: 60vh`

#### Scenario: Todas las filas quedan dentro del contenedor desplazable

**Given** la pestaña "Herramientas" con varias tools
**When** se renderiza el panel
**Then** todas las filas de tools están contenidas en el mismo contenedor con scroll
