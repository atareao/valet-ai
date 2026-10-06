# Tasks

## 1. Backend — RED

- [x] 1.1 Test de repo: `set_widgets` persiste el JSON y `find_by_id`/`list_all` lo devuelven; `widgets` es `None` cuando no hay.
- [x] 1.2 Test de migración: `messages` tiene la columna `widgets` tras `run_migrations()`, idempotente.
- [x] 1.3 Test del orquestador: un turno que invoca `render_widget` con éxito deja `widgets` no vacío en el mensaje assistant persistido.
- [x] 1.4 `cargo test` → los tests nuevos fallan y el resto verde (RED: 648 passed, 5 failed).

## 2. Backend — GREEN

- [x] 2.1 Migración `20261004000002_message_widgets.sql` (`ALTER TABLE messages ADD COLUMN widgets TEXT`).
- [x] 2.2 `Message.widgets`; `MessagesRepo` incluye `widgets` en los SELECT y añade `set_widgets`.
- [x] 2.3 El orquestador recoge los widgets emitidos en el turno y llama a `set_widgets` tras crear el mensaje assistant (reutilizando el `id` del evento).
- [x] 2.4 `cargo test` verde; `cargo clippy --all-targets -- -D warnings` y `cargo fmt --check`.

## 3. Frontend — RED/GREEN

- [x] 3.1 Tests: al cargar el historial, `useMainChat` puebla `widgetsByMessage` desde `message.widgets`; sin widgets no crea entradas.
- [x] 3.2 `Message.widgets?: WidgetInstance[] | null` y reconstrucción en `useMainChat` al recibir `chatInit`.
- [x] 3.3 `npm test` verde; `npm run typecheck` y `npm run lint`.

## 4. Revisión y cierre

- [x] 4.1 Revisión `rust-reviewer` y `react-reviewer`.
- [x] 4.2 PR (backend + frontend) a `development`. (PR #130, merge `c7de43f`)
- [x] 4.3 Prueba real: desplegar, renderizar un widget, **recargar** y comprobar que reaparece. (verificado en el despliegue el 2026-10-06: el widget reaparece al recargar)
- [x] 4.4 `openspec archive persist-rendered-widgets` y PR de archivado. (ejecutado: commit `20fe2fa`, deltas volcados a `openspec/specs/`)
