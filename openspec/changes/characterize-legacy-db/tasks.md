# Tasks

## 1. Caracterización (Phase 0)

- [x] 1.1 Contrastar cada requisito con el código/migraciones: `src/db/repos/{reminders,tasks,notes}.rs`, `migrations/20260925000001_initial.sql` y `migrations/20260926000001_gtd_statuses.sql`.
- [x] 1.2 Verificar por CLI que el comportamiento documentado pasa: `cargo test --no-fail-fast` (0 failed).
- [x] 1.3 Validación `openspec validate characterize-legacy-db --strict`.

## 2. Cierre

- [x] 2.1 Revisión (que solo describa el comportamiento actual).
- [ ] 2.2 PR a `development`; tras el merge, PR de archivado (`openspec archive characterize-legacy-db`).
- [ ] 2.3 Tras el archive, rellenar el `## Purpose` de las specs creadas (`db/repos` y `db/schema` ya existen: solo si se creara capability nueva).
