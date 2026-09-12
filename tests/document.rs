//! Configuration safety regressions. All IO is isolated in disposable folders.

use codex_config::doc::Document;
use codex_config::doc::toml_ext::TomlPathExt;
use codex_config::doc::validate::{Severity, validate};
use std::fs;

#[test]
fn builtin_defaults_do_not_require_a_custom_provider() {
    let home = tempfile::tempdir().unwrap();
    fs::write(home.path().join("config.toml"), "# Use built-in defaults\n").unwrap();
    let doc = Document::load(home.path().to_path_buf()).unwrap();
    let issues = validate(&doc);
    assert!(
        issues.iter().all(|issue| issue.severity == Severity::Info),
        "valid built-in defaults should not be presented as broken: {issues:?}"
    );
}

#[test]
fn viewing_a_missing_configuration_does_not_create_files() {
    let home = tempfile::tempdir().unwrap();
    let doc = Document::load(home.path().to_path_buf()).unwrap();
    assert!(!doc.dirty());
    assert!(!home.path().join("config.toml").exists());
}

#[test]
fn invalid_raw_edits_leave_the_previous_document_intact() {
    let home = tempfile::tempdir().unwrap();
    fs::write(home.path().join("config.toml"), "model = \"demo\"\n").unwrap();
    let mut doc = Document::load(home.path().to_path_buf()).unwrap();
    let original = doc.config_text();
    assert!(doc.apply_config_text("model = [").is_err());
    assert_eq!(doc.config_text(), original);
    assert!(doc.apply_catalog_text("{").is_err());
    assert!(doc.catalog.is_none());
}

#[test]
fn saving_preserves_unknown_fields_and_backups_are_original() {
    let home = tempfile::tempdir().unwrap();
    let original =
        "# user notes\nmodel = \"old\" # keep inline note\n[future_feature]\nunknown = 42\n";
    fs::write(home.path().join("config.toml"), original).unwrap();
    let mut doc = Document::load(home.path().to_path_buf()).unwrap();
    doc.config
        .set_value_at(&["model"], toml_edit::Value::from("new"));
    let report = doc.save().unwrap();
    assert_eq!(report.written.len(), 1);
    assert_eq!(report.backups.len(), 1);
    assert_eq!(fs::read_to_string(&report.backups[0]).unwrap(), original);
    let saved = fs::read_to_string(home.path().join("config.toml")).unwrap();
    assert!(saved.contains("# user notes"));
    assert!(saved.contains("# keep inline note"));
    assert!(saved.contains("unknown = 42"));
    assert!(saved.contains("model = \"new\""));
    assert!(!doc.dirty());
    assert!(doc.save().unwrap().written.is_empty());
}

#[test]
fn missing_custom_catalog_is_actionable_but_builtin_catalog_is_not_an_error() {
    let home = tempfile::tempdir().unwrap();
    fs::write(
        home.path().join("config.toml"),
        "model_catalog_json = \"missing.json\"\n",
    )
    .unwrap();
    let doc = Document::load(home.path().to_path_buf()).unwrap();
    assert!(validate(&doc).iter().any(|issue| {
        issue.severity == Severity::Error && issue.page == codex_config::page::Page::Models
    }));
}

#[test]
fn duplicate_model_ids_are_rejected_by_validation() {
    let home = tempfile::tempdir().unwrap();
    fs::write(
        home.path().join("config.toml"),
        "model_catalog_json = \"models.json\"\n",
    )
    .unwrap();
    fs::write(
        home.path().join("models.json"),
        r#"{"models":[{"slug":"same"},{"slug":"same"}]}"#,
    )
    .unwrap();
    let doc = Document::load(home.path().to_path_buf()).unwrap();
    assert!(
        validate(&doc)
            .iter()
            .any(|issue| { issue.severity == Severity::Error && issue.title.contains("重复") })
    );
}

#[test]
fn catalog_formatting_alone_is_not_an_unsaved_change() {
    let home = tempfile::tempdir().unwrap();
    fs::write(
        home.path().join("config.toml"),
        "model_catalog_json = \"models.json\"\n",
    )
    .unwrap();
    fs::write(
        home.path().join("models.json"),
        r#"{"models":[{"slug":"demo"}]}"#,
    )
    .unwrap();
    let mut doc = Document::load(home.path().to_path_buf()).unwrap();
    assert!(
        !doc.dirty(),
        "opening compact JSON must not show a false unsaved warning"
    );
    assert!(doc.save().unwrap().written.is_empty());
}
