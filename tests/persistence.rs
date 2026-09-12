//! Regressions for saving without overwriting unrelated or concurrent edits.

use codex_config::doc::Document;
use codex_config::doc::toml_ext::TomlPathExt;
use std::fs;

fn edit_model(doc: &mut Document, model: &str) {
    doc.config
        .set_value_at(&["model"], toml_edit::Value::from(model));
}

#[test]
fn concurrent_config_edits_are_not_overwritten() {
    let home = tempfile::tempdir().unwrap();
    let path = home.path().join("config.toml");
    fs::write(&path, "model = \"before\"\n").unwrap();
    let mut doc = Document::load(home.path().to_owned()).unwrap();
    edit_model(&mut doc, "ours");
    fs::write(&path, "model = \"theirs\"\n").unwrap();
    assert!(doc.save().is_err());
    assert_eq!(fs::read_to_string(&path).unwrap(), "model = \"theirs\"\n");
    assert!(doc.dirty());
}

#[test]
fn a_newly_created_empty_file_is_also_a_conflict() {
    let home = tempfile::tempdir().unwrap();
    let mut doc = Document::load(home.path().to_owned()).unwrap();
    edit_model(&mut doc, "ours");
    fs::write(home.path().join("config.toml"), "").unwrap();
    assert!(doc.save().is_err());
    assert_eq!(
        fs::read_to_string(home.path().join("config.toml")).unwrap(),
        ""
    );
}

