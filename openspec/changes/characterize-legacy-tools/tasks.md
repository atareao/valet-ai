# Tasks

## 1. Caracterización (Phase 0)

- [x] 1.1 Contrastar cada spec delta con el código y sus tests: `reminders` (`src/tools/reminders.rs`), `tasks` (`src/tools/tasks.rs`), `notes` (`src/tools/notes.rs`), `time-location` (`src/tools/current_time.rs`, `src/tools/current_location.rs`).
- [x] 1.2 Confirmar el fallback Europe/Madrid en `get_current_time` (código + test) y corregir el UTC del delta archivado.
- [x] 1.3 Verificar por CLI que el comportamiento documentado está cubierto: `cargo test --no-fail-fast` (0 failed).

## 2. Cierre

- [x] 2.1 Validación `openspec validate characterize-legacy-tools --strict`.
- [ ] 2.2 Revisión de la documentación (que no invente comportamiento: solo el actual).
- [ ] 2.3 PR a `development`; tras el merge, PR de archivado (`openspec archive characterize-legacy-tools`).

## 3. Follow-up (fuera de este change)

- [ ] 3.1 Change aparte para `db/repos` (RemindersRepo/TasksRepo/NotesRepo) y `db/schema` (tablas `reminders`/`tasks`/`notes`, claves `timezone`/`latitude`/`longitude`).
