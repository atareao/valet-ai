//! Tests de integración de los endpoints de notas (`/api/notes` y
//! `/api/notes/{id}`), según `openspec/changes/notes-crud/specs/notes-api/spec.md`:
//! el listado con filtro por categoría y tope, el alta, la lectura, la
//! actualización parcial y el borrado, con `400` para lo inválido (sin escribir
//! nada), `404` para lo inexistente o ajeno, `204` al borrar y `401` sin sesión.
//!
//! El perfil lo resuelve la API con `ProfilesRepo::get_or_create`, que hace
//! `SELECT ... LIMIT 1` **sin `ORDER BY`**: con una sola fila no hay ambigüedad,
//! pero con varias el plan de consulta puede devolver cualquiera. Por eso los
//! escenarios de pertenencia no asumen cuál es el perfil de la sesión —
//! `session_and_other` se lo pregunta al repositorio antes de sembrar las notas.

mod common;
use common::TestApp;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::json;
use tower::ServiceExt;

use valet::db::repos::notes::NotesRepo;
use valet::db::repos::profiles::ProfilesRepo;

// ---------------------------------------------------------------------------
// Ayudantes
// ---------------------------------------------------------------------------

/// Crea (o devuelve) el perfil de la sesión. Sobre una base vacía solo hay una
/// fila, así que `ProfilesRepo::get_or_create` la devuelve sin ambigüedad.
async fn session_profile(app: &TestApp) -> String {
    ProfilesRepo::get_or_create(&app.db)
        .await
        .expect("the session profile must be creatable")
        .id
}

/// Siembra (fuera de la API) un perfil con ese `id`. `profiles.name` no admite
/// nulos; el resto de columnas tienen valores por defecto.
async fn seed_profile(app: &TestApp, id: &str) {
    sqlx::query("INSERT INTO profiles (id, name) VALUES (?1, ?2)")
        .bind(id)
        .bind("Perfil de prueba")
        .execute(&app.db)
        .await
        .expect("seed profile");
}

/// Crea dos perfiles y devuelve `(sesión, ajeno)`: la **sesión** es la que
/// `ProfilesRepo::get_or_create` resuelve y el **ajeno** la otra. No se asume
/// cuál de los dos elige el repositorio —resuelve con `SELECT ... LIMIT 1`, sin
/// `ORDER BY`—, se le pregunta: así los escenarios de pertenencia no dependen
/// del plan de consulta.
async fn session_and_other(app: &TestApp) -> (String, String) {
    const ALPHA: &str = "profile-alpha";
    const BETA: &str = "profile-beta";

    seed_profile(app, ALPHA).await;
    seed_profile(app, BETA).await;

    let session = ProfilesRepo::get_or_create(&app.db)
        .await
        .expect("resolve the session profile")
        .id;
    let other = if session == ALPHA { BETA } else { ALPHA };

    (session, other.to_string())
}

/// Siembra (fuera de la API) una nota con `created_at` explícito, para que el
/// orden del listado sea determinista. El `profile_id` debe existir.
async fn seed_note(
    app: &TestApp,
    id: &str,
    profile_id: &str,
    content: &str,
    category: &str,
    tags: Option<&str>,
    created_at: &str,
) {
    sqlx::query(
        "INSERT INTO notes (id, profile_id, content, category, tags, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
    )
    .bind(id)
    .bind(profile_id)
    .bind(content)
    .bind(category)
    .bind(tags)
    .bind(created_at)
    .bind(created_at)
    .execute(&app.db)
    .await
    .expect("seed note");
}

/// La nota de la tabla, leída por el repositorio (nunca por la API), para poder
/// comprobar el estado persistido.
async fn stored_note(app: &TestApp, id: &str) -> valet::db::repos::notes::Note {
    NotesRepo::find_by_id(&app.db, id)
        .await
        .expect("read stored note")
        .expect("the note must exist")
}

/// Si esa nota sigue en la tabla.
async fn note_exists(app: &TestApp, id: &str) -> bool {
    NotesRepo::find_by_id(&app.db, id)
        .await
        .expect("read note")
        .is_some()
}

