# Design

## Context

- Los widgets ya son genéricos: `WIDGET_REGISTRY` mapea nombre → componente; `WidgetRenderer` lo pinta
  con `data` y `onAction`; las acciones vuelven al backend como turno de usuario
  (`[widget:Name#id] action payload`).
- El backend valida `widget_name` contra `ALLOWED_WIDGETS = ["QuickForm","Checklist"]`.
- `App.tsx` usa antd `darkAlgorithm` → tema oscuro; `index.html` no fija CSP.
- `vite.config.ts` usa `manualChunks` (vendor-react, vendor-antd) y hay un test de carga diferida de
  dependencias.
- React 18 + antd 5; no hay librería de mapas instalada.

## Goals / Non-Goals

- **Goal:** un `LocationWidget` que muestre una ubicación en un mapa, con acciones útiles, integrado en
  el sistema de widgets existente y sin inflar el bundle inicial.
- **Non-Goal:** Route/PlacesExplore/WeatherRadar; persistencia de lugares; tabla/estadísticas; cambios
  en el prompt.

## Decisions

- **Leaflet + react-leaflet** (no MapLibre ni Google): ligera, sin API key, tiles OSM/CartoDB. Se fijan
  versiones compatibles con React 18 (`react-leaflet@^4`, `leaflet@^1.9`; `react-leaflet@5` exige React 19).
- **Tiles oscuros** CartoDB *Dark Matter* para casar con el tema oscuro, con **atribución obligatoria**
  (OSM + CARTO) en el mapa.
- **Marcador sin assets PNG:** se usa `CircleMarker` (o un `divIcon`) en lugar del `Marker` por defecto
  de Leaflet, evitando el baile del icono (`L.icon` + imports de PNG) y facilitando los tests en jsdom.
- **Carga diferida:** la entrada del registry es un componente `lazy(() => import("./LocationWidget"))`,
  de modo que `leaflet`/`react-leaflet` solo se descargan cuando aparece un `LocationWidget`.
  `WidgetRenderer` envuelve el componente en `Suspense` con un fallback discreto.
- **Contrato tolerante:** si `latitude`/`longitude` faltan o no son números finitos, el widget muestra un
  aviso (no un mapa roto) y no lanza. `title` cae a `address` y, si no, a un rótulo genérico.
- **Acciones:**
  - **Guardar** → `onAction("save_place", { title, address, latitude, longitude })` (turno de usuario,
    sin persistencia).
  - **Cómo llegar** → abre `https://www.google.com/maps/search/?api=1&query=<lat>,<lon>` en pestaña nueva.
  - **Copiar coordenadas** → `navigator.clipboard.writeText`.
- **`scrollWheelZoom` desactivado** para no secuestrar el scroll del chat.

## Contracts

### `data` de `LocationWidget`

```jsonc
{
  "title": "Restaurante El Laurel",   // obligatorio en la práctica
  "description": "Comida mediterránea con terraza.",
  "latitude": 39.4699,                 // number
  "longitude": -0.3763,                // number
  "address": "Calle Mayor 12, Valencia"
}
```

### TypeScript (`widgets/types.ts`)

```ts
export interface LocationData {
  title?: string;
  description?: string;
  latitude?: number;
  longitude?: number;
  address?: string;
}
```

### Backend (`src/tools/widget.rs`)

```rust
const ALLOWED_WIDGETS: &[&str] = &["QuickForm", "Checklist", "LocationWidget"];
// DATA_SCHEMA_DESCRIPTION añade:
// LocationWidget: {"title": str, "description": str, "latitude": num, "longitude": num, "address": str}.
```

## Risks / Trade-offs

- [Leaflet en jsdom no renderiza] → Los tests mockean `react-leaflet`/`leaflet`; se prueba el
  comportamiento del widget (texto, botones, `onAction`), no el mapa real. El E2E visual se valida en la
  prueba real contra el contenedor.
- [Peso del bundle] → Mitigado con carga diferida y chunk propio.
- [El modelo da coordenadas incorrectas o ausentes] → Contrato tolerante + aviso; nunca revienta.
- [Dependencia externa de tiles] → Fallback visual si los tiles no cargan; la atribución se mantiene.

## Migration Plan

Sin migración de datos. Solo backend (allowlist/esquema) y frontend (nuevo widget + dependencias).
`docker-compose.prod.yml` no se toca.
