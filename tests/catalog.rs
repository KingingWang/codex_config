//! Unit tests for the model catalog JSON helpers.

use codex_config::doc::catalog;
use serde_json::json;

#[test]
fn models_returns_array() {
    let doc = json!({"models": [{"slug": "a"}, {"slug": "b"}]});
    let list = catalog::models(&doc).unwrap();
    assert_eq!(list.len(), 2);
}

#[test]
fn models_returns_none_for_missing_key() {
    let doc = json!({});
    assert!(catalog::models(&doc).is_none());
}

#[test]
fn slugs_extracts_ids() {
    let doc = json!({"models": [{"slug": "gpt-5"}, {"slug": "claude"}]});
    let slugs = catalog::slugs(&doc);
    assert_eq!(slugs, vec!["gpt-5", "claude"]);
}

#[test]
fn display_name_uses_display_name_when_present() {
    let model = json!({"slug": "gpt-5", "display_name": "GPT-5 Codex"});
    assert_eq!(catalog::display_name(&model), "GPT-5 Codex");
}

#[test]
fn display_name_falls_back_to_slug() {
    let model = json!({"slug": "claude-opus"});
    assert_eq!(catalog::display_name(&model), "claude-opus");
}

#[test]
fn slug_of_extracts_slug() {
    let model = json!({"slug": "test-model"});
    assert_eq!(catalog::slug_of(&model), "test-model");
}

#[test]
fn find_index_locates_model() {
    let doc = json!({"models": [{"slug": "a"}, {"slug": "b"}, {"slug": "c"}]});
    assert_eq!(catalog::find_index(&doc, "b"), Some(1));
    assert_eq!(catalog::find_index(&doc, "missing"), None);
}

#[test]
fn get_and_set_path() {
    let mut doc = json!({"a": {"b": 1}});
    assert_eq!(
        catalog::get(&doc, &["a", "b"]).and_then(|v| v.as_i64()),
        Some(1)
    );
    catalog::set(&mut doc, &["a", "b"], json!(2));
    assert_eq!(
        doc.get("a").unwrap().get("b").and_then(|v| v.as_i64()),
        Some(2)
    );
}

#[test]
fn set_creates_nested_path() {
    let mut doc = json!({});
    catalog::set(&mut doc, &["a", "b", "c"], json!(42));
    assert_eq!(
        doc.get("a")
            .unwrap()
            .get("b")
            .unwrap()
            .get("c")
            .and_then(|v| v.as_i64()),
        Some(42)
    );
}

#[test]
fn remove_deletes_key() {
    let mut doc = json!({"a": {"b": 1, "c": 2}});
    let removed = catalog::remove(&mut doc, &["a", "b"]);
    assert_eq!(removed, Some(json!(1)));
    assert!(doc.get("a").unwrap().get("b").is_none());
}

#[test]
fn str_at_extracts_string() {
    let doc = json!({"a": {"b": "hello"}});
    assert_eq!(
        catalog::str_at(&doc, &["a", "b"]),
        Some("hello".to_string())
    );
}

#[test]
fn bool_at_extracts_bool() {
    let doc = json!({"enabled": true, "disabled": false});
    assert_eq!(catalog::bool_at(&doc, &["enabled"]), Some(true));
    assert_eq!(catalog::bool_at(&doc, &["disabled"]), Some(false));
}

#[test]
fn i64_at_extracts_int() {
    let doc = json!({"count": 42});
    assert_eq!(catalog::i64_at(&doc, &["count"]), Some(42));
}

#[test]
fn str_vec_at_extracts_string_array() {
    let doc = json!({"tags": ["a", "b", "c"]});
    assert_eq!(catalog::str_vec_at(&doc, &["tags"]), vec!["a", "b", "c"]);
}

#[test]
fn reasoning_levels_extracts_efforts() {
    let model = json!({
        "supported_reasoning_levels": [
            {"effort": "low", "description": "Fast"},
            {"effort": "high", "description": "Deep"}
        ]
    });
    let levels = catalog::reasoning_levels(&model);
    assert_eq!(levels, vec!["low", "high"]);
}

#[test]
fn set_reasoning_levels_creates_proper_structure() {
    let mut model = json!({});
    catalog::set_reasoning_levels(&mut model, &["low".to_string(), "high".to_string()]);
    let levels = model
        .get("supported_reasoning_levels")
        .unwrap()
        .as_array()
        .unwrap();
    assert_eq!(levels.len(), 2);
    assert_eq!(levels[0].get("effort").unwrap().as_str(), Some("low"));
    assert!(
        levels[0]
            .get("description")
            .unwrap()
            .as_str()
            .unwrap()
            .contains("轻量")
    );
}

#[test]
fn new_model_template_has_required_fields() {
    let model = catalog::new_model_template("test-slug", "Test Model", Some("test-provider"));
    assert_eq!(
        model.get("slug").and_then(|v| v.as_str()),
        Some("test-slug")
    );
    assert_eq!(
        model.get("display_name").and_then(|v| v.as_str()),
        Some("Test Model")
    );
    assert_eq!(
        model.get("provider").and_then(|v| v.as_str()),
        Some("test-provider")
    );
    assert_eq!(
        model.get("visibility").and_then(|v| v.as_str()),
        Some("list")
    );
    assert!(
        model
            .get("supported_in_api")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    );
}

#[test]
fn new_model_template_without_provider() {
    let model = catalog::new_model_template("test-slug", "Test", None);
    // Provider should be null or absent
    let provider = model.get("provider");
    assert!(provider.is_none() || provider.unwrap().is_null());
}
