# Tasks

## 1. Backend — RED

- [x] 1.1 En `tests/db/migrations.rs`, test de instalación nueva: tras `run_migrations()` sobre base vacía, `system_prompt` contiene `# Instrucciones de Interfaz y Widgets Interactivos` y `render_widget`.
- [x] 1.2 Test de preservación: con `system_prompt = 'Mi prompt personalizado'`, tras la migración el valor contiene el texto del usuario **y** la sección, con el texto del usuario primero.
- [x] 1.3 Test de idempotencia: ejecutar `run_migrations()` dos veces deja **una sola** aparición del marcador.
- [x] 1.4 `cargo test` → los tests nuevos fallan y el resto sigue verde.

## 2. Backend — GREEN y PR

- [x] 2.1 Crear `migrations/20261004000001_widget_prompt_guidance.sql` con el anexado idempotente de la sección `# Instrucciones de Interfaz y Widgets Interactivos`.
- [x] 2.2 `cargo test` → todo verde; `cargo clippy --all-targets -- -D warnings` y `cargo fmt --check`.
- [x] 2.3 Revisión (`rust-reviewer`).

## 3. Cierre

- [x] 3.0 PR de la migración a `development`.
- [x] 3.1 Prueba real: reconstruir/redesplegar, confirmar que `settings.system_prompt` contiene la sección y repetir los prompts de checklist/formulario.
- [x] 3.2 `openspec archive widget-prompt-guidance` y PR de archivado.
