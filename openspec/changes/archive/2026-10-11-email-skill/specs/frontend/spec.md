## MODIFIED Requirements

### Requirement: SettingsDialog SHALL display API Keys tab

SettingsDialog SHALL display an "API Keys" tab with password fields for API keys and the apimail connection settings.

**Given** el SettingsDialog está abierto en la tab "API Keys"
**When** se renderiza
**Then** muestra los campos de clave como Input.Password:
- "OpenWeatherMap API Key"
- "Google Places API Key"
- "Brave Search API Key"
- "apimail · API Key"
**And** muestra un campo de texto para "apimail · URL base"

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
