# frontend — Delta (change oximeal-m2m-resource)

## MODIFIED Requirements

### Requirement: SettingsDialog SHALL display an OxiMeal section in Integraciones

Ajustes → Integraciones SHALL incluir una sección «OxiMeal» con campos no sensibles `oximeal_base_url`, `oximeal_resource`, `oximeal_issuer_url` y `oximeal_client_id` (por `PUT /api/settings`), y `client_secret`, que SHALL arrancar siempre vacío y SHALL enviarse solo por `PUT /api/oximeal/secret`. SHALL mostrar el estado de `GET /api/oximeal/status` sin el secreto, incluyendo el `resource` resuelto, y SHALL ofrecer «Probar conexión» (`GET /api/oximeal/check`) mostrando su `detail`.

#### Scenario: El secreto arranca vacío y no se relee
- **Given** un `oximeal_client_secret` ya guardado
- **When** se abre la sección «OxiMeal» de Ajustes → Integraciones
- **Then** el campo del secreto está vacío
- **And** el frontend no lo lee de ninguna respuesta

#### Scenario: Probar conexión muestra el veredicto
- **Given** la sección «OxiMeal» abierta
- **When** se pulsa «Probar conexión»
- **Then** se llama a `GET /api/oximeal/check`
- **And** se muestra el `detail` de la respuesta, informando del resultado

#### Scenario: Guardar el secreto usa su endpoint dedicado
- **Given** el campo del secreto relleno con un valor
- **When** se guarda la sección
- **Then** el valor viaja por `PUT /api/oximeal/secret`
- **And** los ajustes no sensibles viajan por `PUT /api/settings`

#### Scenario: El campo Resource viaja con los ajustes no sensibles
- **Given** la sección «OxiMeal» de Ajustes → Integraciones
- **When** se guarda con `oximeal_resource` relleno
- **Then** el valor viaja por `PUT /api/settings` como los otros campos no sensibles
- **And** el campo se etiqueta «OxiMeal · Resource» con placeholder `https://oximeal.territoriolinux.es/api`

#### Scenario: La línea de estado refleja el resource resuelto
- **Given** `GET /api/oximeal/status` devolviendo `resource: "https://oximeal.territoriolinux.es/api"`
- **When** se muestra la línea de estado de la sección «OxiMeal»
- **Then** se refleja ese `resource` resuelto
- **And** NO se muestra el secreto
