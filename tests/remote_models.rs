//! Run the real SSH helper in an isolated Python process against loopback HTTP.
//! No SSH hosts, real providers, personal configuration or real keys are used.
#![cfg(unix)]

use std::io::{Read, Write};
use std::net::TcpListener;
use std::process::{Command, Stdio};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::thread;
use std::time::Duration;

use codex_config::net::HttpOutcome;
use serde_json::{Value, json};

struct Server {
    url: String,
    requests: mpsc::Receiver<String>,
    stop: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}

impl Server {
    fn new(status: u16, body: &str, extra_headers: &str) -> Self {
        Self::raw(format!(
            "HTTP/1.1 {status} Fixture\r\nContent-Length: {}\r\nConnection: close\r\n{extra_headers}\r\n{body}",
            body.len()
        ))
    }

    fn raw(response: String) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let (sender, requests) = mpsc::channel();
        let thread = thread::spawn(move || {
            while !flag.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((mut socket, _)) => {
                        socket
                            .set_read_timeout(Some(Duration::from_secs(2)))
                            .unwrap();
                        socket
                            .set_write_timeout(Some(Duration::from_secs(2)))
                            .unwrap();
                        let mut request = Vec::new();
                        let mut buffer = [0; 2048];
                        while !request.windows(4).any(|bytes| bytes == b"\r\n\r\n")
                            && request.len() < 64 * 1024
                        {
                            match socket.read(&mut buffer) {
                                Ok(0) | Err(_) => break,
                                Ok(count) => request.extend_from_slice(&buffer[..count]),
                            }
                        }
                        if let Some(end) = request.windows(4).position(|bytes| bytes == b"\r\n\r\n")
                        {
                            let headers = String::from_utf8_lossy(&request[..end]).to_lowercase();
                            let length = headers
                                .lines()
                                .find_map(|line| {
                                    line.strip_prefix("content-length:")?
                                        .trim()
                                        .parse::<usize>()
                                        .ok()
                                })
                                .unwrap_or(0);
                            while request.len() < end + 4 + length && request.len() < 64 * 1024 {
                                match socket.read(&mut buffer) {
                                    Ok(0) | Err(_) => break,
                                    Ok(count) => request.extend_from_slice(&buffer[..count]),
                                }
                            }
                        }
                        let _ = sender.send(String::from_utf8_lossy(&request).into_owned());
                        // A client may close early for redirects or oversized bodies.
                        let _ = socket.write_all(response.as_bytes());
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(5));
                    }
                    Err(error) => panic!("fixture server: {error}"),
                }
            }
        });
        Self {
            url,
            requests,
            stop,
            thread: Some(thread),
        }
    }

    fn request(&self) -> String {
        self.requests.recv_timeout(Duration::from_secs(2)).unwrap()
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.thread.take().unwrap().join().unwrap();
    }
}

fn provider(base: &str) -> Value {
    json!({
        "base_url": base, "wire_api": "responses",
        "env_key": "", "bearer_token": "", "headers": [],
        "env_http_headers": [], "query_params": [],
        "requires_openai_auth": false, "command_auth": false, "aws_auth": false
    })
}

fn invoke(provider: Value, environment: &[(&str, &str)]) -> Value {
    invoke_with_prefix(provider, environment, "")
}

fn invoke_with_prefix(provider: Value, environment: &[(&str, &str)], prefix: &str) -> Value {
    invoke_request(
        json!({"operation":"list_models", "provider":provider}),
        environment,
        prefix,
    )
}

fn invoke_request(request: Value, environment: &[(&str, &str)], prefix: &str) -> Value {
    let root = tempfile::tempdir().unwrap();
    let mut child = Command::new("python3")
        .args(["-c", &format!("{prefix}\n{}", codex_config::remote::HELPER)])
        .current_dir(root.path())
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .envs(environment.iter().copied())
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
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let envelope: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(output.status.success(), envelope.get("error").is_none());
    envelope
}

