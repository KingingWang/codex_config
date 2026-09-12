use codex_config::doc::Document;
use codex_config::doc::toml_ext::TomlPathExt;
use codex_config::remote::{Snapshot, resolve_path, valid_alias};

fn snapshot() -> Snapshot {
    Snapshot {
        home: "/remote/user/.codex".into(),
        user_home: "/remote/user".into(),
        config: Some("model_catalog_json = \"models.json\"\n# preserve me\n".into()),
        catalog_path: Some("/remote/user/.codex/models.json".into()),
        catalog: Some("{\"models\":[]}".into()),
    }
}

#[test]
fn remote_snapshot_never_uses_local_saving_or_home_expansion() {
    let mut doc = Document::from_remote(snapshot()).unwrap();
    assert!(doc.is_remote());
    assert!(!doc.dirty());
    assert_eq!(
        doc.resolve_against_home("~/catalog.json").to_string_lossy(),
        "/remote/user/catalog.json"
    );
    doc.config
        .set_value_at(&["model"], toml_edit::Value::from("new-model"));
    assert!(doc.save().unwrap_err().to_string().contains("禁止"));
    assert!(doc.dirty());
    assert!(
        doc.remote_files().unwrap()[0]
            .text
            .contains("# preserve me")
    );
    doc.mark_remote_saved();
    assert!(!doc.dirty());
}

#[test]
fn remote_catalog_repair_keeps_exact_bad_original_for_backup() {
    let mut data = snapshot();
    data.catalog = Some("{bad-json".into());
    let mut doc = Document::from_remote(data).unwrap();
    assert_eq!(doc.catalog_on_disk(), "{bad-json");
    doc.apply_catalog_text("{\"models\":[]}").unwrap();
    let files = doc.remote_files().unwrap();
    assert_eq!(files[1].original.as_deref(), Some("{bad-json"));
    assert!(files[1].write);
    assert!(doc.load_notes.is_empty());
}

#[test]
fn remote_catalog_changes_need_explicit_remote_loading() {
    let mut doc = Document::from_remote(snapshot()).unwrap();
    assert!(doc.create_catalog("models.json").is_err());
    assert!(
        doc.apply_config_text("model_catalog_json='other.json'")
            .is_err()
    );
    assert_eq!(
        doc.config.str_at(&["model_catalog_json"]).as_deref(),
        Some("models.json")
    );
    let mut data = snapshot();
    data.catalog_path = Some("/remote/user/.codex/other.json".into());
    doc.replace_remote_catalog("other.json", data).unwrap();
    assert!(doc.config_dirty());
    assert!(!doc.catalog_dirty());
    doc.create_catalog("new.json").unwrap();
    assert!(doc.catalog_dirty());
    assert!(doc.remote_files().unwrap()[1].original.is_none());
}

#[test]
fn invalid_or_mismatched_remote_paths_cannot_be_saved() {
    assert!(resolve_path("/work", "/user", "~other/file").is_err());
    assert!(resolve_path("/work", "/user", "bad\0path").is_err());
    assert_eq!(
        resolve_path("/work", "/user", "../中文 file;$(x)").unwrap(),
        "/中文 file;$(x)"
    );
    let mut data = snapshot();
    data.catalog_path = Some("/different.json".into());
    assert!(Document::from_remote(data).is_err());
    let mut doc = Document::from_remote(snapshot()).unwrap();
    doc.config.set_value_at(
        &["model_catalog_json"],
        toml_edit::Value::from("config.toml"),
    );
    assert!(doc.remote_files().is_err());
}

#[test]
fn destinations_cannot_inject_ssh_options_or_shell_commands() {
    for alias in ["dev", "prod-server", "my_box.2"] {
        assert!(valid_alias(alias));
    }
    for alias in [
        "",
        "-oProxyCommand=bad",
        "box;touch pwned",
        "$(id)",
        "a b",
        "*.host",
        "!x",
        "a\nb",
    ] {
        assert!(!valid_alias(alias));
    }
}

// The exact same Python helper used on remote hosts is exercised locally in
// isolated directories. These tests do not invoke SSH or inspect personal files.
#[cfg(unix)]
mod helper {
    use serde_json::{Value, json};
    use std::fs;
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;
    use std::process::{Command, Stdio};

    fn invoke(request: Value, prefix: &str) -> Value {
        let mut child = Command::new("python3")
            .arg("-c")
            .arg(format!("{prefix}\n{}", codex_config::remote::HELPER))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(&serde_json::to_vec(&request).unwrap())
            .unwrap();
        let output = child.wait_with_output().unwrap();
        serde_json::from_slice(&output.stdout)
            .unwrap_or_else(|_| panic!("{}", String::from_utf8_lossy(&output.stderr)))
    }

    fn file(path: &Path, original: Option<&str>, text: &str, write: bool) -> Value {
        json!({"path":path, "original":original, "text":text, "write":write})
    }

