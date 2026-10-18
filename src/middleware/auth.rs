//! Session authentication middleware.
//!
//! Enforces a valid Valet session on the protected `/api/*` routes when
//! `auth.enabled` is `true`. `/api/health` and everything under `/api/auth/*`
//! stay reachable without a session, and with OIDC disabled the middleware is a
//! pass-through so the dev user can use the API untouched.
//!
//! The session exemption is keyed on the **(method, path)** pair, not on the
//! path alone: `POST /api/notifications` is reachable without a session because
//! it is protected by an ingest token instead
//! ([`require_ingest_key`]), while `GET /api/notifications/unread` and
//! `POST /api/notifications/mark-read` keep requiring a session.

use axum::extract::{Request, State};
use axum::http::{header, Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum_extra::extract::cookie::CookieJar;

use crate::auth::{validate_session, SESSION_COOKIE_NAME};
use crate::db::repos::ingest_keys::IngestRepo;
use crate::AppState;

/// The ingest key that authenticated a `POST /api/notifications` request.
///
/// Placed in the request extensions by [`require_ingest_key`] so the handler
/// can default `origin`/`author` to the key `name`. It never carries the token:
/// [`IngestRepo`] stores only the hash and the prefix.
#[derive(Debug, Clone)]
pub struct AuthenticatedIngestKey {
    /// The key `id` (UUID), useful for correlating logs.
    pub id: String,
    /// The key `name`, used to fill `origin`/`author` when they are absent.
    pub name: String,
}

pub async fn session_middleware(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    let Some(config) = state.auth_config.as_ref() else {
        return next.run(request).await;
    };

    if !config.enabled {
        return next.run(request).await;
    }

    if is_exempt(request.method(), request.uri().path()) {
        return next.run(request).await;
    }

    let jar = CookieJar::from_headers(request.headers());
    let Some(session) = jar.get(SESSION_COOKIE_NAME) else {
        return StatusCode::UNAUTHORIZED.into_response();
    };

    match validate_session(config, session.value()) {
        Ok(_) => next.run(request).await,
        Err(_) => StatusCode::UNAUTHORIZED.into_response(),
    }
}

/// Routes that never require a session, keyed on the **(method, path)** pair.
///
/// `/api/health` and everything under `/api/auth/*` are exempt with any method.
/// `POST /api/notifications` is exempt too —it is authenticated by an ingest
/// token, see [`require_ingest_key`]— but the reads
/// (`GET /api/notifications/unread`, `POST /api/notifications/mark-read`) are
/// **not**: they keep requiring a session.
fn is_exempt(method: &Method, path: &str) -> bool {
    if !path.starts_with("/api/") {
        return true;
    }
    if path == "/api/health" || path.starts_with("/api/auth/") {
        return true;
    }
    method == Method::POST && path == "/api/notifications"
}

/// Middleware that authenticates the ingest token of `POST /api/notifications`.
///
/// Reads `Authorization: Bearer <token>`, requires the `Bearer` scheme and looks
/// the token up by its SHA-256 with [`IngestRepo::find_by_hash`] —the token in
/// clear is never persisted, so no plaintext comparison happens—. A missing,
/// malformed, unknown or revoked key is rejected with `401`; a valid one updates
/// `last_used_at` and is left in the request extensions as
/// [`AuthenticatedIngestKey`] for the handler.
///
/// The token is **never** logged: at most its key `id`/`name` are recorded.
pub async fn require_ingest_key(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Response {
    let authorization = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok());

    let Some(authorization) = authorization else {
        return unauthorized("missing_authorization");
    };

    let Some(token) = authorization
        .strip_prefix("Bearer ")
        .filter(|token| !token.is_empty())
    else {
        return unauthorized("malformed_authorization");
    };

    match IngestRepo::find_by_hash(&state.db, token).await {
        Ok(Some(key)) => {
            tracing::debug!(
                key_id = %key.id,
                key_name = %key.name,
                "ingest: clave autenticada"
            );
            request.extensions_mut().insert(AuthenticatedIngestKey {
                id: key.id,
                name: key.name,
            });
            next.run(request).await
        }
        Ok(None) => unauthorized("unknown_or_revoked_key"),
        Err(error) => {
            tracing::error!(error = %error, "ingest: no se pudo buscar la clave");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// Builds the `401` response shared by every ingest rejection path.
///
/// Logs only the `reason`, never the token nor any prefix of it, and answers
/// with a constant JSON body so the rejection gives away nothing.
fn unauthorized(reason: &str) -> Response {
    tracing::warn!(reason, "ingest: token de ingesta denegado");
    (
        StatusCode::UNAUTHORIZED,
        axum::Json(serde_json::json!({ "error": "unauthorized" })),
    )
        .into_response()
}
