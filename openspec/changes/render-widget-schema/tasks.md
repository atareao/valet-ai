# Tasks

## 1. Backend — RED

- [x] 1.1 Test de `RenderWidgetTool::parameters()`: el esquema documenta `fields` con `name`/`label`/`type` y los tipos admitidos (incluidos `number` y `textarea`), y `items` con `id`/`label`.
- [x] 1.2 `cargo test` → el test nuevo falla y el resto sigue verde.

## 2. Backend — GREEN y PR

- [x] 2.1 Documentar el esquema de `data` de `QuickForm` y `Checklist` en `parameters()` (y/o `description()`), con un ejemplo.
- [x] 2.2 `cargo test` → todo verde; `cargo clippy --all-targets -- -D warnings` y `cargo fmt --check`.
- [x] 2.3 Revisión (`rust-reviewer`).

## 3. Frontend — RED

- [x] 3.1 Tests de `QuickForm`: campo `number` (entrada numérica) y campo `textarea`; submit con sus valores.
- [x] 3.2 Tests de tolerancia: `QuickForm` sin `title` usa `description`; `Checklist` con `items[].text` y sin `id`.
- [x] 3.3 `npm test` → los tests nuevos fallan y el resto sigue verde.

## 4. Frontend — GREEN y PR

- [x] 4.1 Añadir `number` y `textarea` a `QuickFormField.type` y a `QuickFormWidget`.
- [x] 4.2 Tolerancia: `title ?? description` en `QuickForm`; `label ?? text` e `id` sintetizado en `Checklist`.
- [x] 4.3 `npm test` → todo verde; `npm run typecheck` y `npm run lint`.
- [x] 4.4 Revisión (`react-reviewer`).

## 5. Cierre

- [ ] 5.0 PR único (backend + frontend) a `development`.
- [ ] 5.1 Prueba real contra el contenedor: repetir los dos prompts y comprobar que `Checklist` llega con `items[{id,label}]` y `QuickForm` con el tipo numérico.
- [ ] 5.2 `openspec archive render-widget-schema` y PR de archivado.
