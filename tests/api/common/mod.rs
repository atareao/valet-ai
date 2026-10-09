//! Shared helpers for the API integration tests.
//!
//! This module is compiled once per integration-test binary (`messages`,
//! `tools`, `tasks`, `stats`, `memories`, `events`, `chat`, `profile`,
//! `search`). Each binary exercises only a subset of the helpers below, so the
//! compiler's dead-code analysis reports helpers that are used by *other*
//! binaries as unused in the current one. No helper is unused globally (e.g.
//! `get` is used by all nine, `headers` by `chat`/`stats`, `text` by `stats`),
//! so they cannot be removed without breaking tests; a module-level allow is
//! the idiomatic fix for this shared-test-module pattern.
#![allow(dead_code)]

use axum::{
    body::Body,
    http::{Method, Request, StatusCode},
};
use serde_json::Value;
use tower::ServiceExt;

pub struct TestApp {
    pub router: axum::Router,
    /// The database pool backing the app. Exposed additively so tests can seed
    /// raw rows (e.g. a corrupt payload) that no endpoint would produce.
    pub db: sqlx::SqlitePool,
}

impl TestApp {
    pub async fn new() -> Self {
        let state = valet::AppState::new_in_memory().await;
        let db = state.db.clone();
        let router = valet::app_with_state(state);
        Self { router, db }
    }

    /// Creates a TestApp with NO seed data (clean database).
    /// Only migrations and default tools are applied.
    pub async fn new_empty() -> Self {
        let state = valet::AppState::new_in_memory_empty().await;
        let db = state.db.clone();
        let router = valet::app_with_state(state);
        Self { router, db }
    }

    pub async fn get(&self, path: &str) -> TestResponse {
        let req = Request::builder()
            .uri(path)
            .method(Method::GET)
            .body(Body::empty())
            .unwrap();
        let resp = self.router.clone().oneshot(req).await.unwrap();
        TestResponse { resp }
    }

    pub fn post(&self, path: &str) -> TestRequestBuilder {
        TestRequestBuilder::new(self.router.clone(), Method::POST, path)
    }

    pub fn put(&self, path: &str) -> TestRequestBuilder {
        TestRequestBuilder::new(self.router.clone(), Method::PUT, path)
    }

    pub async fn delete(&self, path: &str) -> TestResponse {
        let req = Request::builder()
            .uri(path)
            .method(Method::DELETE)
            .body(Body::empty())
            .unwrap();
        let resp = self.router.clone().oneshot(req).await.unwrap();
        TestResponse { resp }
    }
}

pub struct TestRequestBuilder {
    router: axum::Router,
    method: Method,
    path: String,
    body: Option<String>,
}

impl TestRequestBuilder {
    pub fn new(router: axum::Router, method: Method, path: &str) -> Self {
        Self {
            router,
            method,
            path: path.to_string(),
            body: None,
        }
    }

    pub fn json(mut self, value: &Value) -> Self {
        self.body = Some(value.to_string());
        self
    }

    pub async fn send(self) -> TestResponse {
        let mut builder = Request::builder().uri(&self.path).method(&self.method);
        if let Some(body) = self.body {
            builder = builder.header("content-type", "application/json");
            let req = builder.body(Body::from(body)).unwrap();
            let resp = self.router.clone().oneshot(req).await.unwrap();
            return TestResponse { resp };
        }
        let req = builder.body(Body::empty()).unwrap();
        let resp = self.router.clone().oneshot(req).await.unwrap();
        TestResponse { resp }
    }
}

pub struct TestResponse {
    resp: axum::response::Response,
}

impl TestResponse {
    pub fn status(&self) -> StatusCode {
        self.resp.status()
    }

    pub fn headers(&self) -> &axum::http::HeaderMap {
        self.resp.headers()
    }

    pub async fn json<T: serde::de::DeserializeOwned>(self) -> T {
        let body = axum::body::to_bytes(self.resp.into_body(), usize::MAX)
            .await
            .unwrap();
        serde_json::from_slice(&body).unwrap()
    }

    /// Read the response body as a UTF-8 string (useful for CSV or plain-text
    /// responses).
    pub async fn text(self) -> String {
        let body = axum::body::to_bytes(self.resp.into_body(), usize::MAX)
            .await
            .unwrap();
        String::from_utf8(body.to_vec()).unwrap()
    }
}
