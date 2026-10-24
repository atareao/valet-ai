mod common;
use common::TestApp;
use serde_json::json;

#[tokio::test]
async fn test_create_message() {
    // When POST /api/messages is called with valid body
    // Then returns 201 with the created message
    let app = TestApp::new().await;

    let resp = app
        .post("/api/messages")
        .json(&json!({
            "role": "user",
            "content": "Hello, Valet!"
        }))
        .send()
        .await;

    assert_eq!(resp.status(), 201);
    let body = resp.json::<serde_json::Value>().await;
    assert!(body.get("id").is_some());
    assert_eq!(body["role"], "user");
    assert_eq!(body["content"], "Hello, Valet!");
}

#[tokio::test]
async fn test_create_message_with_tool_calls() {
    // When creating a message with tool_calls
    // Then returns 201 with tool_calls preserved
    let app = TestApp::new().await;

    let resp = app
        .post("/api/messages")
        .json(&json!({
            "role": "assistant",
            "content": "Let me check that.",
            "tool_calls": [{"name": "get_weather", "args": {"city": "Madrid"}}]
        }))
        .send()
        .await;

    assert_eq!(resp.status(), 201);
    let body = resp.json::<serde_json::Value>().await;
    assert_eq!(body["role"], "assistant");
    assert!(body.get("tool_calls").is_some());
}

#[tokio::test]
async fn test_list_messages() {
    // When GET /api/messages is called
    // Then returns 200 with paginated messages
    let app = TestApp::new().await;

    let resp = app.get("/api/messages").await;

    assert_eq!(resp.status(), 200);
    let body = resp.json::<serde_json::Value>().await;
    assert!(body.get("data").is_some());
}

#[tokio::test]
async fn test_list_messages_with_pagination() {
    // When GET /api/messages?limit=10 is called
    // Then returns 200 with paginated data
    let app = TestApp::new().await;

    let resp = app.get("/api/messages?limit=10").await;

    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn test_get_message() {
    // Given a message exists
    // When GET /api/messages/msg-id is called
    // Then returns 200 with the message
    let app = TestApp::new().await;

    let resp = app.get("/api/messages/msg-id").await;

    assert_eq!(resp.status(), 200);
    let body = resp.json::<serde_json::Value>().await;
    assert!(body.get("id").is_some());
}

#[tokio::test]
async fn test_get_message_not_found() {
    // Given no message with that id exists
    // When GET /api/messages/non-existent is called
    // Then returns 404
    let app = TestApp::new().await;

    let resp = app.get("/api/messages/non-existent").await;

    assert_eq!(resp.status(), 404);
}

/// Percent-encode a value for use as a query-string parameter. The cursor is
/// opaque, so the test only round-trips the exact string the API returned; this
/// escapes any byte outside the unreserved set so the request URI stays valid
/// (axum percent-decodes it back before handing it to the handler).
fn encode_query_value(value: &str) -> String {
    let mut out = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// Insert a row with an explicit `created_at` (the column may have one-second
/// resolution, so several rows can tie on the same timestamp).
async fn insert_message(app: &TestApp, id: &str, created_at: &str) {
    sqlx::query(
        "INSERT INTO messages (id, role, content, tokens_count, created_at) \
         VALUES (?1, 'user', ?2, 0, ?3)",
    )
    .bind(id)
    .bind(format!("Message {id}"))
    .bind(created_at)
    .execute(&app.db)
    .await
    .expect("the seed row must be inserted");
}

/// Scenario: an unreadable `cursor` is rejected with `400` and a readable
/// error, instead of being taken as a loose timestamp.
#[tokio::test]
async fn test_list_messages_malformed_cursor_returns_400() {
    let app = TestApp::new_empty().await;
    insert_message(&app, "m1", "2020-01-01 00:00:01").await;

    for cursor in ["basura", "2020-01-01 00:00:01|", "|m1"] {
        let resp = app
            .get(&format!(
                "/api/messages?cursor={}",
                encode_query_value(cursor)
            ))
            .await;

        assert_eq!(
            resp.status(),
            400,
            "a malformed cursor ({cursor:?}) must be rejected"
        );
        let body = resp.json::<serde_json::Value>().await;
        assert!(
            body["error"]
                .as_str()
                .unwrap_or_default()
                .contains("cursor"),
            "the body must carry a readable error about the cursor, got: {body}"
        );
    }
}

/// Scenario: the opaque cursor survives the wire. Page 2 is requested with the
/// `next_cursor` from page 1 and, between both pages, every message shows up —
/// including rows that tie on the same second — without repeats.
#[tokio::test]
async fn test_list_messages_cursor_round_trip_covers_same_second_rows() {
    let app = TestApp::new_empty().await;
    for id in ["n0", "n1", "n2"] {
        // The same `created_at` for all three: a second-level tie.
        insert_message(&app, id, "2020-01-01 00:00:00").await;
    }

    let first = app.get("/api/messages?limit=2").await;
    assert_eq!(first.status(), 200);
    let first_body = first.json::<serde_json::Value>().await;
    let first_page = first_body["data"].as_array().expect("data").clone();
    assert_eq!(first_page.len(), 2, "the first page honours the limit");
    let cursor = first_body["next_cursor"]
        .as_str()
        .expect("a non-empty remainder must yield a cursor")
        .to_string();

    let second = app
        .get(&format!(
            "/api/messages?limit=2&cursor={}",
            encode_query_value(&cursor)
        ))
        .await;
    assert_eq!(
        second.status(),
        200,
        "the cursor issued by page 1 must be accepted"
    );
    let second_body = second.json::<serde_json::Value>().await;
    let second_page = second_body["data"].as_array().expect("data").clone();
    assert_eq!(
        second_page.len(),
        1,
        "the same-second row left out of page 1 must be in page 2"
    );
    assert!(
        second_body["next_cursor"].is_null(),
        "the last page carries no next cursor"
    );

    let mut seen: Vec<String> = first_page
        .iter()
        .chain(second_page.iter())
        .map(|m| m["id"].as_str().expect("id").to_string())
        .collect();
    seen.sort();
    seen.dedup();
    assert_eq!(
        seen,
        vec!["n0".to_string(), "n1".to_string(), "n2".to_string()],
        "both pages must cover every message exactly once"
    );
}

#[tokio::test]
async fn test_create_long_message_triggers_collapse() {
    // When creating a message with very long content (> 2000 estimated tokens)
    // Then the response includes tokens_count > 0
    let app = TestApp::new().await;

    // ~8000 chars → ~2289 tokens (well above 2000 threshold)
    let long_content = "A ".repeat(4000);

    let resp = app
        .post("/api/messages")
        .json(&json!({
            "role": "user",
            "content": long_content
        }))
        .send()
        .await;

    assert_eq!(resp.status(), 201);
    let body = resp.json::<serde_json::Value>().await;
    assert!(body.get("id").is_some(), "Message should have an id");
    assert!(
        body["tokens_count"].as_u64().unwrap_or(0) > 0,
        "tokens_count should be > 0 for long messages"
    );
    // The collapse is async, so collapsed_content may still be null
    // at this point — we only verify the message was created correctly.
}
