# Proposal

## Why

La guía de widgets ya pide al modelo invocar `render_widget` para **datos complejos o geográficos**
(«Muestres direcciones, rutas, mapas, tablas o estadísticas»), pero el registro solo admite `QuickForm`
y `Checklist`: no existe ningún widget de mapa. Cuando el modelo intenta mostrar una ubicación, o cae en
texto plano, o (si inventa un `widget_name`) es rechazado por la allowlist.

Este change introduce el **primer widget de mapa** y deja montada la base reutilizable (librería,
contrato de datos, allowlist, registry, patrón de test con la librería mockeada) para los siguientes
(Route, PlacesExplore, WeatherRadar).

## What Changes

- **`tools/registry` (MODIFIED)**: la definición de `render_widget` documenta además el esquema de
  `data` de `LocationWidget` (`{ title, description?, latitude, longitude, address? }`) y su
  `widget_name` entra en la allowlist.
- **`frontend` (ADDED)**: nuevo widget `LocationWidget` — mapa Leaflet (tiles oscuros), marcador con
  popup, panel de detalle y acciones. Se carga de forma **diferida** para no meter la librería de mapas
  en el bundle inicial.

**Fuera de alcance (decidido en diseño):** los widgets Route/PlacesExplore/WeatherRadar; persistencia de
lugares guardados (el botón «Guardar» devuelve la acción como turno de usuario, sin backend nuevo); los
widgets de tabla/estadísticas; cambios en el `system_prompt` (el trigger geográfico ya existe).

## Capabilities

### New Capabilities

- Ninguna.

### Modified Capabilities

- `tools/registry` (se modifica un requisito: se amplía el esquema documentado y la allowlist).
- `frontend` (se añade un requisito: el widget `LocationWidget`).

## Impact

- **Backend**: `src/tools/widget.rs` (`ALLOWED_WIDGETS` y `DATA_SCHEMA_DESCRIPTION`) y su test asociado,
  que hoy fija el enum a `["QuickForm","Checklist"]`.
- **Frontend**: dependencias nuevas `leaflet`, `react-leaflet` y `@types/leaflet`; nuevo
  `components/widgets/LocationWidget.tsx`, tipos en `types.ts`, entrada en `registry.ts`, y tests con
  Leaflet mockeado (jsdom no renderiza mapas reales). Posible `Suspense` en `WidgetRenderer`.
- **No** se toca `docker-compose.prod.yml` ni la BD.