#[test]
fn remote_chat_uses_protocol_payloads_and_never_returns_generated_content() {
    for (wire, path, body) in [
        (
            "chat",
            "/v1/chat/completions",
            r#"{"choices":[{"message":{"content":"private-response"}}]}"#,
        ),
        (
            "responses",
            "/v1/responses",
            r#"{"output":[{"text":"private-response"}]}"#,
        ),
        (
            "anthropic",
            "/v1/messages",
            r#"{"type":"message","content":[{"text":"private-response"}]}"#,
        ),
    ] {
        let server = Server::new(200, body, "");
        let mut settings = provider(&format!("{}/v1", server.url));
        settings["wire_api"] = json!(wire);
        settings["env_key"] = json!("CHAT_KEY");
        let result = invoke_request(
            json!({"operation":"probe_chat","provider":settings,"model":"fixture-model"}),
            &[("CHAT_KEY", "chat-only-secret")],
            "",
        );
        let returned = outcome(&result);
        assert!(returned.ok, "{result}");
        assert!(returned.remote_models.is_empty());
        assert!(!result.to_string().contains("private-response"));
        assert!(!result.to_string().contains("chat-only-secret"));
        let request = server.request();
        assert!(request.starts_with(&format!("POST {path} ")));
        let payload: Value =
            serde_json::from_str(request.split("\r\n\r\n").nth(1).unwrap()).unwrap();
        assert_eq!(payload["model"], "fixture-model");
        if wire == "responses" {
            assert_eq!(payload["input"], "ping");
            assert_eq!(payload["max_output_tokens"], 16);
        } else {
            assert_eq!(payload["max_tokens"], 1);
            assert_eq!(payload["messages"][0]["content"], "ping");
        }
    }
}

