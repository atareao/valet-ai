mod common;
use common::TestApp;

use axum::http::StatusCode;
use valet::db::repos::settings::SettingsRepo;
use valet::orchestrator::skills::catalog;

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
async fn test_list_tools_includes_all_registered_tools() {
    // Given the tools table is reconciled from the production registry
    // When GET /api/tools is called
    // Then the response lists the 20 real tool names and no legacy ones
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
        "strava_recent_activities",
        "strava_activity_detail",
        "strava_activity_streams",
        "strava_athlete_stats",
        "timeline_get_events",
        "timeline_add_event",
        "timeline_delete_event",
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
async fn test_list_tools_includes_render_widget() {
    // Given the tools table is reconciled from the production registry
    // When GET /api/tools is called
    // Then the response contains the render_widget tool
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
        render["name"].as_str() == Some("render_widget"),
        "render_widget must be listed by name"
    );
}

#[tokio::test]
async fn test_list_tools_includes_the_timeline_tools() {
    // Given the tools table is reconciled from the production registry
    // When GET /api/tools is called
    // Then the response contains the three timeline tools
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
        "timeline_get_events",
        "timeline_add_event",
        "timeline_delete_event",
    ] {
        assert!(names.contains(&expected), "Expected tool {expected}");
    }
}

#[tokio::test]
async fn test_list_skills_returns_the_catalog_and_core_tools() {
    // Given the closed eight-domain skills catalog lives in code
    // When GET /api/skills is called
    // Then it returns the eight skills (with their prompt fragment key and
    //      tools) and the non-routable core set, sourced from the catalog.
    let app = TestApp::new().await;

    let resp = app.get("/api/skills").await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.json::<serde_json::Value>().await;

    let skills = body["skills"]
        .as_array()
        .expect("GET /api/skills must return a `skills` array");
    assert_eq!(skills.len(), 8, "the closed catalog has eight skills");

    let ids: Vec<&str> = skills.iter().filter_map(|s| s["id"].as_str()).collect();
    for expected in [
        "agenda",
        "pendientes",
        "recuerdos",
        "entorno",
        "web",
        "widgets",
        "running",
        "timeline",
    ] {
        assert!(
            ids.contains(&expected),
            "missing skill id {expected}: {ids:?}"
        );
    }
    for legacy in [
        "tareas",
        "recordatorios",
        "notas",
        "clima",
        "lugares",
        "busqueda_web",
        "memoria",
    ] {
        assert!(
            !ids.contains(&legacy),
            "legacy skill id {legacy} must be gone: {ids:?}"
        );
    }

    for skill in skills {
        assert!(
            skill["prompt_key"].as_str().is_some_and(|k| !k.is_empty()),
            "every skill must expose a non-empty prompt_key: {skill}"
        );
        let tools = skill["tools"]
            .as_array()
            .expect("every skill must expose a `tools` array");
        assert!(!tools.is_empty(), "skill {} covers no tools", skill["id"]);
    }

    // A concrete skill carries the documented fragment key/heading and tools.
    let agenda = skills
        .iter()
        .find(|s| s["id"] == "agenda")
        .expect("agenda must be in the catalog");
    assert_eq!(agenda["prompt_key"], "SKILL_AGENDA_PROMPT");
    assert_eq!(agenda["prompt_heading"], "# SKILL ACTIVA: AGENDA");
    let agenda_tools: Vec<&str> = agenda["tools"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|t| t.as_str())
        .collect();
    assert!(agenda_tools.contains(&"calendar"));

    let core: Vec<&str> = body["core_tools"]
        .as_array()
        .expect("GET /api/skills must return a `core_tools` array")
        .iter()
        .filter_map(|t| t.as_str())
        .collect();
    assert!(
        !core.contains(&"render_widget"),
        "render_widget is routed with the widgets skill, not the core"
    );
    assert!(core.contains(&"get_current_time"));
    assert!(core.contains(&"get_current_location"));
}

#[tokio::test]
async fn test_list_skills_catalog_tools_exist_in_the_production_registry() {
    // Given the production tool registry
    // When every tool of the skills catalog is contrasted with it
    // Then none of them is missing (routing would silently degrade otherwise).
    let app = TestApp::new().await;

    let resp = app.get("/api/skills").await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.json::<serde_json::Value>().await;

    let registry = valet::build_tool_registry(&app.db);
    let registry_names: Vec<String> = registry
        .definitions()
        .iter()
        .map(|def| def.name.clone())
        .collect();

    let mut catalog_tools: Vec<String> = Vec::new();
    for skill in body["skills"].as_array().unwrap() {
        for tool in skill["tools"].as_array().unwrap() {
            let name = tool.as_str().unwrap().to_string();
            if !catalog_tools.contains(&name) {
                catalog_tools.push(name);
            }
        }
    }

    for tool in &catalog_tools {
        assert!(
            registry_names.contains(tool),
            "catalog tool {tool} must exist in the production registry: {registry_names:?}"
        );
    }
}

