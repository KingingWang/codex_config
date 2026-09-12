//! Background HTTP helpers: "测试连接" and "拉取模型列表" for a provider.
//!
//! Network calls run on a worker thread; the UI only reads a shared slot.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::doc::providers::ProviderView;
use crate::doc::schema::WireApi;

#[derive(Debug, Clone)]
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

pub fn spawn(ctx: &egui::Context, provider: ProviderView, model: Option<String>, probe: Probe) -> OutcomeSlot {
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

fn run(provider: &ProviderView, model: Option<String>, probe: Probe) -> Result<HttpOutcome, String> {
    if provider.base_url.trim().is_empty() {
        return Err("还没有填写 base_url，无法测试。".into());
    }
    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(20)))
        .http_status_as_error(false)
        .build()
        .new_agent();

    let base = provider.base_url.trim().trim_end_matches('/');
    let wire = provider.wire();
    let started = Instant::now();

    match probe {
        Probe::ListModels => {
            let url = format!("{base}/models");
            let mut req = agent.get(&url);
            req = apply_headers(req, provider, wire);
            let response = req.call().map_err(transport_err)?;
            let status = response.status().as_u16();
            let body = read_body(response);
            let elapsed = started.elapsed().as_millis();
            let remote = extract_model_ids(&body);
            if status < 400 {
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
                    summary: status_summary(status),
                    detail: short_body(&body),
                    elapsed_ms: elapsed,
                    remote_models: remote,
                })
            }
        }
        Probe::Chat => {
            let model = model.unwrap_or_else(|| "test".to_string());
            let (url, payload, extra_headers) = match wire {
                WireApi::Chat => (
                    format!("{base}/chat/completions"),
                    json!({
                        "model": model,
                        "messages": [{"role": "user", "content": "ping"}],
                        "max_tokens": 1,
                        "stream": false
                    }),
                    Vec::new(),
                ),
                WireApi::Responses => (
                    format!("{base}/responses"),
                    json!({ "model": model, "input": "ping", "max_output_tokens": 16 }),
                    Vec::new(),
                ),
                WireApi::Anthropic => (
                    anthropic_url(base),
                    json!({
                        "model": model,
                        "max_tokens": 1,
                        "messages": [{"role": "user", "content": "ping"}]
                    }),
                    vec![("anthropic-version", "2023-06-01")],
                ),
            };

            let mut req = agent.post(&url);
            for (key, value) in extra_headers {
                req = req.header(key, value);
            }
            req = apply_headers(req, provider, wire);
            let response = req.send_json(payload).map_err(transport_err)?;
            let status = response.status().as_u16();
            let body = read_body(response);
            let elapsed = started.elapsed().as_millis();
            Ok(HttpOutcome {
                ok: status < 400,
                status,
                summary: if status < 400 {
                    "连接成功，服务商正常返回了响应".to_string()
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
) -> ureq::RequestBuilder<B> {
    let token = resolve_token(provider);
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
    req.header("content-type", "application/json")
}

fn resolve_token(provider: &ProviderView) -> Option<String> {
    if !provider.env_key.is_empty()
        && let Ok(value) = std::env::var(&provider.env_key)
        && !value.is_empty()
    {
        return Some(value);
    }
    if !provider.bearer_token.is_empty() {
        return Some(provider.bearer_token.clone());
    }
    None
}

fn transport_err(err: ureq::Error) -> String {
    match &err {
        ureq::Error::Io(_) | ureq::Error::Timeout(_) => format!(
            "网络请求失败：{err}\n\n常见原因：地址写错、服务没启动、需要代理、证书问题。"
        ),
        other => format!("网络请求失败：{other}"),
    }
}

fn read_body(response: ureq::http::Response<ureq::Body>) -> String {
    let mut body = response.into_body();
    body.read_to_string().unwrap_or_default()
}

fn short_body(body: &str) -> String {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return "(服务端没有返回内容)".to_string();
    }
    // Try to pretty print JSON so the user can read the error.
    if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
        if let Ok(pretty) = serde_json::to_string_pretty(&value) {
            return pretty.chars().take(1200).collect();
        }
    }
    trimmed.chars().take(1200).collect()
}

fn status_summary(status: u16) -> String {
    match status {
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
                .or_else(|| entry.get("slug").and_then(Value::as_str).map(str::to_string));
            if let Some(id) = id {
                ids.push(id);
            }
        }
    }
    ids
}
