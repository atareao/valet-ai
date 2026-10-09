//! Integration tests for the OIDC authentication surface.
//!
//! RED phase: these tests describe the required behaviour and must fail until
//! the GREEN phase implements the session cookie, the auth routes and the
//! session-enforcement middleware.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::Value;
use tower::ServiceExt;
use valet::auth::{AuthConfig, Claims, SESSION_COOKIE_NAME};

fn enabled_config(issuer: &str) -> AuthConfig {
    AuthConfig {
        enabled: true,
        issuer_url: issuer.to_string(),
        client_id: "valet-client".into(),
        client_secret: "client-secret".into(),
        redirect_url: "http://localhost:3000/api/auth/callback".into(),
        jwt_secret: "integration-test-secret".into(),
        post_logout_redirect_url: "http://localhost:5173/exit".into(),
    }
}

fn disabled_config() -> AuthConfig {
    AuthConfig {
        enabled: false,
        ..enabled_config("http://localhost:9")
    }
}

fn session_claims(id_token: Option<&str>) -> Claims {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    Claims {
        sub: "user-123".into(),
        email: Some("user@example.com".into()),
        name: Some("Test User".into()),
        exp: (now + 3600) as usize,
        iat: now as usize,
        id_token: id_token.map(str::to_string),
    }
}

async fn app_with(config: AuthConfig) -> axum::Router {
    let mut state = valet::AppState::new_in_memory_empty().await;
    state.auth_config = Some(config);
    valet::app_with_state(state)
}

