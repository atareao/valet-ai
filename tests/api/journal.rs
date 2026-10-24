//! Tests de integración de los endpoints de sesión de la bitácora
//! (`/api/journal`), según `openspec/changes/journal-entries/specs/journal-api/spec.md`:
//! el listado por rango de más reciente a más antigua y con tope, la lectura de
//! un día, el alta/reemplazo del día y su borrado, con `400` para lo mal formado
//! (sin escribir nada), `404` para el día sin entrada y `401` sin sesión.
//!
//! El perfil lo resuelve la API con `ProfilesRepo::get_or_create`, que sobre la
//! base sembrada de `TestApp::new()` devuelve el perfil `profile-id`; por eso las
//! entradas que estos tests siembran para el «Given» llevan ese mismo
//! `profile_id`.

mod common;
use common::TestApp;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::json;
use tower::ServiceExt;

use valet::db::repos::journal::JournalRepo;
use valet::models::journal::{JournalEntry, Mood};

/// El perfil que devuelve `ProfilesRepo::get_or_create` sobre la base de
/// `TestApp::new()`.
const PROFILE: &str = "profile-id";

const DAY: &str = "2026-10-18";

// ---------------------------------------------------------------------------
// Ayudantes
// ---------------------------------------------------------------------------

/// Siembra (fuera de la API) la entrada de ese día, para el «Given» de los
/// escenarios de lectura, reemplazo y borrado.
async fn seed_entry(app: &TestApp, date: &str, content: &str, mood: Option<Mood>) {
    let entry = JournalEntry {
        id: format!("seed-{date}"),
        profile_id: PROFILE.into(),
        date: date.into(),
        mood,
        content: content.into(),
        achievements: Vec::new(),
        struggles: Vec::new(),
        tags: Vec::new(),
        created_at: String::new(),
        updated_at: String::new(),
    };
    JournalRepo::upsert(&app.db, &entry)
        .await
        .expect("seed journal entry");
}

/// Cuántas filas hay para ese día en la tabla (nunca a través de la API, para
/// poder probar que no hay duplicados).
async fn rows_for(app: &TestApp, date: &str) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM journal_entries WHERE profile_id = ?1 AND date = ?2")
        .bind(PROFILE)
        .bind(date)
        .fetch_one(&app.db)
        .await
        .expect("count query must succeed")
}

/// El relato guardado de ese día, leído de la tabla.
async fn stored_content(app: &TestApp, date: &str) -> Option<String> {
    JournalRepo::get(&app.db, PROFILE, date)
        .await
        .expect("read stored entry")
        .map(|entry| entry.content)
}

/// Una aplicación con la sesión OIDC habilitada, para probar el `401` sin
/// cookie (con la sesión deshabilitada el middleware es un pasamanos).
async fn app_with_auth_enabled() -> axum::Router {
    let mut state = valet::AppState::new_in_memory_empty().await;
    state.auth_config = Some(valet::auth::AuthConfig {
        enabled: true,
        issuer_url: "http://localhost:9".into(),
        client_id: "valet-client".into(),
        client_secret: "client-secret".into(),
        redirect_url: "http://localhost:3000/api/auth/callback".into(),
        jwt_secret: "integration-test-secret".into(),
        post_logout_redirect_url: "http://localhost:5173/exit".into(),
    });
    valet::app_with_state(state)
}

fn get_request(uri: &str) -> Request<Body> {
    Request::builder()
        .uri(uri)
        .method("GET")
        .body(Body::empty())
        .expect("build GET request")
}

fn put_request(uri: &str, body: &serde_json::Value) -> Request<Body> {
    Request::builder()
        .uri(uri)
        .method("PUT")
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .expect("build PUT request")
}

// ---------------------------------------------------------------------------
// 5.1 — GET /api/journal
// ---------------------------------------------------------------------------

/// Scenario: El listado devuelve el rango de más reciente a más antigua.
#[tokio::test]
async fn list_returns_range_newest_first() {
    let app = TestApp::new().await;
    seed_entry(&app, "2026-10-18", "el más reciente", None).await;
    seed_entry(&app, "2026-10-15", "el de en medio", None).await;
    seed_entry(&app, "2026-10-10", "el más antiguo", None).await;
    seed_entry(&app, "2026-09-30", "fuera del rango", None).await;

    let resp = app.get("/api/journal?from=2026-10-01&to=2026-10-18").await;

    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.json::<serde_json::Value>().await;
    let dates: Vec<&str> = body
        .as_array()
        .expect("the listing must be an array")
        .iter()
        .filter_map(|entry| entry["date"].as_str())
        .collect();
    assert_eq!(
        dates,
        vec!["2026-10-18", "2026-10-15", "2026-10-10"],
        "the range must come newest first and exclude what falls outside it"
    );
}

