//! Tests de integración de las rutas `/api/push`.
//!
//! Cubre los requisitos 2, 3 y 4 de
//! `openspec/changes/webpush-notifications/specs/push/spec.md`: la clave pública
//! VAPID, la validación y registro de suscripciones y la baja idempotente.

mod common;
use common::TestApp;

use axum::http::StatusCode;

/// `p256dh` de contrato: 65 bytes `uncompressed` en base64url sin padding.
const VALID_P256DH: &str =
    "BLn9b-VR0ca83knDNZ32dCHGyjJp-1riX9ZTN40MqV8K_LpQmLqxC_DoHvqvFXO_nGdAB4W9dogZb_sM-uV4JbY";
/// Segundo `p256dh` válido (65 bytes), distinto del anterior.
const VALID_P256DH_2: &str =
    "BAcKDRATFhkcHyIlKCsuMTQ3Oj1AQ0ZJTE9SVVhbXmFkZ2ptcHN2eXx_goWIi46RlJeanaCjpqmsr7K1uLu-wcQ";
/// `p256dh` que decodifica a 64 bytes: longitud incorrecta.
const WRONG_P256DH: &str =
    "BAEDBQcJCw0PERMVFxkbHR8hIyUnKSstLzEzNTc5Oz0_QUNFR0lLTU9RU1VXWVtdX2FjZWdpa21vcXN1d3l7fQ";
/// `auth` de contrato: 16 bytes en base64url sin padding.
const VALID_AUTH: &str = "_ordMnz7uTCmrpBTeUV4Bw";
/// Segundo `auth` válido (16 bytes), distinto del anterior.
const VALID_AUTH_2: &str = "AwgNEhccISYrMDU6P0RJTg";
/// `auth` que decodifica a 15 bytes: longitud incorrecta.
const WRONG_AUTH: &str = "AwgNEhccISYrMDU6P0RJ";

const SUBSCRIPTIONS: &str = "/api/push/subscriptions";

async fn count_subscriptions(app: &TestApp) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM push_subscriptions")
        .fetch_one(&app.db)
        .await
        .expect("count query must succeed")
}

fn subscription(endpoint: &str, p256dh: &str, auth: &str) -> serde_json::Value {
    serde_json::json!({
        "endpoint": endpoint,
        "keys": { "p256dh": p256dh, "auth": auth }
    })
}

// ---------------------------------------------------------------------------
// Requisito 2 — GET /api/push/vapid-key
// ---------------------------------------------------------------------------

#[tokio::test]
async fn vapid_key_returns_200_with_non_empty_public_key() {
    // Given a backend with VAPID keys available (migration seeds them empty)
    // When GET /api/push/vapid-key is called
    // Then 200 and a non-empty public_key.
    let app = TestApp::new_empty().await;

    let resp = app.get("/api/push/vapid-key").await;

    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.json::<serde_json::Value>().await;
    let key = body["public_key"]
        .as_str()
        .expect("public_key must be a string");
    assert!(!key.is_empty(), "public_key must not be empty");
}

#[tokio::test]
async fn vapid_key_with_invalid_stored_key_returns_503_without_leaking_private() {
    // Given a backend whose stored VAPID private key is invalid
    let app = TestApp::new_empty().await;
    valet::db::repos::settings::SettingsRepo::set(&app.db, "push_vapid_public_key", "AQAB")
        .await
        .expect("seed public key");
    valet::db::repos::settings::SettingsRepo::set(
        &app.db,
        "push_vapid_private_key",
        "no-es-base64-valido",
    )
    .await
    .expect("seed private key");

    // When GET /api/push/vapid-key is called
    let resp = app.get("/api/push/vapid-key").await;

    // Then 503 with push_unavailable and no private key in the body.
    assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    let body = resp.json::<serde_json::Value>().await;
    assert_eq!(body["error"], "push_unavailable");
    let raw = body.to_string();
    assert!(
        !raw.contains("no-es-base64-valido"),
        "the private key must not leak: {raw}"
    );
}

// ---------------------------------------------------------------------------
// Requisito 3 — POST /api/push/subscriptions
// ---------------------------------------------------------------------------

#[tokio::test]
async fn subscribe_valid_creates_row_and_returns_201() {
    // Given a valid https subscription
    let app = TestApp::new_empty().await;

    // When POST /api/push/subscriptions is called
    let resp = app
        .post(SUBSCRIPTIONS)
        .json(&subscription(
            "https://push.example.invalid/abc",
            VALID_P256DH,
            VALID_AUTH,
        ))
        .send()
        .await;

    // Then 201, a UUIDv4 id and a new row.
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body = resp.json::<serde_json::Value>().await;
    let id = body["id"].as_str().expect("id must be a string");
    assert!(uuid::Uuid::parse_str(id).is_ok(), "id must be a UUID");
    assert_eq!(count_subscriptions(&app).await, 1);
}

