## ADDED Requirements

### Requirement: SettingsDialog SHALL display a Mastodon section in Integraciones

El diálogo de ajustes SHALL mostrar una sub-pestaña «Mastodon» dentro de «Integraciones», con los campos `mastodon_base_url` y `mastodon_token`, un botón «Probar conexión» y una línea de estado. El campo del token SHALL empezar siempre vacío y SHALL no releer el valor guardado; su guardado SHALL usar `PUT /api/mastodon/token`. La URL base SHALL guardarse con `PUT /api/settings`. La sonda SHALL llamar a `GET /api/mastodon/check` y el estado SHALL derivarse de `GET /api/mastodon/status`. Las funciones de API SHALL vivir en `src/api/mastodon.ts` y el hook en `src/hooks/useMastodon.ts`, sin añadir llamadas a `src/api/client.ts`.

#### Scenario: Token nunca se precarga
- **GIVEN** un `mastodon_token` ya guardado
- **WHEN** se abre la sub-pestaña «Mastodon»
- **THEN** el campo del token está vacío

#### Scenario: Estado de la integración
- **GIVEN** `GET /api/mastodon/status` devuelve `configured: true`
- **WHEN** se renderiza la sub-pestaña
- **THEN** la línea de estado indica «Configurado» y la etiqueta incluye texto accesible, no solo color

#### Scenario: Prueba de conexión
- **GIVEN** el usuario pulsa «Probar conexión»
- **WHEN** `GET /api/mastodon/check` devuelve `{ok: false, detail: "..."}`
- **THEN** se muestra el `detail` devuelto y no se muestra ningún token