/// Scenario: El listado respeta el tope.
#[tokio::test]
async fn list_respects_limit() {
    let app = TestApp::new().await;
    seed_entry(&app, "2026-10-18", "el más reciente", None).await;
    seed_entry(&app, "2026-10-15", "otro", None).await;
    seed_entry(&app, "2026-10-10", "el más antiguo", None).await;

    let resp = app.get("/api/journal?limit=1").await;

    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.json::<serde_json::Value>().await;
    let entries = body.as_array().expect("the listing must be an array");
    assert_eq!(entries.len(), 1, "limit must cap the result");
    assert_eq!(
        entries[0]["date"], "2026-10-18",
        "the single entry kept must be the newest one"
    );
}

/// Scenario: Una fecha mal formada se rechaza en la lectura (un `from`/`to` mal
/// formado responde `400` con el motivo).
#[tokio::test]
async fn list_with_malformed_from_returns_400() {
    let app = TestApp::new().await;

    let resp = app.get("/api/journal?from=2026-10-1").await;

    assert_eq!(
        resp.status(),
        StatusCode::BAD_REQUEST,
        "a malformed `from` must be rejected with 400"
    );
    let body = resp.json::<serde_json::Value>().await;
    assert!(
        body["error"].is_string(),
        "the 400 must carry a reason in `error`: {body}"
    );
}

