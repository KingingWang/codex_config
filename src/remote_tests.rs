use super::*;

#[test]
fn model_list_payload_carries_names_and_explicit_settings_without_local_resolution() {
    let provider = ProviderView {
        id: "relay".into(),
        base_url: "https://fixture.invalid/v1".into(),
        wire_api: "responses".into(),
        env_key: "PATH".into(),
        bearer_token: "fixture-explicit-token".into(),
        headers: vec![("X-Tenant".into(), "fixture".into())],
        env_http_headers: vec![("X-Remote".into(), "PATH".into())],
        query_params: vec![("api-version".into(), "2026-01".into())],
        command_auth: true,
        ..ProviderView::default()
    };
    let payload = model_list_request(&provider);
    assert_eq!(payload["operation"], "list_models");
    assert_eq!(payload["provider"]["env_key"], "PATH");
    assert_eq!(payload["provider"]["env_http_headers"][0][1], "PATH");
    assert_eq!(
        payload["provider"]["bearer_token"],
        "fixture-explicit-token"
    );
    assert_eq!(payload["provider"]["command_auth"], true);
    assert_eq!(payload["provider"]["headers"][0][1], "fixture");
    assert_eq!(payload["provider"]["query_params"][0][1], "2026-01");
    assert!(payload["provider"].get("id").is_none());
}

#[test]
fn ssh_arguments_preserve_host_key_verification_and_send_no_user_data_to_shell() {
    let target = SshTarget {
        alias: "fixture".into(),
        config_file: PathBuf::from("/config with spaces"),
        home: "$(touch must-not-run)".into(),
    };
    let command = ssh_command(&target).unwrap();
    let args: Vec<_> = command
        .get_args()
        .map(|s| s.to_string_lossy().into_owned())
        .collect();
    for required in [
        "BatchMode=yes",
        "StrictHostKeyChecking=yes",
        "ForwardAgent=no",
        "PermitLocalCommand=no",
    ] {
        assert!(args.contains(&required.to_owned()));
    }
    assert!(!args.last().unwrap().contains(&target.home));
    assert_eq!(args[args.len() - 2], "fixture");
    assert!(args.contains(&"/config with spaces".into()));
}

#[cfg(unix)]
#[test]
fn transport_drains_large_pipes_and_handles_timeout_cancellation_and_failure() {
    let cancel = AtomicBool::new(false);
    let mut success = Command::new("python3");
    success.args(["-c", "import sys,json; data=json.load(sys.stdin); sys.stderr.write('x'*100000); print(json.dumps({'result':data}))"]);
    let result = request_command(
        success,
        json!({"test":"中文 and spaces"}),
        &cancel,
        Duration::from_secs(5),
    )
    .unwrap();
    assert_eq!(result["test"], "中文 and spaces");

    let mut slow = Command::new("python3");
    slow.args(["-c", "import time; time.sleep(30)"]);
    let started = Instant::now();
    assert!(
        request_command(slow, json!({}), &cancel, Duration::from_millis(100))
            .unwrap_err()
            .to_string()
            .contains("未知")
    );
    assert!(started.elapsed() < Duration::from_secs(3));

    let mut failure = Command::new("python3");
    failure.args([
        "-c",
        "import sys; sys.stderr.write('fixture failure'); sys.exit(255)",
    ]);
    assert!(
        request_command(failure, json!({}), &cancel, Duration::from_secs(5))
            .unwrap_err()
            .to_string()
            .contains("fixture failure")
    );

    cancel.store(true, Ordering::Relaxed);
    assert!(
        request_command(
            Command::new("must-not-be-launched"),
            json!({}),
            &cancel,
            Duration::from_secs(1)
        )
        .unwrap_err()
        .to_string()
        .contains("取消")
    );
}