async fn oidc_server() -> wiremock::MockServer {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    let uri = server.uri();
    let discovery = serde_json::json!({
        "issuer": uri,
        "authorization_endpoint": format!("{uri}/authorize"),
        "token_endpoint": format!("{uri}/token"),
        "jwks_uri": format!("{uri}/jwks"),
        "end_session_endpoint": format!("{uri}/end-session"),
    });

    Mock::given(method("GET"))
        .and(path("/.well-known/openid-configuration"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/json")
                .set_body_string(discovery.to_string()),
        )
        .mount(&server)
        .await;

    server
}

fn get(uri: &str) -> Request<Body> {
    Request::builder()
        .method("GET")
        .uri(uri)
        .body(Body::empty())
        .unwrap()
}

fn get_with_cookie(uri: &str, cookie: &str) -> Request<Body> {
    Request::builder()
        .method("GET")
        .uri(uri)
        .header("cookie", cookie)
        .body(Body::empty())
        .unwrap()
}

fn post_with_cookie(uri: &str, cookie: &str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header("cookie", cookie)
        .body(Body::empty())
        .unwrap()
}

// ── OIDC test provider helpers ────────────────────────────────────────────
//
// A fixed RSA key pair signs the fake ID tokens; its public half is served as
// the JWKS document so `validate_id_token` can verify the signature.
const TEST_KID: &str = "e2e-kid";
const TEST_JWKS_N: &str = "ybHbf8ju7SODIbYvLInRYmtJadSL4bsjqzO9f-87xoEtPLSPwqZzePmwZywjeN3DjygHPm5Fb7kuZRqYTGfrkWk9uIs4Z6xE_fcXiT1Qq9g9O_oPp-twq0os_--ClSmE7LZC4YESzeKHuFwHq1vRt3ud6vWbyvsNfOHQl_fue2_2gE5EnAzOIZ_zSrWrnl0LAJuT-qKBPC3k-LBvMGeXWSTZ8AgBTqMhxJxITrxgVqGwLOUOp3pyTMYop0B5YWX1m1xpK-r7Ria5t3lMc5-TNOZtb_Rxo6TNjB_Kdwzr7ZlpFcWH_-FWWwfBHKp5_jsFihtazz77sz8tTo_4QVJDHw";
const TEST_JWKS_E: &str = "AQAB";
const TEST_PRIVATE_KEY_PEM: &str = r#"-----BEGIN PRIVATE KEY-----
MIIEvgIBADANBgkqhkiG9w0BAQEFAASCBKgwggSkAgEAAoIBAQDJsdt/yO7tI4Mh
ti8sidFia0lp1IvhuyOrM71/7zvGgS08tI/CpnN4+bBnLCN43cOPKAc+bkVvuS5l
GphMZ+uRaT24izhnrET99xeJPVCr2D07+g+n63CrSiz/74KVKYTstkLhgRLN4oe4
XAerW9G3e53q9ZvK+w184dCX9+57b/aATkScDM4hn/NKtaueXQsAm5P6ooE8LeT4
sG8wZ5dZJNnwCAFOoyHEnEhOvGBWobAs5Q6nenJMxiinQHlhZfWbXGkr6vtGJrm3
eUxzn5M05m1v9HGjpM2MH8p3DOvtmWkVxYf/4VZbB8Ecqnn+OwWKG1rPPvuzPy1O
j/hBUkMfAgMBAAECggEAFU68VRoZn6TGSCvyfN6MZ7zU0yDYrD60bHQ5W0gfRP/F
kymyHEqwXUHnkGa50p1++OMuLLrCjCSAkb7HAx9hZAm9sv7GNTGeUkQl0OJQ4O6l
vW3T9JXrX7Uk3t7jKTXrLISuuRsFLQn285OJXeGD3MHq83UCAeYWzTAf9MYFBDYm
EkKlTGUuX6O6xZEeuaieuaU9qRkUk35vm8fRwJcgvUeduX8xR+CgOVmSlvz8UY/y
c8PV+1hBHZhPFCDAmxbvaiZNFthTc/s3X/CFwaqlh2um+FQYaTxRiSmEC7KKCTaj
NvqcjVXFHh4+KVvVyRDyCuRglsJPYZdrHwgLyKmRxQKBgQD6yDGHo9dlZEIz0CSO
LJ1VfbmAKvJzAQ8oDhaxAHoAQTgk6z6oiC+j1z7ong+Yrp8U6X9ZaEDjtoQsCAW/
lhVNmHiYKayJUUsQ00zdcxDnWvv4vheQb6BERZBkXUSgsL+Ja5YwVSO7s/H9mAP+
Bw89gvplbLGkVmv9kQg5/65eGwKBgQDN5DKR7bVVoUeMJVLklYwAyJoJqt6jZ+pb
vJsN6OcNRUfLz1dqOZ1mX+wjdGunmMQ1s8FXuq1nGkgZjsNXdrHwUFPAQOIy3WjI
OOgh8C5E1b/YeVMn0G8lJYoC3nh0gvNCkeq7tMk8EszDg7BoJnkALmGXqlzAD/qO
u3jP2VsvTQKBgQC4ZBge7nYCo/wVUrZ+Hwm0AVQyi+Fmc+HsBqfij0IlC883Pgz9
J903b51etlErZ0Gqw6CSYZhMljeKlH99heG2AySwZPvqn249Oc/rh55nLbvVAhgb
aCgD6s4nLXi3Wh4K8aMleIRkkAOe/XX6AsDO7o4jow7ekXinoUrWMkvH/wKBgHOF
HQaZrAuekQlGC8trSQFLjHnuIDro3CqamRjl23WrsIvfCvnz16eQHGsMMDb51OUY
tVtmx1I5AcktO9cAJvhh6YvOG9xUNC1bGIuqOhuvHpP0Br8pCKN7+4J+lnEw5BIu
0th8qAgHuFHeuoTFSVDbtTAmiyHMFVYuQ6vDFZFlAoGBANcfxMTXA4tZg/tQUQNH
sD4oZPVlcmNFXsd9j42vgLnPo+F1xtnm2M33fNEyq9xRSdmu8OfjByimDynIINOk
GGqx5iiXrtdzoFsEVj2ueae5LFmbHJOQI/o1j6cse5nKpQs4MyQJ6sI4/d+uul6u
SestSfOxhvIiYxMrOAHQ3iRF
-----END PRIVATE KEY-----
"#;

fn sign_id_token(issuer: &str, audience: &str, nonce: &str) -> String {
    use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};

    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some(TEST_KID.into());

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let claims = serde_json::json!({
        "iss": issuer,
        "aud": audience,
        "sub": "user-123",
        "email": "user@example.com",
        "name": "Test User",
        "nonce": nonce,
        "iat": now,
        "exp": now + 300,
    });

    encode(
        &header,
        &claims,
        &EncodingKey::from_rsa_pem(TEST_PRIVATE_KEY_PEM.as_bytes())
            .expect("test RSA key must parse"),
    )
    .expect("test ID token must encode")
}

