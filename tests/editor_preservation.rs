use codex_config::doc::providers;
use codex_config::editors::{ModelEditor, ProviderEditor};
use serde_json::json;

#[test]
fn changing_model_name_preserves_untouched_metadata_exactly() {
    let mut model = json!({
        "slug": "demo", "display_name": "Before",
        "supported_reasoning_levels": [
            {"effort":"high", "description":"user description", "future":42}
        ],
        "base_instructions": "  keep leading and trailing whitespace\n",
        "availability_nux": {"future":true},
        "future": {"nested":42}
    });
    let mut expected = model.clone();
    expected["display_name"] = json!("After");
    let mut editor = ModelEditor::from_value(&model);
    editor.display_name = "After".into();
    editor.write_to(&mut model);
    assert_eq!(model, expected);
}

#[test]
fn writing_an_unchanged_model_editor_is_a_noop() {
    let mut model = json!({"slug":"demo", "provider":null, "context_window":null});
    let original = model.clone();
    ModelEditor::from_value(&model).write_to(&mut model);
    assert_eq!(model, original);
}

#[test]
fn changing_provider_name_preserves_untouched_auth_and_explicit_false() {
    let mut config: toml_edit::DocumentMut = r#"
[model_providers.demo]
name = "Before"
base_url = "https://example.invalid/v1"
wire_api = "future-wire"
supports_websockets = false
requires_openai_auth = false
http_headers = { "X-Custom" = "preserve" }
[model_providers.demo.auth]
command = "example-token-command"
future = "keep"
"#
    .parse()
    .unwrap();
    let original = config.to_string();
    let view = providers::view(&config, "demo").unwrap();
    let mut editor = ProviderEditor::from_view(&view);
    editor.name = "After".into();
    editor.write_to(&mut config);
    assert_eq!(
        config.to_string(),
        original.replace("\"Before\"", "\"After\"")
    );
}
