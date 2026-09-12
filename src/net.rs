//! Background HTTP helpers: "测试连接" and "拉取模型列表" for a provider.
//!
//! Network calls run on a worker thread; the UI only reads a shared slot.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::doc::providers::ProviderView;
use crate::doc::schema::WireApi;

#[derive(Debug, Clone, serde::Deserialize)]
pub struct HttpOutcome {
    pub ok: bool,
    pub status: u16,
    pub summary: String,
    pub detail: String,
    pub elapsed_ms: u128,
    /// Model ids returned by `GET /models`, when the probe was a listing.
    pub remote_models: Vec<String>,
}

pub type OutcomeSlot = Arc<Mutex<Option<Result<HttpOutcome, String>>>>;

pub fn new_slot() -> OutcomeSlot {
    Arc::new(Mutex::new(None))
}

pub fn take(slot: &OutcomeSlot) -> Option<Result<HttpOutcome, String>> {
    slot.lock().ok().and_then(|mut guard| guard.take())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Probe {
    /// Send a 1-token completion.
    Chat,
    /// `GET /models` and list what the server offers.
    ListModels,
}

pub fn spawn(
    ctx: &egui::Context,
    provider: ProviderView,
    model: Option<String>,
    probe: Probe,
) -> OutcomeSlot {
    let slot = new_slot();
    let writer = slot.clone();
    let ctx = ctx.clone();
    std::thread::spawn(move || {
        let result = run(&provider, model, probe);
        if let Ok(mut guard) = writer.lock() {
            *guard = Some(result);
        }
        ctx.request_repaint();
    });
    slot
}

fn run(
    provider: &ProviderView,
    model: Option<String>,
    probe: Probe,
) -> Result<HttpOutcome, String> {
    if provider.base_url.trim().is_empty() {
        return Err("还没有填写 base_url，无法测试。".into());
    }
    if !provider.has_known_wire_api() {
        return Err(format!(
            "无法测试未知协议 {}；配置已保留，请检查 wire_api。",
            provider.wire_api
        ));
    }
    if provider.requires_openai_auth || provider.command_auth || provider.aws_auth {
        return Err("此测试暂不支持 Codex 登录态、命令或 AWS 认证；不会读取登录凭据或执行认证命令，请使用 Codex 验证。".into());
    }
    let base = provider.base_url.trim().trim_end_matches('/');
    let uri: ureq::http::Uri = base
        .parse()
        .map_err(|_| "base_url 不是合法的 HTTP 地址。".to_string())?;
    if !matches!(uri.scheme_str(), Some("http" | "https")) || uri.host().is_none() {
        return Err("base_url 必须包含 http:// 或 https:// 以及主机名。".into());
    }
    if uri.query().is_some() || base.contains('#') {
        return Err("base_url 请只填写接口地址；查询参数请放在 query_params 中。".into());
    }
    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(20)))
        .http_status_as_error(false)
        // API credentials (including custom headers) must not follow redirects.
        .max_redirects(0)
        .build()
        .new_agent();

    let wire = provider.wire();
    let started = Instant::now();

    match probe {
        Probe::ListModels => {
            let url = if wire == WireApi::Anthropic && !base.ends_with("/v1") {
                format!("{base}/v1/models")
            } else {
                format!("{base}/models")
            };
            let mut req = agent.get(&url);
            req = apply_headers(req, provider, wire)?;
            let response = req.call().map_err(transport_err)?;
            let status = response.status().as_u16();
            let body = read_body(response)?;
            let elapsed = started.elapsed().as_millis();
            let remote = extract_model_ids(&body);
            if (200..300).contains(&status) && is_model_list(&body) {
                Ok(HttpOutcome {
                    ok: true,
                    status,
                    summary: format!("连接成功，发现 {} 个模型", remote.len()),
                    detail: if remote.is_empty() {
                        body.chars().take(600).collect()
                    } else {
                        remote.join(", ")
                    },
                    elapsed_ms: elapsed,
                    remote_models: remote,
                })
            } else {
                Ok(HttpOutcome {
                    ok: false,
                    status,
                    summary: if (200..300).contains(&status) {
                        "HTTP 返回成功，但响应不是有效的模型列表 JSON".into()
                    } else {
                        status_summary(status)
                    },
                    detail: short_body(&body),
                    elapsed_ms: elapsed,
                    remote_models: Vec::new(),
                })
            }
        }
        Probe::Chat => {
            let model = model.unwrap_or_else(|| "test".to_string());
            let (url, payload) = match wire {
                WireApi::Chat => (
                    format!("{base}/chat/completions"),
                    json!({
                        "model": model,
                        "messages": [{"role": "user", "content": "ping"}],
                        "max_tokens": 1,
                        "stream": false
                    }),
                ),
                WireApi::Responses => (
                    format!("{base}/responses"),
                    json!({ "model": model, "input": "ping", "max_output_tokens": 16 }),
                ),
                WireApi::Anthropic => (
                    anthropic_url(base),
                    json!({
                        "model": model,
                        "max_tokens": 1,
                        "messages": [{"role": "user", "content": "ping"}]
                    }),
                ),
            };

            let mut req = agent.post(&url);
            req = apply_headers(req, provider, wire)?;
            let response = req.send_json(payload).map_err(transport_err)?;
            let status = response.status().as_u16();
            let body = read_body(response)?;
            let elapsed = started.elapsed().as_millis();
            let valid = (200..300).contains(&status) && is_completion(&body, wire);
            Ok(HttpOutcome {
                ok: valid,
                status,
                summary: if valid {
                    "连接成功，服务商正常返回了响应".to_string()
                } else if (200..300).contains(&status) {
                    "HTTP 返回成功，但响应与所选协议不匹配（可能是网页或错误 JSON）".into()
                } else {
                    status_summary(status)
                },
                detail: short_body(&body),
                elapsed_ms: elapsed,
                remote_models: Vec::new(),
            })
        }
    }
}

