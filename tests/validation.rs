use codex_config::doc::Document;
use codex_config::doc::validate::{Severity, validate};
use std::fs;

fn load(config: &str, catalog: Option<&str>) -> (Document, tempfile::TempDir) {
    let home = tempfile::tempdir().unwrap();
    fs::write(home.path().join("config.toml"), config).unwrap();
    if let Some(json) = catalog {
        fs::write(home.path().join("models.json"), json).unwrap();
    }
    let path = home.path().to_path_buf();
    (Document::load(path).unwrap(), home)
}

fn has_error(doc: &Document, text: &str) -> bool {
    validate(doc)
        .iter()
        .any(|i| i.severity == Severity::Error && i.title.contains(text))
}

#[test]
fn validates_profiles_and_effective_overrides() {
    let (missing, _) = load("profile = \"missing\"\n", None);
    assert!(has_error(&missing, "profile"));
    let (doc, _) = load(
        "model_catalog_json = \"models.json\"\nmodel = \"top\"\nmodel_provider = \"missing-top\"\nprofile = \"work\"\n[profiles.work]\nmodel = \"override\"\nmodel_provider = \"missing-profile\"\nmodel_reasoning_effort = \"high\"\n",
        Some(
            r#"{"models":[{"slug":"override","supported_reasoning_levels":[{"effort":"low","description":"low"}]}]}"#,
        ),
    );
    assert!(has_error(&doc, "服务商"));
    assert!(validate(&doc).iter().any(|i| i.title.contains("模型")));
    assert!(
        !validate(&doc)
            .iter()
            .any(|i| i.detail.contains("missing-top"))
    );
}

#[test]
fn accepts_inline_profiles_but_rejects_scalar_profiles() {
    let (inline, _) = load(
        "profile = \"work\"\nprofiles = { work = { model_provider = \"ollama\" } }\n",
        None,
    );
    assert!(!has_error(&inline, "profile"));
    let (scalar, _) = load("profiles = { work = \"bad\" }\n", None);
    assert!(has_error(&scalar, "profile"));
}

#[test]
fn validates_catalog_shape_entries_and_slugs() {
    let (root, _) = load("model_catalog_json = \"models.json\"\n", Some(r#"{}"#));
    assert!(has_error(&root, "models"));
    let (doc, _) = load(
        "model_catalog_json = \"models.json\"\n",
        Some(r#"{"models":[1,{"slug":""},{"slug":"ok"},{"slug":"ok"}]}"#),
    );
    assert!(has_error(&doc, "不是对象"));
    assert!(has_error(&doc, "缺少 slug"));
    assert!(has_error(&doc, "重复"));
}

#[test]
fn preserves_custom_reasoning_values_and_metadata() {
    let (doc, _) = load(
        "model_catalog_json = \"models.json\"\nmodel = \"custom\"\nmodel_reasoning_effort = \"custom-effort\"\n",
        Some(
            r#"{"future":true,"models":[{"slug":"custom","future_field":42,"supported_reasoning_levels":[{"effort":"custom-effort","description":"x"}]}]}"#,
        ),
    );
    assert!(!validate(&doc).iter().any(|i| i.title.contains("推理强度")));
    let model = &doc.catalog.as_ref().unwrap()["models"][0];
    assert_eq!(model["future_field"], 42);
}