#[test]
fn remote_chat_rejects_redirects_and_successful_wrong_protocol_responses() {
    for (status, body) in [
        (200, r#"{"data":[]}"#),
        (200, "<html>login</html>"),
        (200, r#"{"choices":[],"error":"hidden"}"#),
        (302, "hidden"),
        (401, "hidden"),
    ] {
        let server = Server::new(status, body, "");
        let mut settings = provider(&server.url);
        settings["wire_api"] = json!("chat");
        let result = invoke_request(
            json!({"operation":"probe_chat","provider":settings,"model":"fixture"}),
            &[],
            "",
        );
        assert!(!outcome(&result).ok);
        assert!(!result.to_string().contains("hidden"));
    }
}

fn outcome(envelope: &Value) -> HttpOutcome {
    serde_json::from_value(envelope["result"].clone()).unwrap_or_else(|_| panic!("{envelope}"))
}

#[test]
fn remote_environment_auth_query_and_models_are_returned_without_secrets() {
    let server = Server::new(
        200,
        r#"{"data":[{"id":"model-b"},{"id":"model-a"},{"id":"model-b"},{"id":""},{"id":"remote-secret"}],"debug":"remote-header-secret"}"#,
        "",
    );
    let mut settings = provider(&format!("{}/v1", server.url));
    settings["env_key"] = json!("REMOTE_TEST_KEY");
    settings["bearer_token"] = json!("ignored-bearer");
    settings["headers"] = json!([["X-Tenant", "fixture-tenant"]]);
    settings["env_http_headers"] = json!([["X-Remote", "REMOTE_TEST_HEADER"]]);
    settings["query_params"] = json!([["api-version", "a b&中文"]]);
    let result = invoke(
        settings,
        &[
            ("REMOTE_TEST_KEY", "remote-secret"),
            ("REMOTE_TEST_HEADER", "remote-header-secret"),
        ],
    );
    let returned = outcome(&result);
    assert!(returned.ok, "{result}");
    assert_eq!(returned.remote_models, ["model-b", "model-a"]);
    for secret in ["remote-secret", "remote-header-secret", "ignored-bearer"] {
        assert!(!result.to_string().contains(secret));
    }
    let request = server.request().to_lowercase();
    assert!(request.starts_with("get /v1/models?api-version=a+b%26%e4%b8%ad%e6%96%87 "));
    assert!(request.contains("authorization: bearer remote-secret\r\n"));
    assert!(request.contains("x-remote: remote-header-secret\r\n"));
    assert!(request.contains("x-tenant: fixture-tenant\r\n"));
    assert!(!request.contains("ignored-bearer"));
}

#[test]
fn explicit_and_environment_headers_override_auth_case_insensitively() {
    for env_header in [false, true] {
        let server = Server::new(200, r#"{"data":[]}"#, "");
        let mut settings = provider(&server.url);
        settings["bearer_token"] = json!("initial-key");
        settings["headers"] = json!([["AUTHORIZATION", "Bearer explicit-key"]]);
        if env_header {
            settings["env_http_headers"] = json!([["Authorization", "REMOTE_AUTH_HEADER"]]);
        }
        assert!(
            outcome(&invoke(
                settings,
                &[("REMOTE_AUTH_HEADER", "Bearer header-env-key")]
            ))
            .ok
        );
        let request = server.request().to_lowercase();
        let expected = if env_header {
            "header-env-key"
        } else {
            "explicit-key"
        };
        assert!(request.contains(&format!("authorization: bearer {expected}\r\n")));
        assert_eq!(request.matches("\r\nauthorization:").count(), 1);
    }
}

#[test]
fn ordinary_header_and_query_values_do_not_filter_legitimate_model_ids() {
    let server = Server::new(200, r#"{"data":[{"id":"gpt-4.1"},{"id":"1"}]}"#, "");
    let mut settings = provider(&server.url);
    settings["headers"] = json!([["X-Tenant", "1"]]);
    settings["query_params"] = json!([["limit", "1"]]);
    let returned = outcome(&invoke(settings, &[]));
    assert!(returned.ok);
    assert_eq!(returned.remote_models, ["gpt-4.1", "1"]);
}

#[test]
fn anthropic_paths_and_version_headers_work_with_and_without_token() {
    for (suffix, token, custom_version) in [
        ("", "", false),
        ("/v1/", "anthropic-secret", false),
        ("", "anthropic-secret", true),
    ] {
        let server = Server::new(200, r#"{"data":[{"id":"claude-fixture"}]}"#, "");
        let mut settings = provider(&format!("{}{suffix}", server.url));
        settings["wire_api"] = json!("anthropic");
        settings["bearer_token"] = json!(token);
        if custom_version {
            settings["headers"] = json!([["Anthropic-Version", "2025-01-01"]]);
        }
        assert!(outcome(&invoke(settings, &[])).ok);
        let request = server.request().to_lowercase();
        assert!(request.starts_with("get /v1/models "));
        assert!(request.contains(if custom_version {
            "anthropic-version: 2025-01-01\r\n"
        } else {
            "anthropic-version: 2023-06-01\r\n"
        }));
        assert_eq!(
            request.contains("x-api-key: anthropic-secret\r\n"),
            !token.is_empty()
        );
    }
}

#[test]
fn alternate_empty_and_invalid_model_responses_are_distinguished() {
    for (body, expected) in [
        (
            r#"{"models":["a",{"id":"b"},{"slug":"c"},"a","",42]}"#,
            Some(vec!["a", "b", "c"]),
        ),
        (r#"{"data":[]}"#, Some(vec![])),
        (r#"{"models":[]}"#, Some(vec![])),
        (r#"<html>not-a-model-list</html>"#, None),
        (r#"{"data":[],"error":"hidden-error-body"}"#, None),
        (r#"{"data":"not-an-array"}"#, None),
        (r#"[]"#, None),
        (r#"null"#, None),
    ] {
        let server = Server::new(200, body, "");
        let mut settings = provider(&server.url);
        settings["wire_api"] = json!("chat");
        let result = invoke(settings, &[]);
        let returned = outcome(&result);
        assert_eq!(returned.ok, expected.is_some(), "{result}");
        assert_eq!(returned.remote_models, expected.unwrap_or_default());
        assert!(!result.to_string().contains("hidden-error-body"));
        assert!(server.request().starts_with("GET /models "));
    }
}

#[test]
fn missing_remote_variables_and_unsupported_auth_fail_before_network_io() {
    let server = Server::new(200, r#"{"data":[]}"#, "");
    for field in ["env_key", "env_http_headers"] {
        let mut settings = provider(&server.url);
        settings["bearer_token"] = json!("must-not-fall-back");
        settings[field] = if field == "env_key" {
            json!("MISSING_REMOTE_KEY")
        } else {
            json!([["X-Api-Key", "MISSING_REMOTE_KEY"]])
        };
        for environment in [vec![], vec![("MISSING_REMOTE_KEY", "  ")]] {
            let result = invoke(settings.clone(), &environment);
            let error = result["error"].as_str().unwrap();
            assert!(error.contains("MISSING_REMOTE_KEY"));
            assert!(error.contains("非交互"));
            assert!(!error.contains("must-not-fall-back"));
        }
    }
    for auth in ["requires_openai_auth", "command_auth", "aws_auth"] {
        let mut settings = provider(&server.url);
        settings[auth] = json!(true);
        assert!(
            invoke(settings, &[])["error"]
                .as_str()
                .unwrap()
                .contains("认证")
        );
    }
    assert!(server.requests.try_recv().is_err());
}

#[test]
fn invalid_urls_protocols_and_header_injection_never_send_requests() {
    let server = Server::new(200, r#"{"data":[]}"#, "");
    for base in [
        "file:///etc/passwd".to_string(),
        server.url.replace("://", "://user:secret@"),
        format!("{}?secret=value", server.url),
        format!("{}#fragment", server.url),
        format!("{}\r\nX-Injected: secret", server.url),
    ] {
        assert!(invoke(provider(&base), &[]).get("error").is_some());
    }
    let mut settings = provider(&server.url);
    settings["wire_api"] = json!("unknown-protocol");
    assert!(invoke(settings, &[]).get("error").is_some());
    let mut settings = provider(&server.url);
    settings["env_http_headers"] = json!([["X-Test", "BAD_HEADER"]]);
    let result = invoke(
        settings,
        &[("BAD_HEADER", "secret-value\r\nInjected: true")],
    );
    assert!(result.get("error").is_some());
    assert!(!result.to_string().contains("secret-value"));
    assert!(server.requests.try_recv().is_err());
}

#[test]
fn redirects_are_not_followed_and_error_bodies_do_not_cross_ssh() {
    let destination = Server::new(200, r#"{"data":[{"id":"must-not-follow"}]}"#, "");
    for status in [301, 302, 303, 307, 308, 401, 403, 404, 429, 500] {
        let source = Server::new(
            status,
            "echoed-fixture-key",
            &format!("Location: {}/models\r\n", destination.url),
        );
        let mut settings = provider(&source.url);
        settings["bearer_token"] = json!("echoed-fixture-key");
        let result = invoke(settings, &[]);
        let returned = outcome(&result);
        assert!(!returned.ok, "{result}");
        assert_eq!(returned.status, status);
        assert!(returned.remote_models.is_empty());
        assert!(!result.to_string().contains("echoed-fixture-key"));
        source.request();
    }
    assert!(destination.requests.try_recv().is_err());
}

#[test]
fn oversized_truncated_and_failed_reads_are_not_successful_lists() {
    let body = "x".repeat(2 * 1024 * 1024 + 1);
    let server = Server::new(200, &body, "");
    let result = invoke(provider(&server.url), &[]);
    assert!(!outcome(&result).ok);
    assert!(outcome(&result).summary.contains("2 MiB"));

    let server = Server::raw(format!(
        "HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n{body}"
    ));
    let result = invoke(provider(&server.url), &[]);
    assert!(!outcome(&result).ok);
    assert!(outcome(&result).summary.contains("2 MiB"));

    let server = Server::raw(
        "HTTP/1.1 200 OK\r\nContent-Length: 100\r\nConnection: close\r\n\r\n{\"data\":[]}".into(),
    );
    let result = invoke(provider(&server.url), &[]);
    assert!(!outcome(&result).ok);
    assert!(outcome(&result).summary.contains("不完整"));
}

#[test]
fn timeout_and_tls_settings_are_bounded_and_errors_are_sanitized() {
    let prefix = r#"
import ssl, urllib.request
def check_open(self, request, timeout):
    handler = next(h for h in self.handlers if isinstance(h, urllib.request.HTTPSHandler))
    if timeout != 20 or handler._context.verify_mode != ssl.CERT_REQUIRED or not handler._context.check_hostname:
        raise SystemExit("HTTP timeout or TLS verification regressed")
    raise TimeoutError("secret-from-library-exception")
urllib.request.OpenerDirector.open = check_open
"#;
    let result = invoke_with_prefix(provider("https://fixture.invalid"), &[], prefix);
    assert!(!outcome(&result).ok);
    assert!(outcome(&result).summary.contains("超时"));
    assert!(!result.to_string().contains("secret-from-library-exception"));
}