#[test]
fn concurrent_catalog_edits_block_both_writes() {
    let home = tempfile::tempdir().unwrap();
    let original = "model_catalog_json = \"models.json\"\n";
    fs::write(home.path().join("config.toml"), original).unwrap();
    fs::write(home.path().join("models.json"), r#"{"models":[]}"#).unwrap();
    let mut doc = Document::load(home.path().to_owned()).unwrap();
    edit_model(&mut doc, "ours");
    doc.apply_catalog_text(r#"{"models":[], "ours":true}"#)
        .unwrap();
    fs::write(
        home.path().join("models.json"),
        r#"{"models":[], "theirs":true}"#,
    )
    .unwrap();
    assert!(doc.save().is_err());
    assert_eq!(
        fs::read_to_string(home.path().join("config.toml")).unwrap(),
        original
    );
    assert!(doc.config_dirty() && doc.catalog_dirty());
}

#[test]
fn catalog_staging_failure_does_not_save_the_config_pointer() {
    let home = tempfile::tempdir().unwrap();
    let original = "# unchanged\n";
    fs::write(home.path().join("config.toml"), original).unwrap();
    let mut doc = Document::load(home.path().to_owned()).unwrap();
    doc.create_catalog("blocked/models.json").unwrap();
    fs::write(home.path().join("blocked"), "not a directory").unwrap();
    assert!(doc.save().is_err());
    assert_eq!(
        fs::read_to_string(home.path().join("config.toml")).unwrap(),
        original
    );
    assert!(doc.config_dirty() && doc.catalog_dirty());
}

#[test]
fn backup_retention_is_scoped_to_each_saved_file() {
    let home = tempfile::tempdir().unwrap();
    let nested = home.path().join("nested");
    fs::create_dir(&nested).unwrap();
    fs::write(
        home.path().join("config.toml"),
        "model_catalog_json = \"nested/models.json\"\n",
    )
    .unwrap();
    fs::write(nested.join("models.json"), r#"{"models":[]}"#).unwrap();
    let unrelated = home.path().join("important.bak-20000101-000000");
    fs::write(&unrelated, "not owned by the editor").unwrap();
    let unrelated_named = home.path().join("config.toml.bak-manual");
    fs::write(&unrelated_named, "manual backup").unwrap();
    let mut doc = Document::load(home.path().to_owned()).unwrap();
    for index in 0..23 {
        edit_model(&mut doc, &format!("model-{index}"));
        doc.apply_catalog_text(&format!(r#"{{"models":[],"iteration":{index}}}"#))
            .unwrap();
        let report = doc.save().unwrap();
        assert!(
            report.backups.iter().all(|path| path.exists()),
            "the newly created backups must survive retention"
        );
    }
    let count_backups = |dir: &std::path::Path, prefix: &str| {
        fs::read_dir(dir)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| e.file_name().to_string_lossy().starts_with(prefix))
            .count()
    };
    assert!(unrelated.exists());
    assert!(unrelated_named.exists());
    assert_eq!(count_backups(home.path(), "config.toml.bak-"), 21);
    assert_eq!(count_backups(&nested, "models.json.bak-"), 20);
}

#[test]
fn repairing_invalid_catalog_backs_up_the_original_bytes() {
    let home = tempfile::tempdir().unwrap();
    fs::write(
        home.path().join("config.toml"),
        "model_catalog_json = \"models.json\"\n",
    )
    .unwrap();
    fs::write(home.path().join("models.json"), "{ broken json").unwrap();
    let mut doc = Document::load(home.path().to_owned()).unwrap();
    assert_eq!(doc.catalog_text(), "{ broken json");
    doc.apply_catalog_text(r#"{"models":[]}"#).unwrap();
    let report = doc.save().unwrap();
    assert_eq!(report.backups.len(), 1);
    assert_eq!(
        fs::read_to_string(&report.backups[0]).unwrap(),
        "{ broken json"
    );
}

#[test]
fn changing_catalog_path_cannot_drop_pending_model_edits() {
    let home = tempfile::tempdir().unwrap();
    let mut doc = Document::load(home.path().to_owned()).unwrap();
    doc.create_catalog("first.json").unwrap();
    let original = doc.config_text();
    assert!(
        doc.apply_config_text("model_catalog_json = \"second.json\"\n")
            .is_err()
    );
    assert_eq!(doc.config_text(), original);
    assert_eq!(doc.catalog_path, Some(home.path().join("first.json")));
    assert!(doc.create_catalog("second.json").is_err());
}

#[test]
fn raw_config_path_changes_reload_catalog_in_the_document_layer() {
    let home = tempfile::tempdir().unwrap();
    fs::write(home.path().join("models.json"), r#"{"models":[]}"#).unwrap();
    let mut doc = Document::load(home.path().to_owned()).unwrap();
    doc.apply_config_text("model_catalog_json = \"models.json\"\n")
        .unwrap();
    assert_eq!(doc.catalog_path, Some(home.path().join("models.json")));
    assert!(doc.catalog.is_some());
    assert!(!doc.catalog_dirty());
}

#[test]
fn catalog_edits_without_a_destination_are_rejected() {
    let home = tempfile::tempdir().unwrap();
    let mut doc = Document::load(home.path().to_owned()).unwrap();
    assert!(doc.apply_catalog_text(r#"{"models":[]}"#).is_err());
    assert!(doc.catalog.is_none());
}

#[cfg(unix)]
#[test]
fn a_preexisting_temporary_symlink_is_never_followed() {
    let home = tempfile::tempdir().unwrap();
    let unrelated = home.path().join("private.txt");
    fs::write(&unrelated, "do not touch").unwrap();
    std::os::unix::fs::symlink(&unrelated, home.path().join(".config.toml.gui-tmp")).unwrap();
    let mut doc = Document::load(home.path().to_owned()).unwrap();
    edit_model(&mut doc, "new");
    doc.save().unwrap();
    assert_eq!(fs::read_to_string(&unrelated).unwrap(), "do not touch");
}

#[cfg(unix)]
#[test]
fn saving_a_symlinked_config_fails_without_replacing_the_link() {
    let home = tempfile::tempdir().unwrap();
    let target = home.path().join("shared.toml");
    fs::write(&target, "# original\n").unwrap();
    let config = home.path().join("config.toml");
    std::os::unix::fs::symlink(&target, &config).unwrap();
    let mut doc = Document::load(home.path().to_owned()).unwrap();
    edit_model(&mut doc, "new");
    assert!(doc.save().is_err());
    assert!(
        fs::symlink_metadata(&config)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(fs::read_to_string(&target).unwrap(), "# original\n");
}
