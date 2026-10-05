//! Session authentication middleware.
//!
//! Enforces a valid Valet session on the protected `/api/*` routes when
//! `auth.enabled` is `true`. `/api/health` and everything under `/api/auth/*`
//! stay reachable without a session, and with OIDC disabled the middleware is a
//! pass-through so the dev user can use the API untouched.

use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum_extra::extract::cookie::CookieJar;

use crate::auth::{validate_session, SESSION_COOKIE_NAME};
use crate::AppState;

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

    if is_exempt(request.uri().path()) {
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

/// Routes that never require a session.
fn is_exempt(path: &str) -> bool {
    !path.starts_with("/api/") || path == "/api/health" || path.starts_with("/api/auth/")
}
