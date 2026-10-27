## MODIFIED Requirements

### Requirement: SettingsDialog SHALL display API Keys tab

SettingsDialog SHALL display an "API Keys" tab with password fields for API keys, the apimail connection settings, the Miniflux connection settings and the ZenNotes token.

**Given** el SettingsDialog está abierto en la tab "API Keys"
**When** se renderiza
**Then** muestra los campos de clave como Input.Password:
- "OpenWeatherMap API Key"
- "Google Places API Key"
- "Brave Search API Key"
- "apimail · API Key"
- "Miniflux · API Key"
**And** muestra un campo de texto para "apimail · URL base"
**And** muestra un campo de texto para "Miniflux · URL base"
**And** muestra el campo del token de ZenNotes, que nunca revela el valor guardado

#### Scenario: API Keys se guardan
**Given** el usuario introduce una API key de OpenWeatherMap
**When** guarda
**Then** `updateSettings` se llama con la API key incluida

#### Scenario: La configuración de apimail se muestra y se guarda
**Given** el SettingsDialog abierto en la tab "API Keys" con `apimail_base_url` y `apimail_api_key` en settings
**When** se renderiza
**Then** el campo de URL base muestra el valor vigente
**And** el campo de la API key está enmascarado
**And** al editar la API key y guardar, `updateSettings` se llama con `apimail_api_key`

#### Scenario: La URL base tiene un valor por defecto
**Given** el SettingsDialog abierto en la tab "API Keys" sin `apimail_base_url` en settings
**When** se renderiza el campo de URL base
**Then** su placeholder es `https://apimail.territoriolinux.es`

#### Scenario: La configuración de Miniflux se muestra y se guarda
**Given** el SettingsDialog abierto en la tab "API Keys" con `miniflux_base_url` y `miniflux_api_key` en settings
**When** se renderiza
**Then** el campo de URL base muestra el valor vigente
**And** el campo del token está enmascarado
**And** al editar el token y guardar, `updateSettings` se llama con `miniflux_api_key`

#### Scenario: La URL base de Miniflux tiene un valor por defecto
**Given** el SettingsDialog abierto en la tab "API Keys" sin `miniflux_base_url` en settings
**When** se renderiza el campo de URL base de Miniflux
**Then** su placeholder es `https://miniflux.territoriolinux.es`

#### Scenario: El token de ZenNotes nunca muestra el valor guardado
**Given** el SettingsDialog abierto en la tab "API Keys" con el token de ZenNotes configurado
**When** se renderiza
**Then** el campo del token está vacío y muestra el estado "Configurado"
**And** el valor del token NO se lee desde `GET /api/settings`

#### Scenario: Guardar el token de ZenNotes usa su endpoint dedicado
**Given** el usuario escribe un token de ZenNotes en el campo
**When** guarda
**Then** se llama a `PUT /api/zennotes/token` con el token
**And** NO se envía `zennotes_token` en `PUT /api/settings`