#[tokio::test]
async fn subscribe_same_endpoint_preserves_id_and_single_row() {
    // Given a subscription already registered
    let app = TestApp::new_empty().await;
    let endpoint = "https://push.example.invalid/same";

    let first = app
        .post(SUBSCRIPTIONS)
        .json(&subscription(endpoint, VALID_P256DH, VALID_AUTH))
        .send()
        .await;
    assert_eq!(first.status(), StatusCode::CREATED);
    let first_id = first.json::<serde_json::Value>().await["id"]
        .as_str()
        .expect("id")
        .to_string();

    // When the same endpoint is posted again with new keys
    let second = app
        .post(SUBSCRIPTIONS)
        .json(&subscription(endpoint, VALID_P256DH_2, VALID_AUTH_2))
        .send()
        .await;

    // Then the same id comes back and there is still a single row.
    assert_eq!(second.status(), StatusCode::CREATED);
    let second_id = second.json::<serde_json::Value>().await["id"]
        .as_str()
        .expect("id")
        .to_string();
    assert_eq!(first_id, second_id, "the upsert must preserve the id");
    assert_eq!(count_subscriptions(&app).await, 1);

    let stored: String =
        sqlx::query_scalar("SELECT p256dh FROM push_subscriptions WHERE endpoint = ?1")
            .bind(endpoint)
            .fetch_one(&app.db)
            .await
            .expect("row must exist");
    assert_eq!(stored, VALID_P256DH_2, "p256dh must be updated");
}

#[tokio::test]
async fn subscribe_non_https_endpoint_returns_400_and_no_row() {
    // Given an endpoint that is not an absolute https URL
    let app = TestApp::new_empty().await;

    // When POST /api/push/subscriptions is called
    let resp = app
        .post(SUBSCRIPTIONS)
        .json(&subscription(
            "http://push.example.invalid/abc",
            VALID_P256DH,
            VALID_AUTH,
        ))
        .send()
        .await;

    // Then 400, an error body and no row inserted.
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body = resp.json::<serde_json::Value>().await;
    assert!(body["error"].is_string(), "the body must carry an error");
    assert_eq!(count_subscriptions(&app).await, 0);
}

#[tokio::test]
async fn subscribe_relative_endpoint_returns_400() {
    // Given a relative endpoint (not an absolute URL)
    let app = TestApp::new_empty().await;

    let resp = app
        .post(SUBSCRIPTIONS)
        .json(&subscription("not-a-url", VALID_P256DH, VALID_AUTH))
        .send()
        .await;

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body = resp.json::<serde_json::Value>().await;
    assert!(body["error"].is_string());
}

#[tokio::test]
async fn subscribe_wrong_p256dh_length_returns_400() {
    // Given a p256dh that decodes to a length other than 65 bytes
    let app = TestApp::new_empty().await;

    let resp = app
        .post(SUBSCRIPTIONS)
        .json(&subscription(
            "https://push.example.invalid/abc",
            WRONG_P256DH,
            VALID_AUTH,
        ))
        .send()
        .await;

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body = resp.json::<serde_json::Value>().await;
    assert!(body["error"].is_string());
    assert_eq!(count_subscriptions(&app).await, 0);
}

#[tokio::test]
async fn subscribe_wrong_auth_length_returns_400() {
    // Given an auth that decodes to a length other than 16 bytes
    let app = TestApp::new_empty().await;

    let resp = app
        .post(SUBSCRIPTIONS)
        .json(&subscription(
            "https://push.example.invalid/abc",
            VALID_P256DH,
            WRONG_AUTH,
        ))
        .send()
        .await;

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body = resp.json::<serde_json::Value>().await;
    assert!(body["error"].is_string());
    assert_eq!(count_subscriptions(&app).await, 0);
}

// ---------------------------------------------------------------------------
// Requisito 4 — DELETE /api/push/subscriptions
// ---------------------------------------------------------------------------

#[tokio::test]
async fn unsubscribe_deletes_row_and_returns_204() {
    // Given a registered subscription
    let app = TestApp::new_empty().await;
    let endpoint = "https://push.example.invalid/gone";
    let created = app
        .post(SUBSCRIPTIONS)
        .json(&subscription(endpoint, VALID_P256DH, VALID_AUTH))
        .send()
        .await;
    assert_eq!(created.status(), StatusCode::CREATED);

    // When DELETE /api/push/subscriptions is called with that endpoint
    let resp = app
        .delete_with_body(SUBSCRIPTIONS)
        .json(&serde_json::json!({ "endpoint": endpoint }))
        .send()
        .await;

    // Then 204 and the row is gone.
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    assert_eq!(count_subscriptions(&app).await, 0);
}

#[tokio::test]
async fn unsubscribe_unknown_endpoint_returns_204() {
    // Given an endpoint that matches no row
    let app = TestApp::new_empty().await;

    // When DELETE /api/push/subscriptions is called
    let resp = app
        .delete_with_body(SUBSCRIPTIONS)
        .json(&serde_json::json!({ "endpoint": "https://push.example.invalid/unknown" }))
        .send()
        .await;

    // Then 204, no error.
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn unsubscribe_missing_endpoint_returns_400() {
    // Given a request without endpoint in the body
    let app = TestApp::new_empty().await;

    // When DELETE /api/push/subscriptions is called
    let resp = app
        .delete_with_body(SUBSCRIPTIONS)
        .json(&serde_json::json!({}))
        .send()
        .await;

    // Then 400.
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}