fn anthropic_url(base: &str) -> String {
    if base.ends_with("/v1") {
        format!("{base}/messages")
    } else {
        format!("{base}/v1/messages")
    }
}

fn apply_headers<B>(
    mut req: ureq::RequestBuilder<B>,
    provider: &ProviderView,
    wire: WireApi,
) -> Result<ureq::RequestBuilder<B>, String> {
    let token = resolve_token(provider)?;
    if let Some(token) = token {
        if wire == WireApi::Anthropic {
            req = req.header("x-api-key", &token);
        } else {
            req = req.header("authorization", &format!("Bearer {token}"));
        }
    }
    for (key, value) in &provider.headers {
        req = req.header(key, value);
    }
    for (key, variable) in &provider.env_http_headers {
        if let Ok(value) = std::env::var(variable) {
            req = req.header(key, value);
        }
    }
    if wire == WireApi::Anthropic
        && !provider
            .headers
            .iter()
            .chain(&provider.env_http_headers)
            .any(|(key, _)| key.eq_ignore_ascii_case("anthropic-version"))
    {
        req = req.header("anthropic-version", "2023-06-01");
    }
    for (key, value) in &provider.query_params {
        req = req.query(key, value);
    }
    Ok(req.header("content-type", "application/json"))
}

fn resolve_token(provider: &ProviderView) -> Result<Option<String>, String> {
    if !provider.env_key.is_empty() {
        return std::env::var(&provider.env_key)
            .ok()
            .filter(|v| !v.trim().is_empty())
            .map(Some)
            .ok_or_else(|| {
                format!(
                    "环境变量 {} 未设置或为空，未发送测试请求。",
                    provider.env_key
                )
            });
    }
    if !provider.bearer_token.is_empty() {
        return Ok(Some(provider.bearer_token.clone()));
    }
    Ok(None)
}

fn transport_err(err: ureq::Error) -> String {
    match &err {
        ureq::Error::Io(_) | ureq::Error::Timeout(_) => {
            format!("网络请求失败：{err}\n\n常见原因：地址写错、服务没启动、需要代理、证书问题。")
        }
        other => format!("网络请求失败：{other}"),
    }
}

