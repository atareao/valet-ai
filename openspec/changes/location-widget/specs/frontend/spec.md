# Spec Delta: frontend

## ADDED Requirements

### Requirement: El widget `LocationWidget` SHALL mostrar una ubicación en un mapa con acciones

El frontend SHALL registrar un widget `LocationWidget` que, a partir de `LocationData`
(`{ title?, description?, latitude?, longitude?, address? }`), pinte un mapa Leaflet con **tiles
oscuros** y atribución visible, centrado en las coordenadas, con un marcador y su popup, y un panel con
el título, la dirección y la descripción. El mapa SHALL tener `scrollWheelZoom` desactivado. El widget
SHALL ofrecer las acciones «Guardar», «Cómo llegar» y «Copiar coordenadas». El componente SHALL
cargarse de forma **diferida** para no incluir la librería de mapas en el bundle inicial.

#### Scenario: Renderiza la ubicación

**Given** un `LocationData` con `title`, `latitude` y `longitude` válidos
**When** se renderiza `LocationWidget`
**Then** se muestra el título y un mapa centrado en esas coordenadas con un marcador

#### Scenario: Guardar devuelve la acción al backend

**Given** un `LocationWidget` renderizado con coordenadas válidas
**When** el usuario pulsa «Guardar»
**Then** se invoca `onAction("save_place", …)` con el título, la dirección y las coordenadas

#### Scenario: Coordenadas ausentes no rompen el widget

**Given** un `LocationData` sin `latitude` o `longitude`
**When** se renderiza `LocationWidget`
**Then** se muestra un aviso visible
**And** no se lanza ninguna excepción
