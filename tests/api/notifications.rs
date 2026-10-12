//! Tests de integración de las rutas `/api/notifications` y del middleware de
//! ingesta.
//!
//! Cubre los requisitos de `openspec/changes/notification-ingest/specs/
//! notifications/spec.md`: la autenticación por token de ingesta, la validación
//! y normalización del cuerpo, la persistencia antes de responder, el listado de
//! no leídas y la marca idempotente. La exención de sesión por par (método,
//! ruta) se prueba en `tests/api/auth.rs`.

mod common;
use common::TestApp;

use axum::http::StatusCode;
use serde_json::{json, Value};

use valet::db::repos::ingest_keys::{generate_token, IngestRepo};
use valet::db::repos::notifications::NotificationsRepo;

const NOTIFICATIONS: &str = "/api/notifications";
const UNREAD: &str = "/api/notifications/unread";
const MARK_READ: &str = "/api/notifications/mark-read";

/// Siembra una clave de ingesta con un token conocido y la llama `name`.
async fn app_with_key(name: &str) -> (TestApp, String) {
    let app = TestApp::new_empty().await;
    let token = generate_token();
    IngestRepo::create(&app.db, name, &token)
        .await
        .expect("seed ingest key");
    (app, token)
}

async fn count_notifications(app: &TestApp) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM notifications")
        .fetch_one(&app.db)
        .await
        .expect("count query must succeed")
}

async fn stored_row(app: &TestApp, id: &str) -> (String, String, String, String, Option<String>) {
    sqlx::query_as(
        "SELECT origin, author, message, level, metadata FROM notifications WHERE id = ?1",
    )
    .bind(id)
    .fetch_one(&app.db)
    .await
    .expect("the notification row must exist")
}

// ---------------------------------------------------------------------------
// 4.1.1 — Middleware `require_ingest_key`
// ---------------------------------------------------------------------------

/// Scenario: sin cabecera de autorización no se ingesta.
#[tokio::test]
async fn ingest_without_authorization_returns_401_and_no_row() {
    let (app, _token) = app_with_key("monitor").await;

    let resp = app
        .post(NOTIFICATIONS)
        .json(&json!({ "message": "hola" }))
        .send()
        .await;

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        resp.headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok()),
        Some("application/json"),
        "the 401 must carry a JSON content type"
    );
    assert_eq!(
        resp.text().await,
        r#"{"error":"unauthorized"}"#,
        "the 401 body must be the unauthorized error"
    );
    assert_eq!(
        count_notifications(&app).await,
        0,
        "no row must be inserted"
    );
}

/// Scenario: un token malformado no ingesta (falta el esquema `Bearer` o el
/// valor es arbitrario).
#[tokio::test]
async fn ingest_with_malformed_authorization_returns_401() {
    let (app, token) = app_with_key("monitor").await;

    for header in [
        token.clone(),
        format!("Basic {token}"),
        "Bearer".to_string(),
    ] {
        let resp = app
            .post(NOTIFICATIONS)
            .header("authorization", &header)
            .json(&json!({ "message": "hola" }))
            .send()
            .await;
        assert_eq!(
            resp.status(),
            StatusCode::UNAUTHORIZED,
            "a malformed Authorization header must be rejected: {header:?}"
        );
        assert_eq!(
            resp.headers()
                .get("content-type")
                .and_then(|v| v.to_str().ok()),
            Some("application/json"),
            "the 401 must carry a JSON content type: {header:?}"
        );
        assert_eq!(
            resp.text().await,
            r#"{"error":"unauthorized"}"#,
            "the 401 body must be the unauthorized error: {header:?}"
        );
    }

    assert_eq!(
        count_notifications(&app).await,
        0,
        "no row must be inserted"
    );
}

/// Scenario: un token desconocido no ingesta.
#[tokio::test]
async fn ingest_with_unknown_token_returns_401() {
    let (app, _token) = app_with_key("monitor").await;
    let unknown = generate_token();

    let resp = app
        .post(NOTIFICATIONS)
        .header("authorization", &format!("Bearer {unknown}"))
        .json(&json!({ "message": "hola" }))
        .send()
        .await;

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        resp.headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok()),
        Some("application/json"),
        "the 401 must carry a JSON content type"
    );
    assert_eq!(
        resp.text().await,
        r#"{"error":"unauthorized"}"#,
        "the 401 body must be the unauthorized error"
    );
    assert_eq!(
        count_notifications(&app).await,
        0,
        "no row must be inserted"
    );
}

