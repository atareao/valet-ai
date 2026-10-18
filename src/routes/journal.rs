//! Rutas HTTP de la bitácora (`/api/journal`).
//!
//! El listado por rango y la lectura, el alta o reemplazo y el borrado de un día
//! concreto. Todas exigen sesión OIDC: el middleware de sesión ya la impone
//! sobre `/api/*`, así que aquí no hay nada que autenticar.
//!
//! Ver `openspec/changes/journal-entries/specs/journal-api/spec.md`.

use axum::routing::get;
use axum::Router;

use crate::handlers::journal;
use crate::AppState;

/// Registra las rutas de `/api/journal`.
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/journal", get(journal::list_entries))
        .route(
            "/api/journal/{date}",
            get(journal::get_entry)
                .put(journal::upsert_entry)
                .delete(journal::delete_entry),
        )
}