async fn mount_jwks(server: &wiremock::MockServer) {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, ResponseTemplate};

    let body = format!(
        r#"{{"keys":[{{"kty":"RSA","use":"sig","kid":"{TEST_KID}","alg":"RS256","n":"{TEST_JWKS_N}","e":"{TEST_JWKS_E}"}}]}}"#
    );
    Mock::given(method("GET"))
        .and(path("/jwks"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/json")
                .set_body_string(body),
        )
        .mount(server)
        .await;
}

async fn mount_token(server: &wiremock::MockServer, id_token: &str) {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, ResponseTemplate};

    let body = serde_json::json!({
        "id_token": id_token,
        "token_type": "Bearer",
        "access_token": "provider-access-token",
    })
    .to_string();
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/json")
                .set_body_string(body),
        )
        .mount(server)
        .await;
}

/// Extract the value of a `Set-Cookie` header by cookie name.
fn set_cookie_value(headers: &axum::http::HeaderMap, name: &str) -> Option<String> {
    for value in headers.get_all(axum::http::header::SET_COOKIE).iter() {
        let header = value.to_str().ok()?;
        let pair = header.split(';').next().unwrap_or("");
        if let Some((key, value)) = pair.split_once('=') {
            if key.trim() == name {
                return Some(value.trim().to_string());
            }
        }
    }
    None
}

/// Read a query parameter from a URL.
fn query_param(url: &str, key: &str) -> String {
    url::Url::parse(url)
        .unwrap()
        .query_pairs()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.into_owned())
        .unwrap_or_else(|| panic!("missing query param `{key}` in {url}"))
}

// ── /api/auth/me ──────────────────────────────────────────────────────────

#[tokio::test]
async fn me_without_cookie_returns_401() {
    let app = app_with(enabled_config("http://localhost:9")).await;

    let resp = app.oneshot(get("/api/auth/me")).await.unwrap();

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn me_with_valid_session_returns_claims() {
    let config = enabled_config("http://localhost:9");
    let token = valet::auth::emit_session(&config, &session_claims(None))
        .expect("emit_session should succeed for a valid session");
    let app = app_with(config).await;

    let resp = app
        .oneshot(get_with_cookie(
            "/api/auth/me",
            &format!("{SESSION_COOKIE_NAME}={token}"),
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);

    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["sub"], "user-123");
    assert_eq!(json["email"], "user@example.com");
    assert_eq!(json["name"], "Test User");
    assert!(
        json.get("id_token").is_none() && json.get("token").is_none(),
        "the session token / ID token must not be exposed in the body"
    );
}

// ── Enforcement and exemptions ────────────────────────────────────────────

#[tokio::test]
async fn protected_route_without_cookie_returns_401() {
    let app = app_with(enabled_config("http://localhost:9")).await;

    let resp = app.oneshot(get("/api/messages")).await.unwrap();

    assert_eq!(
        resp.status(),
        StatusCode::UNAUTHORIZED,
        "a protected /api route must require a session when auth is enabled"
    );
}

#[tokio::test]
async fn protected_route_with_valid_session_is_not_rejected() {
    let config = enabled_config("http://localhost:9");
    let token = valet::auth::emit_session(&config, &session_claims(None))
        .expect("emit_session should succeed for a valid session");
    let app = app_with(config).await;

    let resp = app
        .oneshot(get_with_cookie(
            "/api/messages",
            &format!("{SESSION_COOKIE_NAME}={token}"),
        ))
        .await
        .unwrap();

    assert_ne!(
        resp.status(),
        StatusCode::UNAUTHORIZED,
        "a valid session must not be rejected by the middleware"
    );
}

#[tokio::test]
async fn health_is_exempt_from_session_enforcement() {
    let app = app_with(enabled_config("http://localhost:9")).await;

    let resp = app.oneshot(get("/api/health")).await.unwrap();

    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "/api/health must stay reachable without a session"
    );
}