/// Scenario: una clave revocada no ingesta.
#[tokio::test]
async fn ingest_with_revoked_key_returns_401() {
    let app = TestApp::new_empty().await;
    let token = generate_token();
    let key = IngestRepo::create(&app.db, "monitor", &token)
        .await
        .expect("seed ingest key");
    IngestRepo::revoke(&app.db, &key.id).await.expect("revoke");

    let resp = app
        .post(NOTIFICATIONS)
        .header("authorization", &format!("Bearer {token}"))
        .json(&json!({ "message": "hola" }))
        .send()
        .await;

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        resp.headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok()),
        Some("application/json"),
        "the 401 must carry a JSON content type"
    );
    assert_eq!(
        resp.text().await,
        r#"{"error":"unauthorized"}"#,
        "the 401 body must be the unauthorized error"
    );
    assert_eq!(
        count_notifications(&app).await,
        0,
        "no row must be inserted"
    );
}

/// Scenario: una clave revocada y un token desconocido son indistinguibles.
///
/// El cliente no debe poder deducir, por la respuesta, si un token existió y
/// fue revocado o si nunca existió: ambas respuestas deben coincidir en estado
/// y en cuerpo.
#[tokio::test]
async fn ingest_revoked_and_unknown_token_are_indistinguishable_401() {
    let app = TestApp::new_empty().await;
    let revoked_token = generate_token();
    let key = IngestRepo::create(&app.db, "monitor", &revoked_token)
        .await
        .expect("seed ingest key");
    IngestRepo::revoke(&app.db, &key.id).await.expect("revoke");

    let unknown_token = generate_token();

    let revoked = app
        .post(NOTIFICATIONS)
        .header("authorization", &format!("Bearer {revoked_token}"))
        .json(&json!({ "message": "hola" }))
        .send()
        .await;
    let unknown = app
        .post(NOTIFICATIONS)
        .header("authorization", &format!("Bearer {unknown_token}"))
        .json(&json!({ "message": "hola" }))
        .send()
        .await;

    assert_eq!(
        revoked.status(),
        StatusCode::UNAUTHORIZED,
        "a revoked key must be rejected"
    );
    assert_eq!(
        unknown.status(),
        StatusCode::UNAUTHORIZED,
        "an unknown token must be rejected"
    );
    assert_eq!(
        revoked.status(),
        unknown.status(),
        "revoked and unknown tokens must yield the same status"
    );

    let revoked_body = revoked.text().await;
    let unknown_body = unknown.text().await;
    assert_eq!(
        revoked_body, r#"{"error":"unauthorized"}"#,
        "the revoked-key 401 body must be the unauthorized error"
    );
    assert_eq!(
        unknown_body, r#"{"error":"unauthorized"}"#,
        "the unknown-token 401 body must be the unauthorized error"
    );
    assert_eq!(
        revoked_body, unknown_body,
        "revoked and unknown tokens must yield the same body"
    );
}

/// Scenario: un token válido actualiza `last_used_at` y pasa.
#[tokio::test]
async fn ingest_with_valid_token_reaches_handler_and_updates_last_used() {
    let (app, token) = app_with_key("monitor").await;

    let resp = app
        .post(NOTIFICATIONS)
        .header("authorization", &format!("Bearer {token}"))
        .json(&json!({ "message": "hola" }))
        .send()
        .await;

    assert_eq!(resp.status(), StatusCode::CREATED);

    let key = IngestRepo::list_all(&app.db)
        .await
        .expect("list keys")
        .into_iter()
        .next()
        .expect("the seeded key must exist");
    assert!(
        key.last_used_at.is_some(),
        "a successful ingest must set last_used_at"
    );

    // El repo no guarda el token: ninguna columna de la clave lo contiene.
    for value in [
        &key.id,
        &key.name,
        &key.key_hash,
        &key.prefix,
        &key.created_at,
    ] {
        assert!(!value.contains(&token), "a column leaked the token");
    }
}

// ---------------------------------------------------------------------------
// 5.1 — POST /api/notifications
// ---------------------------------------------------------------------------

/// Scenario: una clave válida ingesta con 201 y devuelve `{"id": "<uuid>"}`.
#[tokio::test]
async fn ingest_valid_returns_201_with_uuid() {
    let (app, token) = app_with_key("monitor").await;

    let resp = app
        .post(NOTIFICATIONS)
        .header("authorization", &format!("Bearer {token}"))
        .json(&json!({ "message": "hola" }))
        .send()
        .await;

    assert_eq!(resp.status(), StatusCode::CREATED);
    let body = resp.json::<Value>().await;
    let id = body["id"].as_str().expect("id must be a string");
    assert!(uuid::Uuid::parse_str(id).is_ok(), "id must be a UUID");
    assert_eq!(count_notifications(&app).await, 1);
}

