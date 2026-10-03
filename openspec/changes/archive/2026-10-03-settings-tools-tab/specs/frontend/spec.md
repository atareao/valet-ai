# Spec Delta: frontend

## ADDED Requirements

### Requirement: SettingsDialog SHALL display Herramientas tab

SettingsDialog SHALL display a "Herramientas" top-level tab that lists the registered tools and lets
the user enable or disable each one. Al abrirse, el panel SHALL cargar la lista desde `GET /api/tools`
y SHALL mostrar cada tool con su nombre, su descripción y un `Switch` que refleja su campo `enabled`.
Al cambiar un `Switch`, SHALL llamar a `PUT /api/tools/{id}/toggle` y actualizar la fila con la tool
devuelta; si la llamada falla, SHALL restaurar el valor previo y mostrar un aviso de error.

**Given** el SettingsDialog abierto en la tab "Herramientas"
**When** el panel se monta
**Then** se llama a `GET /api/tools`
**And** se listan todas las tools con nombre, descripción y un `Switch` con su estado `enabled`

#### Scenario: La pestaña lista las tools con su estado
**Given** `GET /api/tools` devuelve una tool `weather` con `enabled: true`
**When** el usuario abre la pestaña "Herramientas"
**Then** la fila de `weather` muestra su nombre, su descripción y un `Switch` activado

#### Scenario: Deshabilitar una tool
**Given** la fila de la tool `weather` con el `Switch` activado
**When** el usuario apaga el `Switch`
**Then** se llama a `PUT /api/tools/weather/toggle` (con el `id` de la tool)
**And** el `Switch` queda apagado

#### Scenario: Habilitar de nuevo una tool
**Given** la fila de la tool `weather` con el `Switch` apagado
**When** el usuario enciende el `Switch`
**Then** se llama a `PUT /api/tools/weather/toggle`
**And** el `Switch` queda activado

#### Scenario: Error al cambiar el estado
**Given** la fila de la tool `weather` con el `Switch` activado
**When** el usuario apaga el `Switch` y la llamada a `PUT /api/tools/.../toggle` falla
**Then** el `Switch` vuelve a quedar activado
**And** se muestra un aviso de error

#### Scenario: Estado de carga
**Given** que `GET /api/tools` aún no ha respondido
**When** el usuario abre la pestaña "Herramientas"
**Then** se muestra un indicador de carga
**And** la lista de tools no se muestra hasta que llegan los datos

## MODIFIED Requirements

### Requirement: SettingsDialog SHALL fit all its top-level tabs without overflow

El `Modal` de SettingsDialog SHALL declarar un ancho de al menos **960 px** (se fija en 1000) para que sus siete pestañas superiores —Perfil, Interfaz, Prompts, API Keys, Memoria, Generación y Herramientas— quepan en una sola fila, sin que ninguna se oculte en el desplegable de desbordamiento de antd.

**Given** el SettingsDialog abierto  
**When** se renderiza  
**Then** el ancho del diálogo es de al menos 960 px  
**And** sus siete pestañas superiores son alcanzables

#### Scenario: El diálogo es más ancho que el mínimo
**Given** el SettingsDialog abierto  
**When** se renderiza  
**Then** el ancho declarado del `Modal` es mayor o igual a 960 px

#### Scenario: La pestaña Memoria persistente es alcanzable
**Given** el SettingsDialog abierto  
**When** el usuario abre la pestaña "Memoria" y selecciona la sub-pestaña "Persistente"  
**Then** se muestra el panel de memoria persistente

#### Scenario: Existen exactamente seis pestañas superiores
**Given** el SettingsDialog abierto  
**When** el usuario busca las pestañas superiores  
**Then** existen exactamente siete: Perfil, Interfaz, Prompts, API Keys, Memoria, Generación y Herramientas  
**And** NO existe una pestaña superior "Memoria persistente"
