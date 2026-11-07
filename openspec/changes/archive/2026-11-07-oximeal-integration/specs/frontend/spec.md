# frontend — Delta de especificación (change oximeal-integration)

## ADDED Requirements

### Requirement: SettingsDialog SHALL display an OxiMeal section in Integraciones

El diálogo de ajustes SHALL incluir una sección «OxiMeal» dentro de la pestaña «Integraciones», con campos editables para `oximeal_base_url`, `oximeal_issuer_url` y `oximeal_client_id` (no sensibles, guardados por `PUT /api/settings`) y un campo para el `client_secret` que SHALL arrancar **siempre vacío** al abrir la sección y SHALL enviarse **solo** por `PUT /api/oximeal/secret`, sin releer nunca el valor guardado. La sección SHALL mostrar el estado de configuración obtenido de `GET /api/oximeal/status` sin revelar el secreto, y SHALL ofrecer un botón «Probar conexión» que llama a `GET /api/oximeal/check` y muestra el `detail` devuelto.

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