fn read_body(response: ureq::http::Response<ureq::Body>) -> Result<String, String> {
    let mut body = response.into_body();
    body.with_config()
        .limit(2 * 1024 * 1024)
        .read_to_string()
        .map_err(|err| format!("读取响应失败（内容可能不完整或超过 2 MiB）：{err}"))
}

fn is_model_list(body: &str) -> bool {
    serde_json::from_str::<Value>(body).is_ok_and(|value| {
        value.get("error").is_none()
            && ["data", "models"]
                .iter()
                .any(|key| value.get(key).is_some_and(Value::is_array))
    })
}

fn is_completion(body: &str, wire: WireApi) -> bool {
    serde_json::from_str::<Value>(body).is_ok_and(|value| {
        value.get("error").is_none()
            && match wire {
                WireApi::Chat => value.get("choices").is_some_and(Value::is_array),
                WireApi::Responses => value.get("output").is_some_and(Value::is_array),
                WireApi::Anthropic => {
                    value.get("type").and_then(Value::as_str) == Some("message")
                        && value.get("content").is_some_and(Value::is_array)
                }
            }
    })
}
fn short_body(body: &str) -> String {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return "(服务端没有返回内容)".to_string();
    }
    // Try to pretty print JSON so the user can read the error.
    if let Ok(value) = serde_json::from_str::<Value>(trimmed)
        && let Ok(pretty) = serde_json::to_string_pretty(&value)
    {
        return pretty.chars().take(1200).collect();
    }
    trimmed.chars().take(1200).collect()
}

fn status_summary(status: u16) -> String {
    match status {
        300..=399 => format!("HTTP {status} 重定向：为保护密钥未自动跟随，请检查 base_url"),
        400 => "HTTP 400 请求参数有误（可能是模型名不对）".to_string(),
        401 => "HTTP 401 未授权：API Key 缺失或写错了".to_string(),
        403 => "HTTP 403 没有权限：Key 有效但不能访问这个资源".to_string(),
        404 => "HTTP 404 地址不存在：base_url 或协议选错了".to_string(),
        405 => "HTTP 405 方法不允许：协议可能选错了，试试另一个".to_string(),
        429 => "HTTP 429 请求过于频繁或额度用尽".to_string(),
        s if s >= 500 => format!("HTTP {s} 服务端错误，稍后再试").to_string(),
        s => format!("HTTP {s} 请求被拒绝").to_string(),
    }
}