/// Cuántas notas hay en total en la tabla (para el «no se crea nada»).
async fn total_notes(app: &TestApp) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM notes")
        .fetch_one(&app.db)
        .await
        .expect("count query must succeed")
}

/// Una aplicación con la sesión OIDC habilitada, para probar el `401` sin cookie
/// (con la sesión deshabilitada el middleware es un pasamanos).
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

fn post_request(uri: &str, body: &serde_json::Value) -> Request<Body> {
    Request::builder()
        .uri(uri)
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .expect("build POST request")
}

// ---------------------------------------------------------------------------
// 2.1 — GET /api/notes (listado)
// ---------------------------------------------------------------------------

/// Scenario: El listado devuelve las notas del perfil, de más reciente a más
/// antigua — y **no** las del otro perfil.
#[tokio::test]
async fn list_returns_the_profile_notes_newest_first() {
    let app = TestApp::new_empty().await;
    let (session, other) = session_and_other(&app).await;

    seed_note(
        &app,
        "n-antigua",
        &session,
        "la más antigua",
        "idea",
        None,
        "2026-10-10T09:00:00Z",
    )
    .await;
    seed_note(
        &app,
        "n-media",
        &session,
        "la de en medio",
        "fact",
        None,
        "2026-10-15T09:00:00Z",
    )
    .await;
    seed_note(
        &app,
        "n-reciente",
        &session,
        "la más reciente",
        "journal",
        None,
        "2026-10-18T09:00:00Z",
    )
    .await;
    seed_note(
        &app,
        "n-ajena",
        &other,
        "nota ajena",
        "idea",
        None,
        "2026-10-20T09:00:00Z",
    )
    .await;

    let resp = app.get("/api/notes").await;

    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.json::<serde_json::Value>().await;
    let contents: Vec<&str> = body
        .as_array()
        .expect("the listing must be an array")
        .iter()
        .filter_map(|note| note["content"].as_str())
        .collect();
    assert_eq!(
        contents,
        vec!["la más reciente", "la de en medio", "la más antigua"],
        "the listing must come newest first and exclude other profiles"
    );
}

/// Scenario: El listado filtra por categoría.
#[tokio::test]
async fn list_filters_by_category() {
    let app = TestApp::new_empty().await;
    let session = session_profile(&app).await;

    seed_note(
        &app,
        "n-idea",
        &session,
        "una idea",
        "idea",
        None,
        "2026-10-18T09:00:00Z",
    )
    .await;
    seed_note(
        &app,
        "n-journal",
        &session,
        "un diario",
        "journal",
        None,
        "2026-10-17T09:00:00Z",
    )
    .await;
    seed_note(
        &app,
        "n-fact",
        &session,
        "un dato",
        "fact",
        None,
        "2026-10-16T09:00:00Z",
    )
    .await;

    let resp = app.get("/api/notes?category=journal").await;

    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.json::<serde_json::Value>().await;
    let categories: Vec<&str> = body
        .as_array()
        .expect("the listing must be an array")
        .iter()
        .filter_map(|note| note["category"].as_str())
        .collect();
    assert_eq!(
        categories,
        vec!["journal"],
        "the listing must keep only the requested category"
    );
}

/// Scenario: Una categoría de filtro inválida se rechaza.
#[tokio::test]
async fn list_with_invalid_category_returns_400() {
    let app = TestApp::new_empty().await;
    let _ = session_profile(&app).await;

    let resp = app.get("/api/notes?category=otra").await;

    assert_eq!(
        resp.status(),
        StatusCode::BAD_REQUEST,
        "a category outside the closed set must be rejected with 400"
    );
    let body = resp.json::<serde_json::Value>().await;
    assert!(
        body["error"].is_string(),
        "the 400 must carry a reason in `error`: {body}"
    );
}