/// Scenario: un mensaje ausente o en blanco se rechaza con `400` y `{"error"}`.
#[tokio::test]
async fn ingest_missing_or_blank_message_returns_400() {
    let (app, token) = app_with_key("monitor").await;

    let absent = app
        .post(NOTIFICATIONS)
        .header("authorization", &format!("Bearer {token}"))
        .json(&json!({ "origin": "x" }))
        .send()
        .await;
    assert_eq!(absent.status(), StatusCode::BAD_REQUEST);
    let body = absent.json::<Value>().await;
    assert!(body["error"].is_string(), "the body must carry an error");

    let blank = app
        .post(NOTIFICATIONS)
        .header("authorization", &format!("Bearer {token}"))
        .json(&json!({ "message": "   " }))
        .send()
        .await;
    assert_eq!(blank.status(), StatusCode::BAD_REQUEST);
    let body = blank.json::<Value>().await;
    assert!(body["error"].is_string(), "the body must carry an error");

    assert_eq!(count_notifications(&app).await, 0);
}

/// Scenario: `origin` y `author` ausentes toman el nombre de la clave.
#[tokio::test]
async fn ingest_origin_and_author_default_to_key_name() {
    let (app, token) = app_with_key("monitor").await;

    let resp = app
        .post(NOTIFICATIONS)
        .header("authorization", &format!("Bearer {token}"))
        .json(&json!({ "message": "hola" }))
        .send()
        .await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let id = resp.json::<Value>().await["id"]
        .as_str()
        .expect("id")
        .to_string();

    let (origin, author, message, level, _) = stored_row(&app, &id).await;
    assert_eq!(origin, "monitor", "origin defaults to the key name");
    assert_eq!(author, "monitor", "author defaults to the key name");
    assert_eq!(message, "hola");
    assert_eq!(level, "info", "level defaults to info");

    // Un `origin`/`author` explícitos se conservan.
    let resp = app
        .post(NOTIFICATIONS)
        .header("authorization", &format!("Bearer {token}"))
        .json(&json!({ "message": "hola", "origin": "backup", "author": "cron" }))
        .send()
        .await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let id = resp.json::<Value>().await["id"]
        .as_str()
        .expect("id")
        .to_string();
    let (origin, author, _, _, _) = stored_row(&app, &id).await;
    assert_eq!(origin, "backup");
    assert_eq!(author, "cron");
}

/// Scenario: se recorta el mensaje antes de persistir.
#[tokio::test]
async fn ingest_trims_message_before_persisting() {
    let (app, token) = app_with_key("monitor").await;

    let resp = app
        .post(NOTIFICATIONS)
        .header("authorization", &format!("Bearer {token}"))
        .json(&json!({ "message": "  hola mundo  " }))
        .send()
        .await;
    assert_eq!(resp.status(), StatusCode::CREATED);
    let id = resp.json::<Value>().await["id"]
        .as_str()
        .expect("id")
        .to_string();

    let (_, _, message, _, _) = stored_row(&app, &id).await;
    assert_eq!(message, "hola mundo", "the message must be trimmed");
}

/// Scenario: un `level` fuera de la lista blanca se rechaza; `critical` pasa.
#[tokio::test]
async fn ingest_level_whitelist() {
    let (app, token) = app_with_key("monitor").await;

    let rejected = app
        .post(NOTIFICATIONS)
        .header("authorization", &format!("Bearer {token}"))
        .json(&json!({ "message": "hola", "level": "debug" }))
        .send()
        .await;
    assert_eq!(rejected.status(), StatusCode::BAD_REQUEST);

    let accepted = app
        .post(NOTIFICATIONS)
        .header("authorization", &format!("Bearer {token}"))
        .json(&json!({ "message": "hola", "level": "critical" }))
        .send()
        .await;
    assert_eq!(accepted.status(), StatusCode::CREATED);
    let id = accepted.json::<Value>().await["id"]
        .as_str()
        .expect("id")
        .to_string();
    let (_, _, _, level, _) = stored_row(&app, &id).await;
    assert_eq!(level, "critical");
}

