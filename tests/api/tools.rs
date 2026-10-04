mod common;
use common::TestApp;

use axum::http::StatusCode;

#[tokio::test]
async fn test_list_tools() {
    // Given tools exist in the database
    // When GET /api/tools is called
    // Then returns 200 with a list of tools
    let app = TestApp::new().await;

    let resp = app.get("/api/tools").await;

    assert_eq!(resp.status(), 200);
    let body = resp.json::<serde_json::Value>().await;
    assert!(body.is_array());
}

#[tokio::test]
async fn test_toggle_tool() {
    // Given a tool exists with enabled=true
    // First, get the list of tools to find a real ID
    let app = TestApp::new().await;

    let list_resp = app.get("/api/tools").await;
    let tools = list_resp.json::<serde_json::Value>().await;
    let tool_id = tools[0]["id"].as_str().unwrap().to_string();
    assert!(tools[0]["enabled"].as_bool().unwrap());

    // When PUT /api/tools/:id/toggle is called
    // Then returns 200 with enabled=false
    let resp = app
        .put(&format!("/api/tools/{}/toggle", tool_id))
        .json(&serde_json::json!({}))
        .send()
        .await;

    assert_eq!(resp.status(), 200);
    let body = resp.json::<serde_json::Value>().await;
    assert!(!body["enabled"].as_bool().unwrap());
}

#[tokio::test]
async fn test_toggle_tool_not_found() {
    // Given no tool with that id exists
    // When PUT /api/tools/:id/toggle is called
    // Then returns 404
    let app = TestApp::new().await;

    let resp = app
        .put("/api/tools/non-existent/toggle")
        .json(&serde_json::json!({}))
        .send()
        .await;

    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn test_list_tools_includes_all_registered_tools() {
    // Given the tools table is reconciled from the production registry
    // When GET /api/tools is called
    // Then the response lists the 13 real tool names and no legacy ones
    let app = TestApp::new().await;

    let resp = app.get("/api/tools").await;
    assert_eq!(resp.status(), StatusCode::OK);
    let tools = resp.json::<serde_json::Value>().await;
    let names: Vec<&str> = tools
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|t| t["name"].as_str())
        .collect();

    for expected in [
        "calendar",
        "tasks",
        "weather",
        "geocode",
        "reverse_geocode",
        "search_places",
        "web_search",
        "reminders",
        "get_current_time",
        "get_current_location",
        "notes",
        "unified_search",
        "render_widget",
    ] {
        assert!(names.contains(&expected), "Expected tool {expected}");
    }
    for legacy in ["geo", "knowledge", "contacts", "meals", "habits"] {
        assert!(
            !names.contains(&legacy),
            "Legacy tool {legacy} must be gone"
        );
    }
}

#[tokio::test]
async fn test_list_tools_includes_unified_search() {
    // Given the database is seeded with the F5b unified_search tool
    // When GET /api/tools is called
    // Then the response contains the unified_search tool
    let app = TestApp::new().await;

    let resp = app.get("/api/tools").await;

    assert_eq!(resp.status(), StatusCode::OK);
    let tools = resp.json::<serde_json::Value>().await;
    let names: Vec<&str> = tools
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|t| t["name"].as_str())
        .collect();
    assert!(
        names.contains(&"unified_search"),
        "Expected unified_search tool"
    );
}

#[tokio::test]
async fn test_toggle_tool_enabled() {
    // Given the calendar tool exists in a known enabled state
    // When PUT /api/tools/:id/toggle is called with the calendar tool id
    // Then the response returns the tool with the enabled flag flipped
    let app = TestApp::new().await;

    // First, list tools to get the calendar tool's ID and current state
    let resp = app.get("/api/tools").await;
    assert_eq!(resp.status(), StatusCode::OK);
    let tools = resp.json::<serde_json::Value>().await;
    let calendar_tool = tools
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == "calendar")
        .expect("calendar tool should be seeded");
    let tool_id = calendar_tool["id"].as_str().unwrap();
    let was_enabled = calendar_tool["enabled"].as_bool().unwrap();

    // Toggle
    let resp = app
        .put(&format!("/api/tools/{}/toggle", tool_id))
        .json(&serde_json::json!({}))
        .send()
        .await;

    assert_eq!(resp.status(), StatusCode::OK);
    let toggled = resp.json::<serde_json::Value>().await;
    assert_eq!(toggled["enabled"].as_bool().unwrap(), !was_enabled);
}

#[tokio::test]
async fn test_list_tools_includes_geo_and_time_tools() {
    // Given the tools table is reconciled from the production registry
    // When GET /api/tools is called
    // Then the real geo and time tool names are present, not the legacy "geo"
    let app = TestApp::new().await;

    let resp = app.get("/api/tools").await;
    assert_eq!(resp.status(), StatusCode::OK);
    let tools = resp.json::<serde_json::Value>().await;
    let names: Vec<&str> = tools
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|t| t["name"].as_str())
        .collect();
    for expected in [
        "geocode",
        "reverse_geocode",
        "search_places",
        "web_search",
        "get_current_time",
        "get_current_location",
    ] {
        assert!(names.contains(&expected), "Expected tool {expected}");
    }
    assert!(!names.contains(&"geo"), "geo is not a real tool name");
}

#[tokio::test]
async fn test_toggle_nonexistent_tool_returns_error() {
    // Given no tool with that id exists
    // When PUT /api/tools/:id/toggle is called with a nonexistent id
    // Then returns 404 with an error body
    let app = TestApp::new().await;

    let resp = app
        .put("/api/tools/nonexistent-id/toggle")
        .json(&serde_json::json!({}))
        .send()
        .await;

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    let body = resp.json::<serde_json::Value>().await;
    assert!(body["error"].is_string(), "Expected an error message");
}

#[tokio::test]
async fn test_list_tools_includes_render_widget() {
    // Given the tools table is reconciled from the production registry
    // When GET /api/tools is called
    // Then the response contains the render_widget tool, enabled by default
    let app = TestApp::new().await;

    let resp = app.get("/api/tools").await;
    assert_eq!(resp.status(), StatusCode::OK);
    let tools = resp.json::<serde_json::Value>().await;
    let render = tools
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == "render_widget")
        .expect("GET /api/tools must include the render_widget tool");
    assert!(
        render["enabled"].as_bool().unwrap(),
        "render_widget must be enabled by default"
    );
}

#[tokio::test]
async fn test_toggle_render_widget() {
    // Given render_widget is listed by the API
    // When PUT /api/tools/{id}/toggle is called on it
    // Then the response returns the tool with the enabled flag flipped
    let app = TestApp::new().await;

    let resp = app.get("/api/tools").await;
    assert_eq!(resp.status(), StatusCode::OK);
    let tools = resp.json::<serde_json::Value>().await;
    let render = tools
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == "render_widget")
        .expect("GET /api/tools must include the render_widget tool");
    let tool_id = render["id"].as_str().unwrap().to_string();
    let was_enabled = render["enabled"].as_bool().unwrap();

    let resp = app
        .put(&format!("/api/tools/{}/toggle", tool_id))
        .json(&serde_json::json!({}))
        .send()
        .await;

    assert_eq!(resp.status(), StatusCode::OK);
    let toggled = resp.json::<serde_json::Value>().await;
    assert_eq!(toggled["enabled"].as_bool().unwrap(), !was_enabled);
}
