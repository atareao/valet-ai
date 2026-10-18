## REMOVED Requirements

### Requirement: El widget `LocationWidget` SHALL mostrar una ubicación en un mapa con acciones

**Reason**: la acción «Guardar» (`save_place`) desaparece —no tenía ningún consumidor en el backend— y con ella el resto de acciones que devolvían un turno de usuario. Un bloque `MODIFIED` no puede eliminar escenarios sueltos, así que el requisito se retira y entra el sustituto. Lo sustituye «El widget `LocationWidget` SHALL mostrar una ubicación en un mapa con las acciones Abrir, Cómo llegar y Copiar coordenadas», que recoge el mapa y las tres acciones vigentes.

## ADDED Requirements

### Requirement: El widget `LocationWidget` SHALL mostrar una ubicación en un mapa con las acciones Abrir, Cómo llegar y Copiar coordenadas

El frontend SHALL registrar un widget `LocationWidget` que, a partir de `LocationData`
(`{ title?, description?, latitude?, longitude?, address? }`), pinte un mapa Leaflet centrado en las
coordenadas, con un marcador y su popup, y un panel con el título, la dirección y la descripción. El
mapa SHALL usar tiles de **OpenStreetMap** (sin API key) con su atribución visible, y SHALL aplicar un
filtro CSS sobre el panel de tiles para el aspecto oscuro. El mapa SHALL tener `scrollWheelZoom`
desactivado. El widget SHALL ofrecer las acciones «Abrir», «Cómo llegar» y «Copiar coordenadas», y
SHALL NOT ofrecer ninguna acción que devuelva un turno de usuario al backend. El componente SHALL
cargarse de forma **diferida** para no incluir la librería de mapas en el bundle inicial.

La acción «Abrir» SHALL llevar el punto a la aplicación de mapas del dispositivo y SHALL resolver su
destino según la plataforma del navegador: en Android SHALL usar el esquema `geo:` **sin fijar
paquete**, de modo que el sistema ofrezca el selector de aplicaciones o use la aplicación de mapas
predeterminada del usuario; en iOS SHALL usar la URL de Apple Maps; y en el resto de plataformas SHALL
usar la URL oficial de Google Maps. El destino SHALL derivarse siempre de las **coordenadas** y SHALL
NOT usar la dirección ni el título como consulta de búsqueda. Cuando el esquema lo permita, el título
—o, en su defecto, la dirección— SHALL viajar como **etiqueta codificada** del punto, nunca como
sustituto de las coordenadas. «Abrir» SHALL iniciar la navegación en el contexto actual y SHALL NOT
abrir una pestaña nueva.

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

#### Scenario: Abrir en Android usa el esquema geo: sin fijar paquete

**Given** un `LocationWidget` renderizado con coordenadas válidas
**When** se resuelve la acción «Abrir» en un navegador de Android
**Then** el destino empieza por `geo:`
**And** el destino contiene las coordenadas
**And** el destino NO fija ningún paquete de aplicación

#### Scenario: Abrir en iOS usa Apple Maps

**Given** un `LocationWidget` renderizado con coordenadas válidas
**When** se resuelve la acción «Abrir» en iOS
**Then** el destino apunta a `maps.apple.com`
**And** el destino contiene las coordenadas

#### Scenario: Abrir en las demás plataformas usa Google Maps

**Given** un `LocationWidget` renderizado con coordenadas válidas
**When** se resuelve la acción «Abrir» en un navegador de escritorio
**Then** el destino apunta a la URL oficial de Google Maps
**And** el destino contiene las coordenadas

#### Scenario: Abrir usa las coordenadas, no la dirección

**Given** un `LocationData` cuya `address` es el nombre de un lugar y cuyas coordenadas son válidas
**When** se resuelve la acción «Abrir»
**Then** el destino contiene las coordenadas
**And** la dirección NO se usa como consulta de búsqueda

#### Scenario: La etiqueta del punto viaja codificada

**Given** un `LocationData` con un `title` que contiene espacios, acentos y paréntesis
**When** se resuelve la acción «Abrir» en Android
**Then** la etiqueta aparece codificada en el destino
**And** los paréntesis de la etiqueta no alteran la estructura del destino

#### Scenario: Sin etiqueta no se añaden paréntesis

**Given** un `LocationData` sin `title` ni `address`
**When** se resuelve la acción «Abrir» en Android
**Then** el destino NO contiene paréntesis

#### Scenario: Abrir no abre una pestaña nueva

**Given** un `LocationWidget` renderizado con coordenadas válidas
**When** el usuario pulsa «Abrir»
**Then** la navegación se inicia en el contexto actual
**And** NO se abre una pestaña nueva

#### Scenario: El widget ya no ofrece «Guardar»

**Given** un `LocationWidget` renderizado con coordenadas válidas
**When** se inspeccionan sus acciones
**Then** sus acciones son «Abrir», «Cómo llegar» y «Copiar coordenadas»
**And** NO existe una acción «Guardar»
**And** NO se invoca `onAction` en ningún caso

#### Scenario: Coordenadas ausentes no rompen el widget

**Given** un `LocationData` sin `latitude` o `longitude`
**When** se renderiza `LocationWidget`
**Then** se muestra un aviso visible
**And** no se lanza ninguna excepción
