//! Rutas HTTP de las notas (`/api/notes`).
//!
//! El listado con filtro por categoría y tope, el alta, la lectura, la
//! actualización parcial y el borrado de una nota concreta. Todas exigen sesión
//! OIDC: el middleware de sesión ya la impone sobre `/api/*`, así que aquí no
//! hay nada que autenticar.
//!
//! Ver `openspec/changes/notes-crud/specs/notes-api/spec.md`.

use axum::routing::get;
use axum::Router;

use crate::handlers::notes;
use crate::AppState;

/// Registra las rutas de `/api/notes`.
pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/api/notes",
            get(notes::list_notes).post(notes::create_note),
        )
        .route(
            "/api/notes/{id}",
            get(notes::get_note)
                .put(notes::update_note)
                .delete(notes::delete_note),
        )
}
