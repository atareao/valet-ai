# Tasks: user-name-in-prompt

## Bloque 1 — RED (backend)
- [x] 1.1 Test de `ProfilesRepo::get_by_id` en `src/db/repos/profiles.rs`: devuelve `Some(Profile)` si existe y `None` si no, sin insertar filas. Ejecutar `cargo test --lib db::repos::profiles`.
- [x] 1.2 Test en `src/orchestrator/agent.rs`: con un perfil `name = "Lorenzo"`, el mensaje de sistema contiene `# USUARIO` y `El nombre del usuario es Lorenzo.`
- [x] 1.3 Test: la sección del nombre aparece **después del prompt y antes de la de memoria persistente**.
- [x] 1.4 Test: con `name` vacío o en blanco, NO aparece `# USUARIO` ni separador de relleno.
- [x] 1.5 Test: con `name = "Valet User"` (por defecto), la sección NO se inyecta.
- [x] 1.6 Verificar por CLI: `cargo test` → los nuevos en rojo y los legacy en verde.

## Bloque 2 — GREEN (backend)
- [x] 2.1 Añadir `ProfilesRepo::get_by_id(pool, id) -> Result<Option<Profile>, sqlx::Error>`.
- [x] 2.2 Leer el nombre por `profile_id` en `process_message_stream`, tolerando fallo con `warn` y omisión.
- [x] 2.3 Componer la sección `# USUARIO` en código e insertarla en `compose_system_message` entre el prompt y la memoria persistente.
- [x] 2.4 Verificar por CLI: `cargo test` y `cargo check`.

## Bloque 3 — REFACTOR y revisión
- [x] 3.1 `cargo fmt --check` y `cargo clippy --all-targets -- -D warnings`.
- [x] 3.2 Revisión con `rust-reviewer`.
- [x] 3.3 Verificación final por CLI y PR a `development`.