#[tokio::test]
async fn login_redirects_to_the_provider() {
    let server = oidc_server().await;
    let app = app_with(enabled_config(&server.uri())).await;

    let resp = app.oneshot(get("/api/auth/login")).await.unwrap();

    assert_eq!(resp.status(), StatusCode::FOUND);
    let location = resp
        .headers()
        .get("location")
        .expect("login must redirect")
        .to_str()
        .unwrap();

    assert!(location.contains("response_type=code"), "{location}");
    assert!(location.contains("client_id=valet-client"), "{location}");
    assert!(location.contains("redirect_uri="), "{location}");
    assert!(location.contains("openid"), "{location}");
    assert!(location.contains("state="), "{location}");
    assert!(location.contains("nonce="), "{location}");
    assert!(
        location.starts_with(&format!("{}/authorize", server.uri())),
        "login must redirect to the provider authorization endpoint: {location}"
    );
}

#[tokio::test]
async fn callback_without_state_returns_401() {
    let server = oidc_server().await;
    let app = app_with(enabled_config(&server.uri())).await;

    let resp = app
        .oneshot(get("/api/auth/callback?code=some-code"))
        .await
        .unwrap();

    assert_eq!(
        resp.status(),
        StatusCode::UNAUTHORIZED,
        "a callback without a valid state must be rejected"
    );
}

#[tokio::test]
async fn callback_with_valid_state_establishes_session() {
    let server = oidc_server().await;
    let app = app_with(enabled_config(&server.uri())).await;

    // 1. Login → signed flow cookie + state/nonce in the authorization URL.
    let login = app.clone().oneshot(get("/api/auth/login")).await.unwrap();
    assert_eq!(login.status(), StatusCode::FOUND);
    let location = login
        .headers()
        .get("location")
        .expect("login must redirect")
        .to_str()
        .unwrap()
        .to_string();
    let flow = set_cookie_value(login.headers(), "valet_oauth_flow")
        .expect("login must set the signed flow cookie");
    let state = query_param(&location, "state");
    let nonce = query_param(&location, "nonce");

    // 2. Provider side: JWKS + token endpoint returning a signed ID token.
    let id_token = sign_id_token(&server.uri(), "valet-client", &nonce);
    mount_jwks(&server).await;
    mount_token(&server, &id_token).await;

    // 3. Callback with the matching state and the flow cookie.
    let callback = app
        .clone()
        .oneshot(get_with_cookie(
            &format!("/api/auth/callback?code=the-code&state={state}"),
            &format!("valet_oauth_flow={flow}"),
        ))
        .await
        .unwrap();

    assert_eq!(callback.status(), StatusCode::FOUND);
    let session = set_cookie_value(callback.headers(), SESSION_COOKIE_NAME)
        .expect("a valid callback must establish the session cookie");
    assert!(
        !session.is_empty(),
        "the session cookie must carry a signed token"
    );
}