/// Scenario: `metadata` debe ser un objeto JSON (arreglo → 400, objeto → 201).
#[tokio::test]
async fn ingest_metadata_must_be_object() {
    let (app, token) = app_with_key("monitor").await;

    let array = app
        .post(NOTIFICATIONS)
        .header("authorization", &format!("Bearer {token}"))
        .json(&json!({ "message": "hola", "metadata": [1, 2, 3] }))
        .send()
        .await;
    assert_eq!(array.status(), StatusCode::BAD_REQUEST);

    let object = app
        .post(NOTIFICATIONS)
        .header("authorization", &format!("Bearer {token}"))
        .json(&json!({ "message": "hola", "metadata": { "source": "cron" } }))
        .send()
        .await;
    assert_eq!(object.status(), StatusCode::CREATED);
    let id = object.json::<Value>().await["id"]
        .as_str()
        .expect("id")
        .to_string();
    let (_, _, _, _, metadata) = stored_row(&app, &id).await;
    let metadata = metadata.expect("metadata must be stored");
    let parsed: Value = serde_json::from_str(&metadata).expect("metadata must be JSON");
    assert_eq!(parsed["source"], "cron");
}

/// Scenario: se rechazan los cuerpos que exceden los límites (mensaje > 4096 y
/// cuerpo HTTP > 64 KiB).
#[tokio::test]
async fn ingest_rejects_oversized_bodies() {
    let (app, token) = app_with_key("monitor").await;

    // `message` de más de 4096 bytes, cuerpo por debajo de 64 KiB → 400.
    let long_message = "a".repeat(4097);
    let resp = app
        .post(NOTIFICATIONS)
        .header("authorization", &format!("Bearer {token}"))
        .json(&json!({ "message": long_message }))
        .send()
        .await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    // Cuerpo HTTP de más de 64 KiB → 413.
    let huge = "a".repeat(70_000);
    let resp = app
        .post(NOTIFICATIONS)
        .header("authorization", &format!("Bearer {token}"))
        .json(&json!({ "message": huge }))
        .send()
        .await;
    assert_eq!(resp.status(), StatusCode::PAYLOAD_TOO_LARGE);

    assert_eq!(count_notifications(&app).await, 0);
}

