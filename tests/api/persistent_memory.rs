//! Integration tests for the persistent-memory HTTP surface (Bloque 2 of the
//! `persistent-memory-ui` change).
//!
//! Each test exercises one requirement of the change: HTTP read, validated
//! write with budget/ceiling, optimistic concurrency and idempotent clearing.

mod common;
use common::TestApp;
use serde_json::json;
use valet::persistent_memory::payload_token_count;

/// A minimal valid version-1 state that fits comfortably in the default budget.
fn small_state() -> serde_json::Value {
    json!({"schema_version": 1, "user_profile": {"city": "Madrid"}})
}

/// Override `settings.PERSISTENT_MEMORY_BUDGET_TOKENS` through the settings API.
async fn set_budget(app: &TestApp, value: usize) {
    let resp = app
        .put("/api/settings")
        .json(&json!({"PERSISTENT_MEMORY_BUDGET_TOKENS": value.to_string()}))
        .send()
        .await;
    assert_eq!(resp.status(), 200, "setting the budget must succeed");
}

// ─── Requirement: la lectura del estado persistente SHALL exponerse por HTTP ─

/// Estado ausente se lee como vacío y la lectura no crea la fila.
#[tokio::test]
async fn get_empty_state_is_read_as_empty_without_creating_row() {
    let app = TestApp::new().await;

    let resp = app.get("/api/persistent-memory").await;
    assert_eq!(resp.status(), 200);
    let body = resp.json::<serde_json::Value>().await;
    assert_eq!(body["payload"], serde_json::Value::Null);
    assert_eq!(body["updated_at"], serde_json::Value::Null);
    assert_eq!(body["token_count"], 0);
    assert_eq!(body["is_empty"], true);

    // If GET had created a row, this PUT expecting `null` (no row) would be a
    // 409 Conflict. It succeeds, proving the row was never created.
    let resp = app
        .put("/api/persistent-memory")
        .json(&json!({"payload": small_state(), "expected_updated_at": null}))
        .send()
        .await;
    assert_eq!(
        resp.status(),
        200,
        "GET must not have created the persistent_memory row"
    );
}

/// Estado existente se devuelve con su payload, marca y cotas.
#[tokio::test]
async fn get_state_returns_payload_mark_and_bounds() {
    let app = TestApp::new().await;
    set_budget(&app, 1234).await;

    let state = small_state();
    let put = app
        .put("/api/persistent-memory")
        .json(&json!({"payload": state}))
        .send()
        .await;
    assert_eq!(put.status(), 200);
    let updated_at = put.json::<serde_json::Value>().await["updated_at"]
        .as_str()
        .expect("updated_at is a string")
        .to_string();

    let resp = app.get("/api/persistent-memory").await;
    assert_eq!(resp.status(), 200);
    let body = resp.json::<serde_json::Value>().await;
    assert_eq!(body["payload"], state);
    assert_eq!(body["updated_at"], updated_at);
    assert_eq!(body["budget_tokens"], 1234);
    assert_eq!(body["ceiling_tokens"], 2468);
    assert_eq!(body["is_empty"], false);
    // Contrast against the low-level estimator over the minified state, so the
    // assertion is not tautological through `payload_token_count`.
    let expected_tokens = valet::token_estimate::estimate_json_tokens(
        &valet::persistent_memory::minified_json(&state),
    );
    assert_eq!(body["token_count"], expected_tokens);
}

/// Una fila almacenada cuyo JSON no parsea es una violación de invariante: la
/// lectura responde 500 y nunca la trata como estado vacío.
#[tokio::test]
async fn get_corrupt_stored_payload_returns_500() {
    let app = TestApp::new().await;

    // Seed a raw row that no endpoint would ever produce.
    sqlx::query(
        "INSERT INTO persistent_memory (id, payload, updated_at) \
         VALUES ('global_state', 'not valid json', '2026-10-03T00:00:00Z')",
    )
    .execute(&app.db)
    .await
    .expect("seed a corrupt row");

    let resp = app.get("/api/persistent-memory").await;
    assert_eq!(resp.status(), 500);
}

// ─── Requirement: la escritura manual SHALL validar el esquema y respetar el techo

/// Escritura válida dentro del presupuesto: 200 sin aviso y estado almacenado.
#[tokio::test]
async fn valid_write_within_budget_has_no_warning() {
    let app = TestApp::new().await;
    let state = small_state();

    let resp = app
        .put("/api/persistent-memory")
        .json(&json!({"payload": state}))
        .send()
        .await;
    assert_eq!(resp.status(), 200);
    let body = resp.json::<serde_json::Value>().await;
    assert_eq!(body["payload"], state);
    assert_eq!(body["warning"], serde_json::Value::Null);

    let get = app
        .get("/api/persistent-memory")
        .await
        .json::<serde_json::Value>()
        .await;
    assert_eq!(get["payload"], state);
}