fn extract_model_ids(body: &str) -> Vec<String> {
    let Ok(value) = serde_json::from_str::<Value>(body) else {
        return Vec::new();
    };
    let mut ids = Vec::new();
    if let Some(list) = value.get("data").and_then(Value::as_array) {
        for entry in list {
            if let Some(id) = entry.get("id").and_then(Value::as_str) {
                ids.push(id.to_string());
            }
        }
    }
    if ids.is_empty()
        && let Some(list) = value.get("models").and_then(Value::as_array)
    {
        for entry in list {
            let id = entry
                .as_str()
                .map(str::to_string)
                .or_else(|| entry.get("id").and_then(Value::as_str).map(str::to_string))
                .or_else(|| {
                    entry
                        .get("slug")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                });
            if let Some(id) = id {
                ids.push(id);
            }
        }
    }
    let mut seen = std::collections::HashSet::new();
    ids.retain(|id| !id.trim().is_empty() && seen.insert(id.clone()));
    ids
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::mpsc;

    /// One disposable loopback request; no credentials or external network.
    fn server(status: u16, body: &str, headers: &str) -> (String, mpsc::Receiver<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = listener.local_addr().unwrap();
        let response = format!(
            "HTTP/1.1 {status} Test\r\nContent-Length: {}\r\nConnection: close\r\n{headers}\r\n{body}",
            body.len()
        );
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            let started = Instant::now();
            while started.elapsed() < Duration::from_secs(5) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        // macOS can inherit the listener's nonblocking mode.
                        stream.set_nonblocking(false).unwrap();
                        stream
                            .set_read_timeout(Some(Duration::from_secs(2)))
                            .unwrap();
                        stream
                            .set_write_timeout(Some(Duration::from_secs(2)))
                            .unwrap();
                        let mut request = Vec::new();
                        let mut buffer = [0; 2048];
                        while let Ok(count) = stream.read(&mut buffer) {
                            if count == 0 {
                                break;
                            }
                            request.extend_from_slice(&buffer[..count]);
                            if request.windows(4).any(|w| w == b"\r\n\r\n") {
                                break;
                            }
                        }
                        let _ = stream.write_all(response.as_bytes());
                        let _ = sender.send(String::from_utf8_lossy(&request).into_owned());
                        break;
                    }
                    Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    Err(_) => break,
                }
            }
        });
        (format!("http://{address}"), receiver)
    }

    #[test]
    fn anthropic_listing_uses_versioned_url_headers_and_encoded_query() {
        let (base_url, request) = server(200, r#"{"data":[{"id":"demo"},{"id":"demo"}]}"#, "");
        let provider = ProviderView {
            base_url,
            wire_api: "anthropic".into(),
            bearer_token: "fake-token".into(),
            query_params: vec![("api-version".into(), "a b".into())],
            env_http_headers: vec![("x-test-path".into(), "PATH".into())],
            ..Default::default()
        };
        let outcome = run(&provider, None, Probe::ListModels).unwrap();
        assert!(outcome.ok);
        assert_eq!(outcome.remote_models, ["demo"]);
        let request = request
            .recv_timeout(Duration::from_secs(3))
            .unwrap()
            .to_lowercase();
        assert!(request.starts_with("get /v1/models?api-version=a%20b "));
        assert!(request.contains("anthropic-version: 2023-06-01"));
        assert!(request.contains("x-api-key: fake-token"));
        assert!(request.contains("x-test-path: "));
    }

    #[test]
    fn html_and_error_json_are_not_reported_as_success() {
        for body in [
            "<html>login</html>",
            r#"{"error":"not authorized"}"#,
            r#"{"other":[]}"#,
        ] {
            let (base_url, _) = server(200, body, "");
            let outcome = run(
                &ProviderView {
                    base_url,
                    wire_api: "responses".into(),
                    ..Default::default()
                },
                None,
                Probe::ListModels,
            )
            .unwrap();
            assert!(!outcome.ok);
        }
        assert!(!is_completion("<html>login</html>", WireApi::Responses));
        assert!(is_completion(r#"{"output":[]}"#, WireApi::Responses));
        assert!(is_completion(r#"{"choices":[]}"#, WireApi::Chat));
        assert!(is_completion(
            r#"{"type":"message","content":[]}"#,
            WireApi::Anthropic
        ));
    }

    #[test]
    fn redirects_are_not_followed_or_reported_as_success() {
        let (target, requests) = server(200, r#"{"data":[]}"#, "");
        let (base_url, _) = server(302, "", &format!("Location: {target}/models\r\n"));
        let outcome = run(
            &ProviderView {
                base_url,
                wire_api: "anthropic".into(),
                bearer_token: "fake-token".into(),
                ..Default::default()
            },
            None,
            Probe::ListModels,
        )
        .unwrap();
        assert!(!outcome.ok);
        assert_eq!(outcome.status, 302);
        assert!(requests.recv_timeout(Duration::from_millis(100)).is_err());
    }

    #[test]
    fn unsupported_auth_and_missing_environment_keys_fail_before_network_io() {
        let mut provider = ProviderView {
            base_url: "http://127.0.0.1:1".into(),
            wire_api: "responses".into(),
            requires_openai_auth: true,
            ..Default::default()
        };
        assert!(
            run(&provider, None, Probe::ListModels)
                .unwrap_err()
                .contains("暂不支持")
        );
        provider.requires_openai_auth = false;
        provider.env_key = "CODEX_CONFIG_TEST_ABSENT_KEY_802239".into();
        assert!(std::env::var_os(&provider.env_key).is_none());
        assert!(
            run(&provider, None, Probe::ListModels)
                .unwrap_err()
                .contains("未设置")
        );
    }

    #[test]
    fn incomplete_body_is_an_error_not_an_empty_success() {
        let response = ureq::http::Response::builder()
            .body(ureq::Body::builder().data(vec![b'x'; 2 * 1024 * 1024 + 1]))
            .unwrap();
        assert!(read_body(response).is_err());
    }
}