/// `metadata` cuya serialización supera 8192 bytes se rechaza con `400`.
#[tokio::test]
async fn ingest_rejects_metadata_over_the_size_limit() {
    let (app, token) = app_with_key("monitor").await;

    let big = "b".repeat(9000);
    let resp = app
        .post(NOTIFICATIONS)
        .header("authorization", &format!("Bearer {token}"))
        .json(&json!({ "message": "hola", "metadata": { "blob": big } }))
        .send()
        .await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

/// `origin` de más de 64 bytes o `author` de más de 128 bytes se rechazan.
#[tokio::test]
async fn ingest_rejects_oversized_origin_and_author() {
    let (app, token) = app_with_key("monitor").await;

    let long_origin = "o".repeat(65);
    let resp = app
        .post(NOTIFICATIONS)
        .header("authorization", &format!("Bearer {token}"))
        .json(&json!({ "message": "hola", "origin": long_origin }))
        .send()
        .await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    let long_author = "a".repeat(129);
    let resp = app
        .post(NOTIFICATIONS)
        .header("authorization", &format!("Bearer {token}"))
        .json(&json!({ "message": "hola", "author": long_author }))
        .send()
        .await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

// ---------------------------------------------------------------------------
// 5.1 — GET /api/notifications/unread
// ---------------------------------------------------------------------------

async fn insert_notification(app: &TestApp, message: &str, created_at: &str) -> String {
    let id = NotificationsRepo::insert(&app.db, "monitor", "cron", message, "info", None)
        .await
        .expect("insert notification");
    sqlx::query("UPDATE notifications SET created_at = ?1 WHERE id = ?2")
        .bind(created_at)
        .bind(&id)
        .execute(&app.db)
        .await
        .expect("pin created_at");
    id
}

/// Scenario: solo se listan las no leídas, más recientes primero, con
/// `PaginatedResponse`; `metadata` parseado a objeto o `null`.
#[tokio::test]
async fn unread_lists_only_unread_newest_first() {
    let app = TestApp::new_empty().await;

    let oldest = insert_notification(&app, "vieja", "2020-01-01 00:00:01").await;
    let _middle = insert_notification(&app, "media", "2020-01-01 00:00:02").await;
    let newest = insert_notification(&app, "nueva", "2020-01-01 00:00:03").await;
    NotificationsRepo::mark_read(&app.db, std::slice::from_ref(&oldest))
        .await
        .expect("mark read");

    // Una notificación con metadata objeto.
    let with_meta = NotificationsRepo::insert(
        &app.db,
        "monitor",
        "cron",
        "con meta",
        "info",
        Some(r#"{"k":"v"}"#),
    )
    .await
    .expect("insert");
    sqlx::query("UPDATE notifications SET created_at = '2020-01-01 00:00:04' WHERE id = ?1")
        .bind(&with_meta)
        .execute(&app.db)
        .await
        .expect("pin created_at");

    let resp = app.get(UNREAD).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.json::<Value>().await;

    assert_eq!(body["total"], 3, "three unread rows");
    assert!(body["next_cursor"].is_null(), "no cursor is issued");

    let data = body["data"].as_array().expect("data must be an array");
    assert_eq!(data.len(), 3);
    assert_eq!(data[0]["id"], with_meta, "newest unread comes first");
    assert_eq!(data[1]["id"], newest);
    assert!(
        !data.iter().any(|n| n["id"] == oldest),
        "a read notification must not be listed"
    );

    // `metadata` llega parseado a objeto.
    assert_eq!(data[0]["metadata"]["k"], "v");
    // `metadata` NULL llega como `null`.
    assert!(data[1]["metadata"].is_null());

    // Cada item expone los campos exigidos.
    for item in data {
        for field in ["id", "origin", "author", "message", "level", "created_at"] {
            assert!(
                item.get(field).is_some(),
                "the item must expose `{field}`: {item}"
            );
        }
    }
}

/// Scenario: `limit` por defecto 50 y `limit` inválido (`0`, `201`, `abc`) → 400.
#[tokio::test]
async fn unread_limit_defaults_and_rejects_invalid() {
    let app = TestApp::new_empty().await;
    for i in 0..51 {
        NotificationsRepo::insert(&app.db, "monitor", "cron", &format!("n{i}"), "info", None)
            .await
            .expect("insert");
    }

    let body = app.get(UNREAD).await.json::<Value>().await;
    assert_eq!(
        body["data"].as_array().expect("data").len(),
        50,
        "the default limit is 50"
    );
    assert_eq!(body["total"], 51, "total counts every unread row");

    for query in ["?limit=0", "?limit=201", "?limit=abc"] {
        let resp = app.get(&format!("{UNREAD}{query}")).await;
        assert_eq!(
            resp.status(),
            StatusCode::BAD_REQUEST,
            "an invalid limit must be rejected: {query}"
        );
        let body = resp.json::<Value>().await;
        assert!(body["error"].is_string(), "the body must carry an error");
    }

    let body = app
        .get(&format!("{UNREAD}?limit=1"))
        .await
        .json::<Value>()
        .await;
    assert_eq!(body["data"].as_array().expect("data").len(), 1);
}

// ---------------------------------------------------------------------------
// 5.1 — POST /api/notifications/mark-read
// ---------------------------------------------------------------------------

/// Scenario: marcar dos no leídas responde `{"marked": 2}`; repetir es
/// idempotente; un id inexistente se ignora.
#[tokio::test]
async fn mark_read_marks_and_is_idempotent() {
    let app = TestApp::new_empty().await;
    let a = insert_notification(&app, "a", "2020-01-01 00:00:01").await;
    let b = insert_notification(&app, "b", "2020-01-01 00:00:02").await;

    let first = app
        .post(MARK_READ)
        .json(&json!({ "ids": [a, b] }))
        .send()
        .await;
    assert_eq!(first.status(), StatusCode::OK);
    assert_eq!(first.json::<Value>().await["marked"], 2);

    let again = app
        .post(MARK_READ)
        .json(&json!({ "ids": [a, b] }))
        .send()
        .await;
    assert_eq!(again.status(), StatusCode::OK);
    assert_eq!(again.json::<Value>().await["marked"], 0);

    let missing = app
        .post(MARK_READ)
        .json(&json!({ "ids": ["no-such-id"] }))
        .send()
        .await;
    assert_eq!(missing.status(), StatusCode::OK);
    assert_eq!(missing.json::<Value>().await["marked"], 0);
}

/// Scenario: una lista vacía o ausente se rechaza con `400`.
#[tokio::test]
async fn mark_read_empty_or_missing_ids_returns_400() {
    let app = TestApp::new_empty().await;

    for body in [json!({}), json!({ "ids": [] })] {
        let resp = app.post(MARK_READ).json(&body).send().await;
        assert_eq!(
            resp.status(),
            StatusCode::BAD_REQUEST,
            "a missing or empty list must be rejected: {body}"
        );
    }
}
