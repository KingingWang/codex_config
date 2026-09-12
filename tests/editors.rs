//! Unit tests for the model, provider, and profile editors.

use codex_config::doc::providers::ProviderView;
use codex_config::doc::schema::WireApi;
use codex_config::editors::{AuthMode, ModelEditor, ProfileEditor, ProviderEditor};
use serde_json::json;

#[test]
fn model_editor_reads_and_writes_slug() {
    let model = json!({"slug": "gpt-5", "display_name": "GPT-5"});
    let editor = ModelEditor::from_value(&model);
    assert_eq!(editor.slug, "gpt-5");
    assert_eq!(editor.display_name, "GPT-5");
}

#[test]
fn model_editor_uses_slug_as_fallback_display_name() {
    let model = json!({"slug": "claude-opus"});
    let editor = ModelEditor::from_value(&model);
    assert_eq!(editor.display_name, "");
}

#[test]
fn model_editor_writes_back_to_json() {
    let mut model = json!({"slug": "old"});
    let mut editor = ModelEditor::from_value(&model);
    editor.slug = "new".to_string();
    editor.context_window = "200000".to_string();
    editor.write_to(&mut model);
    assert_eq!(model.get("slug").and_then(|v| v.as_str()), Some("new"));
    assert_eq!(
        model.get("context_window").and_then(|v| v.as_i64()),
        Some(200000)
    );
}

#[test]
fn model_editor_removes_empty_fields() {
    let mut model = json!({"slug": "test", "description": "remove me"});
    let mut editor = ModelEditor::from_value(&model);
    editor.description = String::new();
    editor.write_to(&mut model);
    assert!(model.get("description").is_none() || model.get("description").unwrap().is_null());
}

#[test]
fn model_editor_detects_invalid_slug() {
    let mut editor = ModelEditor::default();
    assert!(editor.problems().iter().any(|p| p.contains("slug")));

    editor.slug = "has space".to_string();
    assert!(editor.problems().iter().any(|p| p.contains("空格")));
}

#[test]
fn model_editor_detects_invalid_context_window() {
    let editor = ModelEditor {
        slug: "valid".into(),
        context_window: "not a number".into(),
        ..Default::default()
    };
    assert!(editor.problems().iter().any(|p| p.contains("上下文窗口")));
}

#[test]
fn model_editor_detects_reasoning_level_mismatch() {
    let editor = ModelEditor {
        slug: "test".into(),
        supported_reasoning_levels: vec!["low".into(), "high".into()],
        default_reasoning_level: "medium".into(),
        ..Default::default()
    };
    assert!(editor.problems().iter().any(|p| p.contains("默认推理强度")));
}

#[test]
fn auth_mode_labels_are_chinese() {
    assert!(AuthMode::EnvKey.label().contains("环境变量"));
    assert!(AuthMode::Bearer.label().contains("配置文件"));
    assert!(AuthMode::CodexLogin.label().contains("Codex"));
    assert!(AuthMode::None.label().contains("不需要"));
}

#[test]
fn provider_editor_reads_view() {
    let view = ProviderView {
        id: "test".to_string(),
        name: "Test Provider".to_string(),
        base_url: "https://api.test.com/v1".to_string(),
        wire_api: "chat".to_string(),
        env_key: "TEST_KEY".to_string(),
        ..Default::default()
    };
    let editor = ProviderEditor::from_view(&view);
    assert_eq!(editor.id, "test");
    assert_eq!(editor.name, "Test Provider");
    assert_eq!(editor.base_url, "https://api.test.com/v1");
    assert_eq!(editor.wire_api, WireApi::Chat);
    assert_eq!(editor.auth_mode, AuthMode::EnvKey);
}

#[test]
fn provider_editor_detects_missing_base_url() {
    let editor = ProviderEditor::from_view(&ProviderView {
        id: "test".to_string(),
        ..Default::default()
    });
    assert!(editor.problems().iter().any(|p| p.contains("base_url")));
}

#[test]
fn provider_editor_detects_invalid_url_scheme() {
    let editor = ProviderEditor::from_view(&ProviderView {
        id: "test".to_string(),
        base_url: "invalid-url".to_string(),
        ..Default::default()
    });
    assert!(editor.problems().iter().any(|p| p.contains("http")));
}

#[test]
fn provider_editor_detects_missing_env_key() {
    // Create editor with auth_mode set to EnvKey but empty env_key
    let mut editor = ProviderEditor::from_view(&ProviderView {
        id: "test".to_string(),
        base_url: "https://api.test.com".to_string(),
        ..Default::default()
    });
    editor.auth_mode = AuthMode::EnvKey;
    editor.env_key = String::new();
    editor.env_key = String::new(); // Clear the default key
    assert!(editor.problems().iter().any(|p| p.contains("变量名")));
}

#[test]
fn provider_editor_detects_missing_bearer_token() {
    let mut editor = ProviderEditor::from_view(&ProviderView {
        id: "test".to_string(),
        base_url: "https://api.test.com".to_string(),
        ..Default::default()
    });
    editor.auth_mode = AuthMode::Bearer;
    assert!(editor.problems().iter().any(|p| p.contains("Token")));
}

#[test]
fn profile_editor_reads_and_writes() {
    use toml_edit::DocumentMut;
    let config_text = r#"
[profiles.dev]
model = "gpt-5"
model_provider = "openai"
"#;
    let config: DocumentMut = config_text.parse().unwrap();
    let editor = ProfileEditor::from_config(&config, "dev");
    assert_eq!(editor.model, "gpt-5");
    assert_eq!(editor.model_provider, "openai");
}