/// Payload inválido no se escribe y el estado anterior permanece intacto.
#[tokio::test]
async fn invalid_schema_is_rejected_without_writing() {
    let app = TestApp::new().await;
    let previous = small_state();
    let seeded = app
        .put("/api/persistent-memory")
        .json(&json!({"payload": previous}))
        .send()
        .await;
    assert_eq!(seeded.status(), 200);

    let resp = app
        .put("/api/persistent-memory")
        .json(&json!({"payload": {"schema_version": 2, "user_profile": {}}}))
        .send()
        .await;
    assert_eq!(resp.status(), 422);

    let get = app
        .get("/api/persistent-memory")
        .await
        .json::<serde_json::Value>()
        .await;
    assert_eq!(get["payload"], previous, "previous state must be intact");
}

/// Por encima del presupuesto pero bajo el techo: se guarda con aviso.
#[tokio::test]
async fn over_budget_stores_with_warning() {
    let app = TestApp::new().await;
    let state = json!({"schema_version": 1, "user_profile": {"note": "x".repeat(50)}});
    let tokens = payload_token_count(&state);
    assert!(tokens > 1, "precondition: measurable state");
    set_budget(&app, tokens - 1).await;

    let resp = app
        .put("/api/persistent-memory")
        .json(&json!({"payload": state}))
        .send()
        .await;
    assert_eq!(resp.status(), 200);
    let body = resp.json::<serde_json::Value>().await;
    assert!(
        body["warning"].as_str().is_some_and(|w| !w.is_empty()),
        "an over-budget write must warn"
    );

    let get = app
        .get("/api/persistent-memory")
        .await
        .json::<serde_json::Value>()
        .await;
    assert_eq!(get["payload"], state, "the state must still be stored");
}

/// Por encima del techo absoluto: se rechaza con conteo, presupuesto y techo,
/// conservando el estado anterior.
#[tokio::test]
async fn over_ceiling_is_rejected_and_previous_kept() {
    let app = TestApp::new().await;
    let previous = small_state();
    let seeded = app
        .put("/api/persistent-memory")
        .json(&json!({"payload": previous}))
        .send()
        .await;
    assert_eq!(seeded.status(), 200);

    let big = json!({"schema_version": 1, "user_profile": {"note": "x".repeat(200)}});
    let big_tokens = payload_token_count(&big);
    let budget = big_tokens / 4;
    assert!(big_tokens > budget * 2, "precondition: over the ceiling");
    set_budget(&app, budget).await;

    let resp = app
        .put("/api/persistent-memory")
        .json(&json!({"payload": big}))
        .send()
        .await;
    assert_eq!(resp.status(), 422);
    let error = resp.json::<serde_json::Value>().await;
    let message = error["error"].as_str().expect("error message");
    assert!(
        message.contains(&big_tokens.to_string()),
        "message: {message}"
    );
    assert!(message.contains(&budget.to_string()), "message: {message}");
    assert!(
        message.contains(&(budget * 2).to_string()),
        "message: {message}"
    );

    let get = app
        .get("/api/persistent-memory")
        .await
        .json::<serde_json::Value>()
        .await;
    assert_eq!(get["payload"], previous, "previous state must be intact");
}

// ─── Requirement: la escritura manual SHALL detectar cambios concurrentes ─────

/// La marca obsoleta provoca 409 y el estado vigente permanece intacto.
#[tokio::test]
async fn stale_expected_mark_conflicts_without_writing() {
    let app = TestApp::new().await;
    let state = small_state();
    let seeded = app
        .put("/api/persistent-memory")
        .json(&json!({"payload": state}))
        .send()
        .await;
    assert_eq!(seeded.status(), 200);
    let current = seeded.json::<serde_json::Value>().await["updated_at"]
        .as_str()
        .expect("updated_at")
        .to_string();

    let resp = app
        .put("/api/persistent-memory")
        .json(&json!({
            "payload": {"schema_version": 1, "user_profile": {"city": "Barcelona"}},
            "expected_updated_at": "1999-01-01T00:00:00Z"
        }))
        .send()
        .await;
    assert_eq!(resp.status(), 409);

    let get = app
        .get("/api/persistent-memory")
        .await
        .json::<serde_json::Value>()
        .await;
    assert_eq!(get["payload"], state, "state must be intact");
    assert_eq!(get["updated_at"], current, "mark must be intact");
}