#[tokio::test]
async fn test_list_skills_exposes_effective_values_without_overrides() {
    // Given a freshly migrated database (no per-skill criterion overrides; the
    //       migration seeds an explicit `ROUTER_THRESHOLD_WIDGETS` equal to the
    //       catalog default)
    // When GET /api/skills is called
    // Then every effective value is the catalog's, and the only marked field is
    //      the widget threshold —an explicit per-skill override— while a raised
    //      global would mark nothing.
    let app = TestApp::new().await;

    let resp = app.get("/api/skills").await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.json::<serde_json::Value>().await;
    let skills = body["skills"].as_array().unwrap();

    for skill in skills {
        let id = skill["id"].as_str().unwrap();
        let spec = catalog()
            .iter()
            .find(|spec| spec.id == id)
            .unwrap_or_else(|| panic!("skill {id} must be in the catalog"));

        assert_eq!(
            skill["question"], spec.instructions,
            "effective question for {id}"
        );
        assert_eq!(
            skill["criteria_true"], spec.criteria_true,
            "effective criteria_true for {id}"
        );
        assert_eq!(
            skill["criteria_false"], spec.criteria_false,
            "effective criteria_false for {id}"
        );
        let threshold = skill["threshold"].as_f64().unwrap();
        assert!(
            (threshold - spec.threshold as f64).abs() < 1e-6,
            "effective threshold for {id}: {threshold} != {}",
            spec.threshold
        );

        let overridden: Vec<&str> = skill["overridden"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str())
            .collect();
        if id == "widgets" {
            assert_eq!(
                overridden,
                vec!["threshold"],
                "the seeded widget override is the only marked field"
            );
        } else {
            assert_eq!(
                overridden,
                Vec::<&str>::new(),
                "nothing is overridden for {id} without settings overrides"
            );
        }
    }
}

#[tokio::test]
async fn test_list_skills_exposes_overridden_values_and_marks_them() {
    // Given an overridden agenda question and an overridden widget threshold
    // When GET /api/skills is called
    // Then those effective values travel and the overridden fields are named
    let app = TestApp::new().await;
    SettingsRepo::set(&app.db, "SKILL_AGENDA_QUESTION", "¿Agenda sobrescrita?")
        .await
        .unwrap();
    SettingsRepo::set(&app.db, "ROUTER_THRESHOLD_WIDGETS", "0.35")
        .await
        .unwrap();

    let resp = app.get("/api/skills").await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.json::<serde_json::Value>().await;
    let skills = body["skills"].as_array().unwrap();

    let agenda = skills
        .iter()
        .find(|s| s["id"] == "agenda")
        .expect("agenda must be in the catalog");
    assert_eq!(
        agenda["question"], "¿Agenda sobrescrita?",
        "the effective question is the overridden one"
    );
    let agenda_overridden: Vec<&str> = agenda["overridden"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    assert!(
        agenda_overridden.contains(&"question"),
        "the overridden question must be named: {agenda_overridden:?}"
    );
    assert!(
        !agenda_overridden.contains(&"criteria_true")
            && !agenda_overridden.contains(&"criteria_false"),
        "untouched criteria must not be marked: {agenda_overridden:?}"
    );

    let widgets = skills
        .iter()
        .find(|s| s["id"] == "widgets")
        .expect("widgets must be in the catalog");
    assert!(
        (widgets["threshold"].as_f64().unwrap() - 0.35).abs() < 1e-6,
        "the widget threshold override must travel: {}",
        widgets["threshold"]
    );
    let widgets_overridden: Vec<&str> = widgets["overridden"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    assert!(
        widgets_overridden.contains(&"threshold"),
        "the overridden threshold must be named: {widgets_overridden:?}"
    );
    assert!(
        !widgets_overridden.contains(&"question"),
        "an untouched question must not be marked: {widgets_overridden:?}"
    );

    // A skill with no overrides keeps the catalog values and no marks.
    let web = skills
        .iter()
        .find(|s| s["id"] == "web")
        .expect("web must be in the catalog");
    assert_eq!(
        web["overridden"].as_array().unwrap().len(),
        0,
        "web has no overrides"
    );
}