/// Scenario: El listado se acota por el limit.
#[tokio::test]
async fn list_respects_limit() {
    let app = TestApp::new_empty().await;
    let session = session_profile(&app).await;

    seed_note(
        &app,
        "n-antigua",
        &session,
        "la más antigua",
        "idea",
        None,
        "2026-10-10T09:00:00Z",
    )
    .await;
    seed_note(
        &app,
        "n-media",
        &session,
        "la de en medio",
        "idea",
        None,
        "2026-10-15T09:00:00Z",
    )
    .await;
    seed_note(
        &app,
        "n-reciente",
        &session,
        "la más reciente",
        "idea",
        None,
        "2026-10-18T09:00:00Z",
    )
    .await;

    let resp = app.get("/api/notes?limit=1").await;

    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.json::<serde_json::Value>().await;
    let notes = body.as_array().expect("the listing must be an array");
    assert_eq!(notes.len(), 1, "limit must cap the result");
    assert_eq!(
        notes[0]["content"], "la más reciente",
        "the single note kept must be the newest one"
    );
}

/// Scenario: El listado rechaza los límites inválidos.
#[tokio::test]
async fn list_with_invalid_limit_returns_400() {
    let app = TestApp::new_empty().await;
    let _ = session_profile(&app).await;

    for limit in ["0", "-1", "abc", "501"] {
        let resp = app.get(&format!("/api/notes?limit={limit}")).await;
        assert_eq!(
            resp.status(),
            StatusCode::BAD_REQUEST,
            "`limit={limit}` must be rejected with 400"
        );
    }
}

// ---------------------------------------------------------------------------
// 2.4 — POST /api/notes (alta)
// ---------------------------------------------------------------------------

/// Scenario: El alta crea la nota.
#[tokio::test]
async fn create_note_returns_201_with_the_note() {
    let app = TestApp::new_empty().await;

    let resp = app
        .post("/api/notes")
        .json(&json!({ "content": "Una idea nueva" }))
        .send()
        .await;

    assert_eq!(resp.status(), StatusCode::CREATED);
    let body = resp.json::<serde_json::Value>().await;
    assert!(
        body["id"].is_string(),
        "the created note must expose its id"
    );
    assert_eq!(body["content"], "Una idea nueva");
    assert_eq!(
        body["category"], "idea",
        "the category must default to `idea`"
    );
    assert!(
        body["profile_id"].is_string(),
        "the created note must carry the session profile_id"
    );
    assert!(
        body["created_at"].is_string(),
        "the created note must expose created_at"
    );

    let id = body["id"].as_str().unwrap();
    assert!(
        note_exists(&app, id).await,
        "the created note must be persisted"
    );
}

/// Scenario: El alta rechaza contenido vacío y no crea nada.
#[tokio::test]
async fn create_note_with_empty_content_returns_400_and_creates_nothing() {
    let app = TestApp::new_empty().await;

    let resp = app
        .post("/api/notes")
        .json(&json!({ "content": "   " }))
        .send()
        .await;

    assert_eq!(
        resp.status(),
        StatusCode::BAD_REQUEST,
        "an empty content must be rejected with 400"
    );
    assert_eq!(
        total_notes(&app).await,
        0,
        "a rejected body must not create anything"
    );
}

/// Scenario: El alta rechaza una categoría inválida y no crea nada.
#[tokio::test]
async fn create_note_with_invalid_category_returns_400_and_creates_nothing() {
    let app = TestApp::new_empty().await;

    let resp = app
        .post("/api/notes")
        .json(&json!({ "content": "Algo", "category": "otra" }))
        .send()
        .await;

    assert_eq!(
        resp.status(),
        StatusCode::BAD_REQUEST,
        "a category outside the closed set must be rejected with 400"
    );
    assert_eq!(
        total_notes(&app).await,
        0,
        "a rejected body must not create anything"
    );
}

// ---------------------------------------------------------------------------
// 2.5 — GET /api/notes/{id} (lectura)
// ---------------------------------------------------------------------------

/// Scenario: Leer una nota existente — expone todos sus campos.
#[tokio::test]
async fn get_note_returns_200_with_the_note() {
    let app = TestApp::new_empty().await;
    let session = session_profile(&app).await;
    seed_note(
        &app,
        "n-1",
        &session,
        "una nota",
        "journal",
        Some("viaje, comida"),
        "2026-10-18T09:00:00Z",
    )
    .await;

    let resp = app.get("/api/notes/n-1").await;

    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.json::<serde_json::Value>().await;
    assert_eq!(body["id"], "n-1");
    assert_eq!(body["profile_id"], session);
    assert_eq!(body["content"], "una nota");
    assert_eq!(body["category"], "journal");
    assert_eq!(body["tags"], "viaje, comida");
    assert!(body["created_at"].is_string());
    assert!(body["updated_at"].is_string());
}

