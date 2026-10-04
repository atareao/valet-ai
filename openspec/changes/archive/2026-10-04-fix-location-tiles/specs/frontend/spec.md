# Spec Delta: frontend

## MODIFIED Requirements

### Requirement: El widget `LocationWidget` SHALL mostrar una ubicación en un mapa con acciones

El frontend SHALL registrar un widget `LocationWidget` que, a partir de `LocationData`
(`{ title?, description?, latitude?, longitude?, address? }`), pinte un mapa Leaflet centrado en las
coordenadas, con un marcador y su popup, y un panel con el título, la dirección y la descripción. El
mapa SHALL usar tiles de **OpenStreetMap** (sin API key) con su atribución visible, y SHALL aplicar un
filtro CSS sobre el panel de tiles para el aspecto oscuro. El mapa SHALL tener `scrollWheelZoom`
desactivado. El widget SHALL ofrecer las acciones «Guardar», «Cómo llegar» y «Copiar coordenadas». El
componente SHALL cargarse de forma **diferida** para no incluir la librería de mapas en el bundle
inicial.

#### Scenario: Renderiza la ubicación

**Given** un `LocationData` con `title`, `latitude` y `longitude` válidos
**When** se renderiza `LocationWidget`
**Then** se muestra el título y un mapa centrado en esas coordenadas con un marcador

#### Scenario: El mapa usa tiles de OpenStreetMap sin API key

**Given** la configuración de tiles del widget `LocationWidget`
**When** se inspecciona su URL de tiles
**Then** apunta a `openstreetmap.org`
**And** no apunta a un proveedor que requiera API key
**And** incluye la atribución de OpenStreetMap

#### Scenario: Guardar devuelve la acción al backend

**Given** un `LocationWidget` renderizado con coordenadas válidas
**When** el usuario pulsa «Guardar»
**Then** se invoca `onAction("save_place", …)` con el título, la dirección y las coordenadas

#### Scenario: Coordenadas ausentes no rompen el widget

**Given** un `LocationData` sin `latitude` o `longitude`
**When** se renderiza `LocationWidget`
**Then** se muestra un aviso visible
**And** no se lanza ninguna excepción