/// Scenario: Sin sesión no se lee la bitácora.
#[tokio::test]
async fn list_without_session_returns_401() {
    let app = app_with_auth_enabled().await;

    let resp = app
        .oneshot(get_request("/api/journal"))
        .await
        .expect("the router must answer");

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

// ---------------------------------------------------------------------------
// 5.2 — GET /api/journal/{date}
// ---------------------------------------------------------------------------

/// Scenario: Leer un día con entrada. Cada entrada expone todos sus campos.
#[tokio::test]
async fn get_existing_day_returns_200_with_entry() {
    let app = TestApp::new().await;
    let entry = JournalEntry {
        id: "seed-rico".into(),
        profile_id: PROFILE.into(),
        date: DAY.into(),
        mood: Some(Mood::Good),
        content: "un día largo".into(),
        achievements: vec!["terminar el informe".into()],
        struggles: vec!["dormir poco".into()],
        tags: vec!["trabajo".into()],
        created_at: String::new(),
        updated_at: String::new(),
    };
    JournalRepo::upsert(&app.db, &entry)
        .await
        .expect("seed rich entry");

    let resp = app.get(&format!("/api/journal/{DAY}")).await;

    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.json::<serde_json::Value>().await;
    assert_eq!(body["date"], DAY);
    assert_eq!(body["profile_id"], PROFILE);
    assert_eq!(body["mood"], "good");
    assert_eq!(body["content"], "un día largo");
    assert_eq!(body["achievements"], json!(["terminar el informe"]));
    assert_eq!(body["struggles"], json!(["dormir poco"]));
    assert_eq!(body["tags"], json!(["trabajo"]));
    assert!(body["id"].is_string(), "the entry must expose its id");
    assert!(
        body["created_at"].is_string(),
        "the entry must expose created_at"
    );
    assert!(
        body["updated_at"].is_string(),
        "the entry must expose updated_at"
    );
}

/// Scenario: Leer un día sin entrada.
#[tokio::test]
async fn get_missing_day_returns_404() {
    let app = TestApp::new().await;

    let resp = app.get("/api/journal/2026-10-19").await;

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

/// Scenario: Una fecha mal formada se rechaza en la lectura.
#[tokio::test]
async fn get_malformed_date_returns_400() {
    let app = TestApp::new().await;

    let resp = app.get("/api/journal/2026-10-32").await;

    assert_eq!(
        resp.status(),
        StatusCode::BAD_REQUEST,
        "a malformed `date` must be rejected with 400"
    );
    let body = resp.json::<serde_json::Value>().await;
    assert!(
        body["error"].is_string(),
        "the 400 must carry a reason in `error`: {body}"
    );
}

// ---------------------------------------------------------------------------
// 5.3 — PUT /api/journal/{date}
// ---------------------------------------------------------------------------

/// Scenario: `PUT` crea la entrada del día.
#[tokio::test]
async fn put_creates_entry_of_the_day() {
    let app = TestApp::new().await;

    let resp = app
        .put(&format!("/api/journal/{DAY}"))
        .json(&json!({ "content": "el primer relato" }))
        .send()
        .await;

    assert_eq!(resp.status(), StatusCode::OK);
    let created = resp.json::<serde_json::Value>().await;
    assert_eq!(created["date"], DAY);
    assert_eq!(created["content"], "el primer relato");
    assert_eq!(rows_for(&app, DAY).await, 1, "the day must hold one entry");

    // And GET /api/journal/{date} la devuelve.
    let resp = app.get(&format!("/api/journal/{DAY}")).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let read = resp.json::<serde_json::Value>().await;
    assert_eq!(read["content"], "el primer relato");
}

/// Scenario: `PUT` reemplaza la del mismo día sin duplicar.
#[tokio::test]
async fn put_replaces_same_day_without_duplicating() {
    let app = TestApp::new().await;
    seed_entry(&app, DAY, "el relato viejo", None).await;
    let created_at = JournalRepo::get(&app.db, PROFILE, DAY)
        .await
        .expect("read seeded entry")
        .expect("the seeded entry must exist")
        .created_at;

    let resp = app
        .put(&format!("/api/journal/{DAY}"))
        .json(&json!({ "content": "el relato nuevo", "mood": "great" }))
        .send()
        .await;

    assert_eq!(resp.status(), StatusCode::OK);
    let replaced = resp.json::<serde_json::Value>().await;
    assert_eq!(replaced["content"], "el relato nuevo");
    assert_eq!(replaced["mood"], "great");
    assert_eq!(
        rows_for(&app, DAY).await,
        1,
        "replacing must not duplicate the day"
    );

    let stored = JournalRepo::get(&app.db, PROFILE, DAY)
        .await
        .expect("read replaced entry")
        .expect("the entry must still exist");
    assert_eq!(stored.content, "el relato nuevo");
    assert_eq!(
        stored.created_at, created_at,
        "the original created_at must survive the replacement"
    );
}

/// Scenario: Un `content` ausente responde `400`.
#[tokio::test]
async fn put_without_content_returns_400() {
    let app = TestApp::new().await;

    let resp = app
        .put(&format!("/api/journal/{DAY}"))
        .json(&json!({ "mood": "good" }))
        .send()
        .await;

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body = resp.json::<serde_json::Value>().await;
    assert!(
        body["error"].is_string(),
        "the 400 must carry a reason in `error`: {body}"
    );
    assert_eq!(
        rows_for(&app, DAY).await,
        0,
        "a rejected body must not write anything"
    );
}

/// Un `content` vacío también responde `400`.
#[tokio::test]
async fn put_with_empty_content_returns_400() {
    let app = TestApp::new().await;

    let resp = app
        .put(&format!("/api/journal/{DAY}"))
        .json(&json!({ "content": "   " }))
        .send()
        .await;

    assert_eq!(
        resp.status(),
        StatusCode::BAD_REQUEST,
        "an empty content must be rejected with 400"
    );
    assert_eq!(
        rows_for(&app, DAY).await,
        0,
        "a rejected body must not write anything"
    );
}

/// Scenario: Un ánimo fuera del conjunto se rechaza sin escribir.
#[tokio::test]
async fn put_with_mood_out_of_set_returns_400_and_does_not_write() {
    let app = TestApp::new().await;
    seed_entry(&app, DAY, "el relato intacto", Some(Mood::Good)).await;

    let resp = app
        .put(&format!("/api/journal/{DAY}"))
        .json(&json!({ "content": "el relato nuevo", "mood": "genial" }))
        .send()
        .await;

    assert_eq!(
        resp.status(),
        StatusCode::BAD_REQUEST,
        "a mood outside the closed set must be rejected with 400"
    );
    let body = resp.json::<serde_json::Value>().await;
    assert!(
        body["error"].is_string(),
        "the 400 must carry a reason in `error`: {body}"
    );
    assert_eq!(
        stored_content(&app, DAY).await.as_deref(),
        Some("el relato intacto"),
        "a rejected body must leave the day untouched"
    );
}

/// Scenario: Una entrada sin ánimo se acepta.
#[tokio::test]
async fn put_without_mood_returns_200_with_null_mood() {
    let app = TestApp::new().await;

    let resp = app
        .put(&format!("/api/journal/{DAY}"))
        .json(&json!({ "content": "un día sin etiqueta" }))
        .send()
        .await;

    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.json::<serde_json::Value>().await;
    assert_eq!(body["content"], "un día sin etiqueta");
    assert_eq!(body["mood"], serde_json::Value::Null);
}

/// Scenario: Sin sesión no se escribe la bitácora.
#[tokio::test]
async fn put_without_session_returns_401() {
    let app = app_with_auth_enabled().await;

    let resp = app
        .oneshot(put_request(
            &format!("/api/journal/{DAY}"),
            &json!({ "content": "no debería escribirse" }),
        ))
        .await
        .expect("the router must answer");

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

// ---------------------------------------------------------------------------
// 5.4 — DELETE /api/journal/{date}
// ---------------------------------------------------------------------------

/// Scenario: `DELETE` borra la del día.
#[tokio::test]
async fn delete_removes_entry_and_returns_204() {
    let app = TestApp::new().await;
    seed_entry(&app, DAY, "el relato que se borra", None).await;

    let resp = app.delete(&format!("/api/journal/{DAY}")).await;

    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    assert_eq!(rows_for(&app, DAY).await, 0, "the day must be gone");

    // And GET /api/journal/{date} pasa a responder 404.
    let resp = app.get(&format!("/api/journal/{DAY}")).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

/// Scenario: `DELETE` de lo que no existe.
#[tokio::test]
async fn delete_missing_day_returns_404() {
    let app = TestApp::new().await;

    let resp = app.delete("/api/journal/2026-10-19").await;

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}
