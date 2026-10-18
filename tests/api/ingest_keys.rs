//! Tests de integración de las rutas `/api/ingest-keys`.
//!
//! Cubre los requisitos de `openspec/specs/ingest-keys/spec.md`: el alta que
//! devuelve el token una única vez, el listado
//! que nunca expone un token y la revocación idempotente. La sesión OIDC la
//! impone el middleware de sesión; estos tests corren con OIDC desactivado (el
//! middleware es un passthrough), como el resto del arnés `TestApp`.

mod common;
use common::TestApp;

use axum::http::StatusCode;
use base64::Engine as _;
use serde_json::{json, Value};

use valet::db::repos::ingest_keys::{generate_token, IngestRepo};

const INGEST_KEYS: &str = "/api/ingest-keys";

/// Siembra una clave de ingesta con un token conocido y devuelve `(app, token, id)`.
async fn app_with_key(name: &str) -> (TestApp, String, String) {
    let app = TestApp::new_empty().await;
    let token = generate_token();
    let key = IngestRepo::create(&app.db, name, &token)
        .await
        .expect("seed ingest key");
    (app, token, key.id)
}

/// Lee `revoked_at` de la fila `id`; `None` si la fila no existe.
async fn revoked_at(app: &TestApp, id: &str) -> Option<Option<String>> {
    sqlx::query_scalar::<_, Option<String>>("SELECT revoked_at FROM ingest_keys WHERE id = ?1")
        .bind(id)
        .fetch_optional(&app.db)
        .await
        .expect("read revoked_at")
}

// ---------------------------------------------------------------------------
// POST /api/ingest-keys
// ---------------------------------------------------------------------------

/// Scenario: crear una clave devuelve su token una vez.
#[tokio::test]
async fn create_returns_201_with_id_name_prefix_and_token() {
    let app = TestApp::new_empty().await;

    let resp = app
        .post(INGEST_KEYS)
        .json(&json!({ "name": "monitor" }))
        .send()
        .await;

    assert_eq!(resp.status(), StatusCode::CREATED);
    let body = resp.json::<Value>().await;

    let id = body["id"].as_str().expect("id must be a string");
    assert!(uuid::Uuid::parse_str(id).is_ok(), "id must be a UUID");
    assert_eq!(body["name"], "monitor");
    let token = body["token"].as_str().expect("token must be a string");
    assert!(!token.is_empty(), "the token must not be empty");
    assert_eq!(body["prefix"], &token[..12]);
}

/// Scenario: el token tiene el prefijo y la forma esperados.
#[tokio::test]
async fn create_token_has_val_prefix_and_base64url_payload() {
    let app = TestApp::new_empty().await;

    let resp = app
        .post(INGEST_KEYS)
        .json(&json!({ "name": "monitor" }))
        .send()
        .await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body = resp.json::<Value>().await;
    let token = body["token"].as_str().expect("token").to_string();

    let payload = token
        .strip_prefix("val_")
        .expect("the token must start with `val_`");
    assert!(!payload.is_empty(), "the payload must not be empty");
    assert!(
        !payload.contains(['+', '/', '=']),
        "the payload must be base64url without padding: {payload}"
    );
    let decoded = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload)
        .expect("the payload must be valid base64url");
    assert_eq!(decoded.len(), 32, "the payload must decode to 32 bytes");
}

/// Scenario: el prefijo son los 12 primeros caracteres del token.
#[tokio::test]
async fn create_prefix_is_the_first_twelve_characters() {
    let app = TestApp::new_empty().await;

    let resp = app
        .post(INGEST_KEYS)
        .json(&json!({ "name": "monitor" }))
        .send()
        .await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body = resp.json::<Value>().await;
    let token = body["token"].as_str().expect("token");
    assert_eq!(body["prefix"].as_str().expect("prefix"), &token[..12]);
}

/// Scenario: un nombre en blanco se rechaza.
#[tokio::test]
async fn create_missing_or_blank_name_returns_400() {
    let app = TestApp::new_empty().await;

    for body in [json!({}), json!({ "name": "   " })] {
        let resp = app.post(INGEST_KEYS).json(&body).send().await;
        assert_eq!(
            resp.status(),
            StatusCode::BAD_REQUEST,
            "a missing or blank name must be rejected: {body}"
        );
        let body = resp.json::<Value>().await;
        assert!(body["error"].is_string(), "the body must carry an error");
    }
}

/// Scenario: un nombre de más de 64 bytes se rechaza.
#[tokio::test]
async fn create_too_long_name_returns_400() {
    let app = TestApp::new_empty().await;

    let resp = app
        .post(INGEST_KEYS)
        .json(&json!({ "name": "a".repeat(65) }))
        .send()
        .await;

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body = resp.json::<Value>().await;
    assert!(body["error"].is_string(), "the body must carry an error");
}

