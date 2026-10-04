# Tasks

## 1. Backend — RED

- [x] 1.1 En `src/tools/widget.rs`, actualizar `test_render_widget_parameters_document_data_schema`: el enum de `widget_name` incluye `LocationWidget` y la descripción de `data` menciona `latitude`, `longitude` y `LocationWidget`.
- [x] 1.2 `cargo test` → el test falla y el resto sigue verde.

## 2. Backend — GREEN

- [x] 2.1 Añadir `"LocationWidget"` a `ALLOWED_WIDGETS` y su esquema a `DATA_SCHEMA_DESCRIPTION`.
- [x] 2.2 `cargo test` verde; `cargo clippy --all-targets -- -D warnings` y `cargo fmt --check`.

## 3. Frontend — RED

- [x] 3.1 Añadir dependencias `leaflet`, `react-leaflet@^4` y `@types/leaflet` (dev).
- [x] 3.2 `types.ts`: `LocationData`.
- [x] 3.3 Tests (con `react-leaflet`/`leaflet` mockeados): pinta título/dirección; botón «Guardar» llama a `onAction("save_place", …)`; sin coordenadas muestra aviso y no lanza; el registry incluye `LocationWidget`.
- [x] 3.4 `npm test` → los tests nuevos fallan y el resto sigue verde.

## 4. Frontend — GREEN

- [x] 4.1 `LocationWidget.tsx`: mapa Leaflet (tiles oscuros, atribución, `scrollWheelZoom` off), `CircleMarker` + popup, panel de detalle y acciones («Guardar», «Cómo llegar», «Copiar coordenadas»); contrato tolerante.
- [x] 4.2 Registro `lazy` en `registry.ts` y `Suspense` en `WidgetRenderer` si procede.
- [x] 4.3 `npm test` verde; `npm run typecheck` y `npm run lint`.

## 5. Revisión y cierre

- [x] 5.1 Revisión `rust-reviewer` y `react-reviewer`.
- [ ] 5.2 PR único (backend + frontend) a `development`.
- [ ] 5.3 Prueba real: redesplegar y pedir una ubicación («¿dónde está…?») comprobando que llega `widget: LocationWidget` con coordenadas.
- [ ] 5.4 `openspec archive location-widget` y PR de archivado.