/// Scenario: Leer una nota inexistente.
#[tokio::test]
async fn get_missing_note_returns_404() {
    let app = TestApp::new_empty().await;
    let _ = session_profile(&app).await;

    let resp = app.get("/api/notes/no-existe").await;

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// ---------------------------------------------------------------------------
// 2.6 — PUT /api/notes/{id} (actualización parcial)
// ---------------------------------------------------------------------------

/// Scenario: La actualización es parcial — un campo ausente no se pierde.
#[tokio::test]
async fn update_note_is_partial() {
    let app = TestApp::new_empty().await;
    let session = session_profile(&app).await;
    seed_note(
        &app,
        "n-1",
        &session,
        "Original",
        "journal",
        Some("viaje"),
        "2026-10-18T09:00:00Z",
    )
    .await;

    let resp = app
        .put("/api/notes/n-1")
        .json(&json!({ "content": "Corregido" }))
        .send()
        .await;

    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.json::<serde_json::Value>().await;
    assert_eq!(body["content"], "Corregido");
    assert_eq!(
        body["category"], "journal",
        "an absent category must be preserved"
    );
    assert_eq!(body["tags"], "viaje", "absent tags must be preserved");

    let stored = stored_note(&app, "n-1").await;
    assert_eq!(stored.content, "Corregido");
    assert_eq!(stored.category, "journal");
    assert_eq!(stored.tags.as_deref(), Some("viaje"));
}

/// Scenario: El `PUT` vacía las etiquetas con una cadena vacía.
#[tokio::test]
async fn update_note_empties_tags_with_an_empty_string() {
    let app = TestApp::new_empty().await;
    let session = session_profile(&app).await;
    seed_note(
        &app,
        "n-1",
        &session,
        "Con etiquetas",
        "idea",
        Some("a,b"),
        "2026-10-18T09:00:00Z",
    )
    .await;

    let resp = app
        .put("/api/notes/n-1")
        .json(&json!({ "tags": "" }))
        .send()
        .await;

    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.json::<serde_json::Value>().await;
    assert_eq!(body["tags"], "", "`tags: \"\"` must empty the tags");

    let stored = stored_note(&app, "n-1").await;
    assert_eq!(
        stored.tags.as_deref(),
        Some(""),
        "the empty tags must be persisted, got: {:?}",
        stored.tags
    );
}

/// Scenario: El `PUT` rechaza un cuerpo sin campos y deja la nota intacta.
#[tokio::test]
async fn update_note_without_fields_returns_400_and_leaves_it_intact() {
    let app = TestApp::new_empty().await;
    let session = session_profile(&app).await;
    seed_note(
        &app,
        "n-1",
        &session,
        "Intacta",
        "fact",
        Some("x"),
        "2026-10-18T09:00:00Z",
    )
    .await;

    let resp = app.put("/api/notes/n-1").json(&json!({})).send().await;

    assert_eq!(
        resp.status(),
        StatusCode::BAD_REQUEST,
        "a body with no field at all must be rejected with 400"
    );
    let body = resp.json::<serde_json::Value>().await;
    assert!(
        body["error"].is_string(),
        "the 400 must carry a reason in `error`: {body}"
    );
    assert_eq!(stored_note(&app, "n-1").await.content, "Intacta");
}

/// Scenario: El `PUT` rechaza un content vacío y deja la nota intacta.
#[tokio::test]
async fn update_note_with_empty_content_returns_400_and_leaves_it_intact() {
    let app = TestApp::new_empty().await;
    let session = session_profile(&app).await;
    seed_note(
        &app,
        "n-1",
        &session,
        "Intacta",
        "idea",
        None,
        "2026-10-18T09:00:00Z",
    )
    .await;

    let resp = app
        .put("/api/notes/n-1")
        .json(&json!({ "content": "   " }))
        .send()
        .await;

    assert_eq!(
        resp.status(),
        StatusCode::BAD_REQUEST,
        "an empty content must be rejected with 400"
    );
    assert_eq!(stored_note(&app, "n-1").await.content, "Intacta");
}

/// Scenario: El `PUT` rechaza una categoría inválida y deja la nota intacta.
#[tokio::test]
async fn update_note_with_invalid_category_returns_400_and_leaves_it_intact() {
    let app = TestApp::new_empty().await;
    let session = session_profile(&app).await;
    seed_note(
        &app,
        "n-1",
        &session,
        "Intacta",
        "idea",
        None,
        "2026-10-18T09:00:00Z",
    )
    .await;

    let resp = app
        .put("/api/notes/n-1")
        .json(&json!({ "category": "otra" }))
        .send()
        .await;

    assert_eq!(
        resp.status(),
        StatusCode::BAD_REQUEST,
        "a category outside the closed set must be rejected with 400"
    );
    assert_eq!(stored_note(&app, "n-1").await.content, "Intacta");
    assert_eq!(stored_note(&app, "n-1").await.category, "idea");
}

// ---------------------------------------------------------------------------
// 2.7 — DELETE /api/notes/{id} (borrado)
// ---------------------------------------------------------------------------

/// Scenario: El borrado elimina la nota — `204` y desaparece del listado.
#[tokio::test]
async fn delete_note_returns_204_and_removes_it_from_the_listing() {
    let app = TestApp::new_empty().await;
    let session = session_profile(&app).await;
    seed_note(
        &app,
        "n-1",
        &session,
        "la que se borra",
        "idea",
        None,
        "2026-10-18T09:00:00Z",
    )
    .await;

    let resp = app.delete("/api/notes/n-1").await;

    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    assert!(!note_exists(&app, "n-1").await, "the note must be gone");

    let resp = app.get("/api/notes").await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.json::<serde_json::Value>().await;
    let contents: Vec<&str> = body
        .as_array()
        .expect("the listing must be an array")
        .iter()
        .filter_map(|note| note["content"].as_str())
        .collect();
    assert!(
        !contents.contains(&"la que se borra"),
        "the deleted note must not appear in the listing: {contents:?}"
    );
}

// ---------------------------------------------------------------------------
// 2.8, 2.9 — Pertenencia al perfil (seguridad) y sesión obligatoria
// ---------------------------------------------------------------------------

/// Scenario: Ninguna operación toca notas de otro perfil — `GET`, `PUT` y
/// `DELETE` responden `404` y la nota sigue existiendo sin cambios.
#[tokio::test]
async fn other_profile_notes_are_not_reachable() {
    let app = TestApp::new_empty().await;
    let (_session, other) = session_and_other(&app).await;
    seed_note(
        &app,
        "n-ajena",
        &other,
        "intocable",
        "idea",
        None,
        "2026-10-18T09:00:00Z",
    )
    .await;

    let resp = app.get("/api/notes/n-ajena").await;
    assert_eq!(
        resp.status(),
        StatusCode::NOT_FOUND,
        "GET of another profile's note must be 404"
    );

    let resp = app
        .put("/api/notes/n-ajena")
        .json(&json!({ "content": "secuestrada" }))
        .send()
        .await;
    assert_eq!(
        resp.status(),
        StatusCode::NOT_FOUND,
        "PUT of another profile's note must be 404"
    );
    assert_eq!(stored_note(&app, "n-ajena").await.content, "intocable");

    let resp = app.delete("/api/notes/n-ajena").await;
    assert_eq!(
        resp.status(),
        StatusCode::NOT_FOUND,
        "DELETE of another profile's note must be 404"
    );
    assert!(
        note_exists(&app, "n-ajena").await,
        "the other profile's note must survive"
    );
}

/// Scenario: Sin sesión no se listan ni se crean notas.
#[tokio::test]
async fn without_session_the_notes_api_returns_401() {
    let app = app_with_auth_enabled().await;

    let resp = app
        .clone()
        .oneshot(get_request("/api/notes"))
        .await
        .expect("the router must answer");
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    let resp = app
        .oneshot(post_request(
            "/api/notes",
            &json!({ "content": "no debería crearse" }),
        ))
        .await
        .expect("the router must answer");
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}
