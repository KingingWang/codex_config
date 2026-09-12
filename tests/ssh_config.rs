use codex_config::ssh_config::discover_with_home;
use std::fs;

#[test]
fn reads_literal_aliases_quotes_equals_comments_and_deduplicates() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("config");
    fs::write(
        &path,
        r#"
# never execute this configuration during discovery
Host=dev prod "my-box" *.company !disabled foo?bar
  HostName private.example
HOST dev stage # duplicate and trailing comment
Match exec "touch should-not-exist"
  User irrelevant
Host "-oProxyCommand=bad" "bad;command" "bad alias"
Host final
"#,
    )
    .unwrap();
    let result = discover_with_home(&path, root.path());
    assert_eq!(result.aliases, ["dev", "final", "my-box", "prod", "stage"]);
    assert!(result.warnings.is_empty());
    assert!(!root.path().join("should-not-exist").exists());
}

#[test]
fn expands_includes_from_ssh_home_with_globs_spaces_and_cycles() {
    let root = tempfile::tempdir().unwrap();
    let ssh = root.path().join(".ssh");
    fs::create_dir_all(ssh.join("hosts")).unwrap();
    fs::write(
        ssh.join("config"),
        "Include hosts/*.conf \"with spaces\"\nHost local\n",
    )
    .unwrap();
    fs::write(ssh.join("hosts/a.conf"), "Host alpha\nInclude config\n").unwrap();
    fs::write(
        ssh.join("hosts/b.conf"),
        "Host beta\nInclude ~/.ssh/hosts/a.conf\n",
    )
    .unwrap();
    fs::write(ssh.join("with spaces"), "Host spaced\n").unwrap();
    fs::write(ssh.join("hosts/ignored.txt"), "Host ignored\n").unwrap();
    let result = discover_with_home(&ssh.join("config"), root.path());
    assert_eq!(result.aliases, ["alpha", "beta", "local", "spaced"]);
    assert!(result.warnings.is_empty(), "{:?}", result.warnings);
}

#[test]
fn custom_config_still_resolves_relative_includes_against_ssh_home() {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join(".ssh")).unwrap();
    fs::write(root.path().join("custom"), "Include servers\n").unwrap();
    fs::write(root.path().join(".ssh/servers"), "Host included\n").unwrap();
    let result = discover_with_home(&root.path().join("custom"), root.path());
    assert_eq!(result.aliases, ["included"]);
}

#[test]
fn missing_malformed_and_oversized_files_are_reported_without_panicking() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("config");
    assert!(!discover_with_home(&path, root.path()).warnings.is_empty());
    fs::write(
        &path,
        "Host \"unterminated\nHost valid\nInclude ${UNSUPPORTED}/config\n",
    )
    .unwrap();
    let result = discover_with_home(&path, root.path());
    assert_eq!(result.aliases, ["valid"]);
    assert_eq!(result.warnings.len(), 2);
    fs::write(&path, "a".repeat(4 * 1024 * 1024 + 1)).unwrap();
    assert!(!discover_with_home(&path, root.path()).warnings.is_empty());
}

#[test]
fn include_depth_is_bounded() {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join(".ssh")).unwrap();
    for index in 0..25 {
        fs::write(
            root.path().join(format!(".ssh/{index}")),
            format!("Host host{index}\nInclude {}\n", index + 1),
        )
        .unwrap();
    }
    let result = discover_with_home(&root.path().join(".ssh/0"), root.path());
    assert_eq!(result.aliases.len(), 17);
    assert!(!result.warnings.is_empty());
}
