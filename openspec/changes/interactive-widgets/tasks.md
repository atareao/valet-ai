# Tasks

## 1. Backend — RED

- [x] 1.1 Test unitario de `RenderWidgetTool`: nombre permitido → `Ok`; nombre desconocido o ausente → `Err(ToolError::InvalidArguments)`; `data` ausente/no objeto → objeto vacío; `permission` = `NoConfirm`.
- [x] 1.2 Test de `build_tool_registry`: `render_widget` figura entre las herramientas y en `definitions()`.
- [x] 1.3 Test del orquestador: una tool call `render_widget` con éxito emite `SSEEvent::Widget` con `id` no vacío y `data` intacto; un nombre inválido no emite evento.
- [x] 1.4 Test de `ToolsRepo`: tras sincronizar con el catálogo de producción, `list` incluye `render_widget`; deshabilitarla la devuelve en `disabled_names`.
- [x] 1.5 Test de API (`tests/api/tools.rs`): `GET /api/tools` incluye `render_widget` y `PUT /api/tools/{id}/toggle` funciona sobre ella.
- [x] 1.6 `cargo test` → los tests nuevos fallan y el resto sigue verde.

## 2. Backend — GREEN

- [x] 2.1 Añadir la variante `SSEEvent::Widget { id, name, data }` (`#[serde(rename = "widget")]`).
- [x] 2.2 Crear `RenderWidgetTool` (allowlist + validación de `data`) y registrarla en `build_tool_registry`.
- [x] 2.3 En `process_message_stream`, emitir `SSEEvent::Widget` cuando la tool ejecutada sea `render_widget` con éxito.
- [x] 2.4 Actualizar los espejos del catálogo en tests: `REGISTRY_NAMES` de `src/db/repos/tools.rs` (13 nombres) y sus `assert_eq!(tools.len(), 12)` → `13`; la lista esperada de `tests/api/tools.rs` (+`render_widget`) y sus comentarios.
- [x] 2.5 `cargo test` → todo verde; `cargo check`.

## 3. Backend — REFACTOR y PR

- [x] 3.1 `cargo clippy --all-targets -- -D warnings` y `cargo fmt`.
- [x] 3.2 Revisión (`rust-reviewer`).
- [ ] 3.3 PR a `development` (backend).

## 4. Frontend — RED

- [ ] 4.1 Tests de `WIDGET_REGISTRY`/`WidgetRenderer`: widget conocido se pinta; desconocido muestra aviso y no revienta.
- [ ] 4.2 Test de `useSSE`: el evento `type:"widget"` invoca el callback con `id`/`name`/`data`.
- [ ] 4.3 Tests de `useMainChat`: un widget en vivo se asocia al mensaje del asistente; `sendWidgetAction` envía un turno de usuario con el envoltorio estable.
- [ ] 4.4 Tests de `QuickFormWidget` y `ChecklistWidget`: submit llama a `onAction` con el payload esperado.
- [ ] 4.5 `npm test` → los tests nuevos fallan y el resto sigue verde.

## 5. Frontend — GREEN

- [ ] 5.1 Tipos (`WidgetInstance`, evento SSE `widget`) y `formatWidgetAction`.
- [ ] 5.2 `useSSE`: manejar `type:"widget"` y exponerlo por callback.
- [ ] 5.3 `useMainChat`: acumular widgets por mensaje y `sendWidgetAction`.
- [ ] 5.4 `components/widgets/registry.ts`, `WidgetRenderer.tsx`, `QuickFormWidget.tsx`, `ChecklistWidget.tsx` (antd).
- [ ] 5.5 `MessageBubble`/`ChatView`: renderizar los widgets del mensaje tras el Markdown.
- [ ] 5.6 `npm test` → todo verde; `npm run typecheck`.

## 6. Frontend — REFACTOR y PR

- [ ] 6.1 `npm run lint` (0 warnings) y limpieza.
- [ ] 6.2 Revisión (`react-reviewer`).
- [ ] 6.3 PR a `development` (frontend).

## 7. Cierre

- [ ] 7.1 Verificación de extremo a extremo, incluida la presencia de `render_widget` en `GET /api/tools` y su toggle desde la UI.
- [ ] 7.2 `openspec archive interactive-widgets` y PR de archivado.
