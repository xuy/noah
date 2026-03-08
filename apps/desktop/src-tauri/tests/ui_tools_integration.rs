//! Integration test: verify UI tools register correctly, produce valid output,
//! and the orchestrator's tool definitions include them.

use serde_json::json;

#[test]
fn ui_tools_register_in_router() {
    use itman_desktop_lib::agent::tool_router::ToolRouter;

    let mut router = ToolRouter::new();
    itman_desktop_lib::ui_tools::register_ui_tools(&mut router);

    // All 4 UI tools should be registered
    assert!(router.find_tool("ui_spa").is_some(), "ui_spa not registered");
    assert!(router.find_tool("ui_user_question").is_some(), "ui_user_question not registered");
    assert!(router.find_tool("ui_info").is_some(), "ui_info not registered");
    assert!(router.find_tool("ui_done").is_some(), "ui_done not registered");
}

#[test]
fn ui_tools_are_readonly() {
    use itman_desktop_lib::agent::tool_router::ToolRouter;
    use itman_tools::SafetyTier;

    let mut router = ToolRouter::new();
    itman_desktop_lib::ui_tools::register_ui_tools(&mut router);

    for name in &["ui_spa", "ui_user_question", "ui_info", "ui_done"] {
        let tool = router.find_tool(name).unwrap();
        assert_eq!(
            tool.safety_tier(),
            SafetyTier::ReadOnly,
            "{} should be ReadOnly",
            name
        );
    }
}

#[test]
fn ui_tools_have_valid_schemas() {
    use itman_desktop_lib::agent::tool_router::ToolRouter;

    let mut router = ToolRouter::new();
    itman_desktop_lib::ui_tools::register_ui_tools(&mut router);

    let defs = router.tool_definitions();
    let ui_defs: Vec<_> = defs
        .iter()
        .filter(|d| d.name.starts_with("ui_"))
        .collect();

    assert_eq!(ui_defs.len(), 4, "Expected 4 UI tool definitions");

    for def in &ui_defs {
        assert_eq!(
            def.input_schema["type"].as_str().unwrap(),
            "object",
            "{} schema should be object type",
            def.name
        );
        assert!(
            def.input_schema.get("required").is_some(),
            "{} schema should have required fields",
            def.name
        );
    }
}

#[tokio::test]
async fn ui_spa_tool_executes_correctly() {
    use itman_desktop_lib::agent::tool_router::ToolRouter;

    let mut router = ToolRouter::new();
    itman_desktop_lib::ui_tools::register_ui_tools(&mut router);

    let tool = router.find_tool("ui_spa").unwrap();
    let input = json!({
        "situation_md": "Your DNS cache is stale.",
        "plan_md": "Flush DNS cache to resolve name resolution issues.",
        "action": {
            "label": "Fix it",
            "type": "RUN_STEP"
        }
    });

    let result = tool.execute(&input).await.unwrap();
    let payload: serde_json::Value = serde_json::from_str(&result.output).unwrap();

    assert_eq!(payload["kind"], "spa");
    assert_eq!(payload["situation"], "Your DNS cache is stale.");
    assert_eq!(payload["plan"], "Flush DNS cache to resolve name resolution issues.");
    assert_eq!(payload["action"]["label"], "Fix it");
    assert_eq!(payload["action"]["type"], "RUN_STEP");
    assert!(result.changes.is_empty(), "UI tools should not produce changes");
}

#[tokio::test]
async fn ui_user_question_executes_correctly() {
    use itman_desktop_lib::agent::tool_router::ToolRouter;

    let mut router = ToolRouter::new();
    itman_desktop_lib::ui_tools::register_ui_tools(&mut router);

    let tool = router.find_tool("ui_user_question").unwrap();
    let input = json!({
        "questions": [{
            "header": "Network Type",
            "question_md": "What kind of network are you on?",
            "options": [
                {"label": "Home WiFi", "description": "Personal home network"},
                {"label": "Office", "description": "Corporate network"},
                {"label": "Public", "description": "Coffee shop, airport, etc."}
            ]
        }]
    });

    let result = tool.execute(&input).await.unwrap();
    let payload: serde_json::Value = serde_json::from_str(&result.output).unwrap();

    assert_eq!(payload["kind"], "user_question");
    let questions = payload["questions"].as_array().unwrap();
    assert_eq!(questions.len(), 1);
    assert_eq!(questions[0]["header"], "Network Type");
    assert_eq!(questions[0]["options"].as_array().unwrap().len(), 3);
}

#[tokio::test]
async fn ui_done_executes_correctly() {
    use itman_desktop_lib::agent::tool_router::ToolRouter;

    let mut router = ToolRouter::new();
    itman_desktop_lib::ui_tools::register_ui_tools(&mut router);

    let tool = router.find_tool("ui_done").unwrap();
    let input = json!({"summary_md": "DNS cache flushed. Your internet should be working now."});

    let result = tool.execute(&input).await.unwrap();
    let payload: serde_json::Value = serde_json::from_str(&result.output).unwrap();

    assert_eq!(payload["kind"], "done");
    assert!(payload["summary"].as_str().unwrap().contains("DNS cache flushed"));
}

#[tokio::test]
async fn ui_info_executes_correctly() {
    use itman_desktop_lib::agent::tool_router::ToolRouter;

    let mut router = ToolRouter::new();
    itman_desktop_lib::ui_tools::register_ui_tools(&mut router);

    let tool = router.find_tool("ui_info").unwrap();
    let input = json!({"summary_md": "I can't delete files for safety reasons."});

    let result = tool.execute(&input).await.unwrap();
    let payload: serde_json::Value = serde_json::from_str(&result.output).unwrap();

    assert_eq!(payload["kind"], "info");
    assert!(payload["summary"].as_str().unwrap().contains("safety reasons"));
}

#[tokio::test]
async fn ui_spa_rejects_invalid_action_type() {
    use itman_desktop_lib::agent::tool_router::ToolRouter;

    let mut router = ToolRouter::new();
    itman_desktop_lib::ui_tools::register_ui_tools(&mut router);

    let tool = router.find_tool("ui_spa").unwrap();
    let input = json!({
        "situation_md": "Test",
        "plan_md": "Test",
        "action": {"label": "Test", "type": "INVALID_TYPE"}
    });

    let result = tool.execute(&input).await;
    assert!(result.is_err(), "Invalid action type should fail");
}

#[test]
fn ui_payload_from_tool_call_validates() {
    use itman_desktop_lib::ui_tools::ui_payload_from_tool_call;

    // Valid
    assert!(ui_payload_from_tool_call("ui_spa", &json!({
        "situation_md": "A", "plan_md": "B",
        "action": {"label": "Go", "type": "RUN_STEP"}
    })).is_ok());

    // Missing situation_md
    assert!(ui_payload_from_tool_call("ui_spa", &json!({
        "plan_md": "B",
        "action": {"label": "Go", "type": "RUN_STEP"}
    })).is_err());

    // Missing action
    assert!(ui_payload_from_tool_call("ui_spa", &json!({
        "situation_md": "A", "plan_md": "B"
    })).is_err());

    // Unknown tool
    assert!(ui_payload_from_tool_call("ui_unknown", &json!({})).is_err());
}