    #[test]
    fn reads_and_saves_with_private_exact_backups_and_hostile_paths() {
        let root = tempfile::tempdir().unwrap();
        let home = root.path().join("中文 ' $(touch never) ; space");
        fs::create_dir(&home).unwrap();
        let config = home.join("config.toml");
        fs::write(&config, "# original\n").unwrap();
        let response = invoke(json!({"operation":"load", "home":home}), "");
        assert_eq!(response["result"]["config"], "# original\n");
        let catalog = home.join("models.json");
        let response = invoke(
            json!({"operation":"save", "files":[
                file(&catalog, None, "{\"models\":[]}", true),
                file(&config, Some("# original\n"), "# changed\n", true),
            ]}),
            "",
        );
        assert!(response.get("error").is_none(), "{response}");
        assert_eq!(response["result"]["written"].as_array().unwrap().len(), 2);
        let backup = response["result"]["backups"][0].as_str().unwrap();
        assert_eq!(fs::read_to_string(backup).unwrap(), "# original\n");
        assert_eq!(
            fs::metadata(backup).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(
            fs::metadata(&config).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(fs::read_to_string(&catalog).unwrap(), "{\"models\":[]}");
        assert!(!home.join("never").exists());
    }

    #[test]
    fn conflicts_in_unchanged_companion_and_missing_vs_empty_block_all_writes() {
        let root = tempfile::tempdir().unwrap();
        let config = root.path().join("config.toml");
        let catalog = root.path().join("models.json");
        fs::write(&config, "external change").unwrap();
        fs::write(&catalog, "before").unwrap();
        let response = invoke(
            json!({"operation":"save","files":[
                file(&catalog, Some("before"), "after", true),
                file(&config, Some("original"), "original", false),
            ]}),
            "",
        );
        assert!(response["error"].as_str().unwrap().contains("其他程序"));
        assert_eq!(fs::read_to_string(&catalog).unwrap(), "before");
        fs::write(&config, "").unwrap();
        let response = invoke(
            json!({"operation":"save","files":[file(&config, None, "new", true)]}),
            "",
        );
        assert!(response.get("error").is_some());
        assert_eq!(fs::read_to_string(&config).unwrap(), "");
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 2);
    }

    #[test]
    fn second_publish_failure_rolls_back_and_cleans_staged_files() {
        let root = tempfile::tempdir().unwrap();
        let config = root.path().join("config.toml");
        let catalog = root.path().join("models.json");
        fs::write(&config, "original").unwrap();
        fs::write(&catalog, "old-catalog").unwrap();
        let injection = r#"
import os
real_replace = os.replace
calls = 0
def fail_second(source, target):
    global calls
    calls += 1
    if calls == 2:
        raise OSError("injected publish failure")
    return real_replace(source, target)
os.replace = fail_second
"#;
        let response = invoke(
            json!({"operation":"save","files":[
                file(&catalog, Some("old-catalog"), "new-catalog", true),
                file(&config, Some("original"), "changed", true),
            ]}),
            injection,
        );
        assert!(response["error"].as_str().unwrap().contains("injected"));
        assert_eq!(fs::read_to_string(config).unwrap(), "original");
        assert_eq!(fs::read_to_string(catalog).unwrap(), "old-catalog");
        for entry in fs::read_dir(root.path()).unwrap() {
            assert!(
                !entry
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .contains("gui-tmp")
            );
        }
    }

    #[test]
    fn symlinks_and_same_file_catalog_are_rejected() {
        let root = tempfile::tempdir().unwrap();
        let real = root.path().join("real.toml");
        let config = root.path().join("config.toml");
        fs::write(&real, "untouched").unwrap();
        std::os::unix::fs::symlink(&real, &config).unwrap();
        let response = invoke(json!({"operation":"load","home":root.path()}), "");
        assert!(response["error"].as_str().unwrap().contains("符号链接"));
        let response = invoke(
            json!({"operation":"save","files":[file(&config, Some("untouched"), "new", true)]}),
            "",
        );
        assert!(response.get("error").is_some());
        assert_eq!(fs::read_to_string(real).unwrap(), "untouched");
        let actual = root.path().join("actual");
        let alias = root.path().join("hardlink");
        fs::write(&actual, "same").unwrap();
        fs::hard_link(&actual, &alias).unwrap();
        let response = invoke(
            json!({"operation":"save","files":[
                file(&actual, Some("same"), "changed", true),
                file(&alias, Some("same"), "changed-too", true),
            ]}),
            "",
        );
        assert!(response["error"].as_str().unwrap().contains("同一文件"));
        assert_eq!(fs::read_to_string(actual).unwrap(), "same");
    }

    #[test]
    fn retention_keeps_twenty_own_backups_and_never_prunes_manual_files() {
        let root = tempfile::tempdir().unwrap();
        let config = root.path().join("config.toml");
        fs::write(&config, "original").unwrap();
        for index in 0..25 {
            fs::write(
                root.path()
                    .join(format!("config.toml.bak-20200101-000000-123456789-{index}")),
                index.to_string(),
            )
            .unwrap();
        }
        let manual = root.path().join("config.toml.bak-manual");
        fs::write(&manual, "manual").unwrap();
        let other = root.path().join("other.json.bak-20200101-000000-123456789");
        fs::write(&other, "other").unwrap();
        let response = invoke(
            json!({"operation":"save", "files":[file(&config, Some("original"), "updated", true)]}),
            "",
        );
        assert!(response.get("error").is_none(), "{response}");
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 23); // config, 20 backups, 2 unrelated
        assert!(
            root.path()
                .join("config.toml.bak-20200101-000000-123456789-24")
                .exists()
        );
        assert!(
            !root
                .path()
                .join("config.toml.bak-20200101-000000-123456789-0")
                .exists()
        );
        assert_eq!(fs::read_to_string(manual).unwrap(), "manual");
        assert_eq!(fs::read_to_string(other).unwrap(), "other");
    }

    #[test]
    fn load_refuses_changed_config_between_two_reads() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("config.toml"), "changed").unwrap();
        let result = invoke(
            json!({
                "operation":"load","home":root.path(),"catalog":"models.json",
                "check_config":true,"expected_config":"before",
            }),
            "",
        );
        assert!(result["error"].as_str().unwrap().contains("其他程序"));
    }
}
