use super::*;

#[test]
fn later_commit_failure_restores_the_previous_file_and_cleans_staging() {
    let home = tempfile::tempdir().unwrap();
    let first = home.path().join("models.json");
    let second = home.path().join("config.toml");
    fs::write(&first, "old catalog").unwrap();
    fs::write(&second, "old config").unwrap();
    let changes = [
        Change {
            path: &first,
            text: "new catalog",
            original: Some("old catalog"),
        },
        Change {
            path: &second,
            text: "new config",
            original: Some("old config"),
        },
    ];
    let error = save_changes_with(&changes, |staged, path| {
        if path == second {
            bail!("simulated config replacement failure");
        }
        staged.commit(path)
    })
    .unwrap_err();
    assert!(format!("{error:#}").contains("已回滚"));
    assert_eq!(fs::read_to_string(&first).unwrap(), "old catalog");
    assert_eq!(fs::read_to_string(&second).unwrap(), "old config");
    assert!(
        fs::read_dir(home.path())
            .unwrap()
            .filter_map(Result::ok)
            .all(|entry| !entry.file_name().to_string_lossy().contains(".gui-tmp-"))
    );
}

#[test]
fn later_commit_failure_removes_a_newly_created_catalog() {
    let home = tempfile::tempdir().unwrap();
    let first = home.path().join("models.json");
    let second = home.path().join("config.toml");
    let changes = [
        Change {
            path: &first,
            text: "new catalog",
            original: None,
        },
        Change {
            path: &second,
            text: "new config",
            original: None,
        },
    ];
    assert!(
        save_changes_with(&changes, |staged, path| {
            if path == second {
                bail!("simulated failure");
            }
            staged.commit(path)
        })
        .is_err()
    );
    assert!(!first.exists());
    assert!(!second.exists());
}