/// Un `null` explícito esperado contra una fila existente también es conflicto.
#[tokio::test]
async fn explicit_null_expected_conflicts_with_existing_row() {
    let app = TestApp::new().await;
    let seeded = app
        .put("/api/persistent-memory")
        .json(&json!({"payload": small_state()}))
        .send()
        .await;
    assert_eq!(seeded.status(), 200);

    let resp = app
        .put("/api/persistent-memory")
        .json(&json!({
            "payload": {"schema_version": 1, "user_profile": {"city": "Barcelona"}},
            "expected_updated_at": null
        }))
        .send()
        .await;
    assert_eq!(resp.status(), 409);
}

/// El caso simétrico: una marca string esperada contra una tabla vacía también
/// es conflicto (la spec exige la diferencia «sin fila» vs «con fila» en ambos
/// sentidos), y nada se escribe.
#[tokio::test]
async fn expected_string_mark_against_empty_table_conflicts() {
    let app = TestApp::new().await;

    let resp = app
        .put("/api/persistent-memory")
        .json(&json!({
            "payload": small_state(),
            "expected_updated_at": "2026-10-03T00:00:00Z"
        }))
        .send()
        .await;
    assert_eq!(resp.status(), 409);

    let get = app
        .get("/api/persistent-memory")
        .await
        .json::<serde_json::Value>()
        .await;
    assert_eq!(get["is_empty"], true, "nothing must have been written");
    assert_eq!(get["payload"], serde_json::Value::Null);
}

/// La marca vigente permite la escritura.
#[tokio::test]
async fn current_expected_mark_allows_write() {
    let app = TestApp::new().await;
    let seeded = app
        .put("/api/persistent-memory")
        .json(&json!({"payload": small_state()}))
        .send()
        .await;
    assert_eq!(seeded.status(), 200);
    let current = seeded.json::<serde_json::Value>().await["updated_at"]
        .as_str()
        .expect("updated_at")
        .to_string();

    let next = json!({"schema_version": 1, "user_profile": {"city": "Barcelona"}});
    let resp = app
        .put("/api/persistent-memory")
        .json(&json!({"payload": next, "expected_updated_at": current}))
        .send()
        .await;
    assert_eq!(resp.status(), 200);
    let body = resp.json::<serde_json::Value>().await;
    assert_eq!(body["payload"], next);
}

/// Sin `expected_updated_at` la escritura procede con independencia de la marca.
#[tokio::test]
async fn omitted_expected_mark_proceeds() {
    let app = TestApp::new().await;
    let seeded = app
        .put("/api/persistent-memory")
        .json(&json!({"payload": small_state()}))
        .send()
        .await;
    assert_eq!(seeded.status(), 200);

    let next = json!({"schema_version": 1, "user_profile": {"city": "Barcelona"}});
    let resp = app
        .put("/api/persistent-memory")
        .json(&json!({"payload": next}))
        .send()
        .await;
    assert_eq!(resp.status(), 200);
    let body = resp.json::<serde_json::Value>().await;
    assert_eq!(body["payload"], next);
}

// ─── Requirement: el estado persistente SHALL poder vaciarse ─────────────────

/// Vaciar elimina la fila y la lectura posterior reporta estado vacío.
#[tokio::test]
async fn delete_removes_state_and_is_idempotent() {
    let app = TestApp::new().await;
    let seeded = app
        .put("/api/persistent-memory")
        .json(&json!({"payload": small_state()}))
        .send()
        .await;
    assert_eq!(seeded.status(), 200);

    let resp = app.delete("/api/persistent-memory").await;
    assert_eq!(resp.status(), 204);

    let get = app
        .get("/api/persistent-memory")
        .await
        .json::<serde_json::Value>()
        .await;
    assert_eq!(get["payload"], serde_json::Value::Null);
    assert_eq!(get["is_empty"], true);

    // Idempotent: deleting again still succeeds and creates nothing.
    let again = app.delete("/api/persistent-memory").await;
    assert_eq!(again.status(), 204);
}

/// Vaciar un estado ausente responde igualmente con éxito.
#[tokio::test]
async fn delete_absent_state_is_idempotent() {
    let app = TestApp::new().await;

    let resp = app.delete("/api/persistent-memory").await;
    assert_eq!(resp.status(), 204);

    let get = app
        .get("/api/persistent-memory")
        .await
        .json::<serde_json::Value>()
        .await;
    assert_eq!(get["is_empty"], true);
}