#[tokio::test]
async fn callback_with_mismatched_state_returns_401() {
    let server = oidc_server().await;
    let app = app_with(enabled_config(&server.uri())).await;

    let login = app.clone().oneshot(get("/api/auth/login")).await.unwrap();
    let flow = set_cookie_value(login.headers(), "valet_oauth_flow").unwrap();

    let resp = app
        .clone()
        .oneshot(get_with_cookie(
            "/api/auth/callback?code=the-code&state=forged-state",
            &format!("valet_oauth_flow={flow}"),
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert!(
        set_cookie_value(resp.headers(), SESSION_COOKIE_NAME).is_none(),
        "a mismatched state must not establish a session"
    );
}

#[tokio::test]
async fn callback_reusing_state_returns_401() {
    let server = oidc_server().await;
    let app = app_with(enabled_config(&server.uri())).await;

    let login = app.clone().oneshot(get("/api/auth/login")).await.unwrap();
    let location = login
        .headers()
        .get("location")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    let flow = set_cookie_value(login.headers(), "valet_oauth_flow").unwrap();
    let state = query_param(&location, "state");
    let nonce = query_param(&location, "nonce");

    let id_token = sign_id_token(&server.uri(), "valet-client", &nonce);
    mount_jwks(&server).await;
    mount_token(&server, &id_token).await;

    let uri = format!("/api/auth/callback?code=the-code&state={state}");
    let cookie = format!("valet_oauth_flow={flow}");

    // First use consumes the state and succeeds.
    let first = app
        .clone()
        .oneshot(get_with_cookie(&uri, &cookie))
        .await
        .unwrap();
    assert_eq!(first.status(), StatusCode::FOUND);
    assert!(set_cookie_value(first.headers(), SESSION_COOKIE_NAME).is_some());

    // Replaying the same state must be refused and must not set a session.
    let replay = app
        .clone()
        .oneshot(get_with_cookie(&uri, &cookie))
        .await
        .unwrap();
    assert_eq!(
        replay.status(),
        StatusCode::UNAUTHORIZED,
        "a consumed state must not be accepted twice"
    );
    assert!(
        set_cookie_value(replay.headers(), SESSION_COOKIE_NAME).is_none(),
        "a replayed callback must not establish a session"
    );
}

// ── Development mode ──────────────────────────────────────────────────────

#[tokio::test]
async fn dev_mode_allows_api_without_cookie() {
    let app = app_with(disabled_config()).await;

    let resp = app.oneshot(get("/api/messages")).await.unwrap();

    assert_ne!(
        resp.status(),
        StatusCode::UNAUTHORIZED,
        "with auth disabled the API is reachable as the dev user"
    );
}

#[tokio::test]
async fn default_app_without_auth_config_allows_api() {
    let app = valet::app().await;

    let resp = app.oneshot(get("/api/messages")).await.unwrap();

    assert_ne!(
        resp.status(),
        StatusCode::UNAUTHORIZED,
        "the default app (no auth config) must keep working"
    );
}

// ── Logout ────────────────────────────────────────────────────────────────

#[tokio::test]
async fn logout_returns_end_session_url_with_id_token_hint() {
    let server = oidc_server().await;
    let config = enabled_config(&server.uri());
    let token = valet::auth::emit_session(&config, &session_claims(Some("id-token-abc")))
        .expect("emit_session should succeed for a valid session");
    let app = app_with(config).await;

    let resp = app
        .oneshot(post_with_cookie(
            "/api/auth/logout",
            &format!("{SESSION_COOKIE_NAME}={token}"),
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);

    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();
    let url = json["end_session_url"]
        .as_str()
        .expect("logout must return the end_session_url");

    assert!(
        url.contains("id_token_hint=id-token-abc"),
        "the end-session URL must carry id_token_hint: {url}"
    );
    assert!(
        url.contains("post_logout_redirect_uri"),
        "the end-session URL must carry post_logout_redirect_uri when configured: {url}"
    );
    assert!(
        url.contains(&format!("{}/end-session", server.uri())),
        "the end-session URL must point at the provider endpoint: {url}"
    );
}