/// Scenario: un nombre duplicado se rechaza con 409.
#[tokio::test]
async fn create_duplicate_name_returns_409() {
    let (app, _token, _id) = app_with_key("monitor").await;

    let resp = app
        .post(INGEST_KEYS)
        .json(&json!({ "name": "monitor" }))
        .send()
        .await;

    assert_eq!(resp.status(), StatusCode::CONFLICT);
    let body = resp.json::<Value>().await;
    assert!(body["error"].is_string(), "the body must carry an error");
}

// ---------------------------------------------------------------------------
// GET /api/ingest-keys
// ---------------------------------------------------------------------------

/// Scenario: sin claves la lista es vacía.
#[tokio::test]
async fn list_without_keys_returns_empty_list() {
    let app = TestApp::new_empty().await;

    let resp = app.get(INGEST_KEYS).await;

    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.json::<Value>().await;
    let data = body.as_array().expect("the body must be a JSON array");
    assert!(data.is_empty(), "the list must be empty");
}

/// Scenario: la lista incluye activas y revocadas sin exponer ningún token.
#[tokio::test]
async fn list_includes_active_and_revoked_without_token() {
    let app = TestApp::new_empty().await;
    let active_token = generate_token();
    let revoked_token = generate_token();
    let active = IngestRepo::create(&app.db, "activa", &active_token)
        .await
        .expect("seed active key");
    let revoked = IngestRepo::create(&app.db, "revocada", &revoked_token)
        .await
        .expect("seed revoked key");
    IngestRepo::revoke(&app.db, &revoked.id)
        .await
        .expect("revoke");

    let resp = app.get(INGEST_KEYS).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.json::<Value>().await;
    let raw = body.to_string();

    assert!(
        !raw.contains(&active_token),
        "the listing must not leak the active token"
    );
    assert!(
        !raw.contains(&revoked_token),
        "the listing must not leak the revoked token"
    );

    let data = body.as_array().expect("the body must be a JSON array");
    assert_eq!(data.len(), 2, "one entry per key, active and revoked");

    for item in data {
        for field in [
            "id",
            "name",
            "prefix",
            "created_at",
            "last_used_at",
            "revoked_at",
        ] {
            assert!(
                item.get(field).is_some(),
                "the item must expose `{field}`: {item}"
            );
        }
        assert!(
            item.get("token").is_none(),
            "no entry may carry a token: {item}"
        );
    }

    let listed = data
        .iter()
        .find(|k| k["id"] == active.id)
        .expect("the active key must be listed");
    assert!(
        listed["revoked_at"].is_null(),
        "the active key is not revoked"
    );
    let listed = data
        .iter()
        .find(|k| k["id"] == revoked.id)
        .expect("the revoked key must be listed");
    assert!(
        !listed["revoked_at"].is_null(),
        "the revoked key keeps a revoked_at"
    );
}

// ---------------------------------------------------------------------------
// DELETE /api/ingest-keys/{id}
// ---------------------------------------------------------------------------

/// Scenario: revocar conserva la fila con `revoked_at`.
#[tokio::test]
async fn delete_revokes_and_keeps_the_row() {
    let (app, _token, id) = app_with_key("monitor").await;

    let resp = app.delete(&format!("{INGEST_KEYS}/{id}")).await;

    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    let revoked = revoked_at(&app, &id).await.expect("the row must exist");
    assert!(revoked.is_some(), "the row keeps existing, now revoked");
}

/// Scenario: revocar dos veces no altera `revoked_at`.
#[tokio::test]
async fn delete_is_idempotent() {
    let (app, _token, id) = app_with_key("monitor").await;

    let first = app.delete(&format!("{INGEST_KEYS}/{id}")).await;
    assert_eq!(first.status(), StatusCode::NO_CONTENT);
    let first_revoked_at = revoked_at(&app, &id)
        .await
        .expect("existing row")
        .expect("revoked");

    let second = app.delete(&format!("{INGEST_KEYS}/{id}")).await;
    assert_eq!(second.status(), StatusCode::NO_CONTENT);
    let second_revoked_at = revoked_at(&app, &id)
        .await
        .expect("existing row")
        .expect("revoked");
    assert_eq!(
        first_revoked_at, second_revoked_at,
        "repeating the delete must not change revoked_at"
    );
}

/// Scenario: revocar un id inexistente también es 204.
#[tokio::test]
async fn delete_unknown_id_returns_204() {
    let app = TestApp::new_empty().await;

    let resp = app.delete(&format!("{INGEST_KEYS}/no-such-id")).await;

    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
}
