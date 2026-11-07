# Proposal

## Why

Al probar la integración, `GET /api/oximeal/check` devuelve «el IdP no entregó token». La causa es que la petición `client_credentials` no envía el parámetro `resource` (RFC 8707). Pocket ID exige `resource` para resolver la API de un scope personalizado, así que sin él rechaza la petición; y aunque la aceptara, el `aud` sería el `client_id` y OxiMeal —que valida `aud == OIDC_API_AUDIENCE`— respondería `401`. Sin este cambio la conexión Valet↔OxiMeal sigue rota.

## What Changes

- Enviar `resource=<API>` en la petición de token `client_credentials`.
- Resolver `resource` con precedencia ajuste (`oximeal_resource`) → entorno (`OXIMEAL_RESOURCE`) → **derivado** `<base_url>/api`, recortando la barra final. Para la config por defecto vale `https://oximeal.territoriolinux.es/api`.
- `check_detail` deja de ocultar el mensaje del IdP: `TokenEndpoint(detail)` → `el IdP no entregó un token M2M: <detail>` (el `detail` ya viene redactado por el servicio). Igual trato para el detalle de las caídas de OxiMeal.
- `GET /api/oximeal/status` incluye además `"resource": <string>` con el valor resuelto (nunca un secreto).
- La sub-pestaña OxiMeal de Ajustes → Integraciones añade el campo no sensible «OxiMeal · Resource» (`oximeal_resource`), que viaja en `GET`/`PUT /api/settings`, y su línea de estado refleja el `resource` resuelto.

## Capabilities

### New Capabilities

_(ninguna)_

### Modified Capabilities

- `oximeal-service`: la resolución de configuración gana `oximeal_resource`/`OXIMEAL_RESOURCE` con default derivado, y la petición M2M envía `resource`.
- `oximeal-api`: el estado expone `resource` y la sonda muestra el detalle del IdP sin filtrar secretos.
- `frontend`: la sección OxiMeal añade el campo Resource y refleja el `resource` resuelto.

## Impact

- Código: `src/services/oximeal.rs`, `src/handlers/oximeal.rs`, `frontend/src/components/OximealSection.tsx`, `frontend/src/hooks/useOximeal.ts`, `frontend/src/types/index.ts`.
- Sin cambios de migración: los ajustes nuevos tienen default derivado y no necesitan siembra.
- Sin el cambio, OxiMeal rechazaría el `aud` aunque el IdP aceptara la petición.
