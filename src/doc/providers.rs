//! Read/write helpers for `[model_providers.<id>]`.
//!
//! Only the keys the user actually touches are written back, so any field this
//! GUI does not know about survives a round trip.

use toml_edit::{DocumentMut, InlineTable, Item, Table, Value};

use crate::doc::schema::WireApi;
use crate::doc::toml_ext::{read_string_map, TomlPathExt};

/// A snapshot of one provider, used for rendering list rows and validation.
#[derive(Debug, Clone, Default)]
pub struct ProviderView {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub wire_api: String,
    pub env_key: String,
    pub env_key_instructions: String,
    pub bearer_token: String,
    pub chat_stream: bool,
    pub requires_openai_auth: bool,
    pub supports_websockets: bool,
    pub request_max_retries: Option<i64>,
    pub stream_max_retries: Option<i64>,
    pub stream_idle_timeout_ms: Option<i64>,
    pub headers: Vec<(String, String)>,
    pub query_params: Vec<(String, String)>,
    pub env_http_headers: Vec<(String, String)>,
    /// Keys present in the file that this GUI does not edit.
    pub extra_keys: Vec<String>,
}

const KNOWN_KEYS: &[&str] = &[
    "name",
    "base_url",
    "wire_api",
    "env_key",
    "env_key_instructions",
    "experimental_bearer_token",
    "chat_stream",
    "requires_openai_auth",
    "supports_websockets",
    "supports_standalone_web_search",
    "request_max_retries",
    "stream_max_retries",
    "stream_idle_timeout_ms",
    "websocket_connect_timeout_ms",
    "http_headers",
    "query_params",
    "env_http_headers",
    "auth",
    "aws",
];

impl ProviderView {
    pub fn wire(&self) -> WireApi {
        WireApi::parse(&self.wire_api).unwrap_or(WireApi::Responses)
    }

    pub fn has_known_wire_api(&self) -> bool {
        WireApi::parse(&self.wire_api).is_some()
    }

    /// How this provider authenticates, in plain words.
    pub fn auth_summary(&self) -> String {
        if !self.env_key.is_empty() {
            format!("读取环境变量 {}", self.env_key)
        } else if !self.bearer_token.is_empty() {
            "配置文件里直接写了 Token".to_string()
        } else if self.requires_openai_auth {
            "使用 Codex 登录态（auth.json）".to_string()
        } else {
            "未设置密钥（本地服务通常不需要）".to_string()
        }
    }
}

pub fn ids(config: &DocumentMut) -> Vec<String> {
    config.keys_at(&["model_providers"])
}

pub fn view(config: &DocumentMut, id: &str) -> Option<ProviderView> {
    let path = ["model_providers", id];
    config.table_at(&path)?;
    let get_str = |key: &str| {
        config
            .str_at(&["model_providers", id, key])
            .unwrap_or_default()
    };
    let get_bool = |key: &str| {
        config
            .bool_at(&["model_providers", id, key])
            .unwrap_or(false)
    };
    let get_int = |key: &str| config.int_at(&["model_providers", id, key]);

    let mut extra_keys = Vec::new();
    if let Some(table) = config.table_at(&path) {
        for (key, _) in table.iter() {
            if !KNOWN_KEYS.contains(&key) {
                extra_keys.push(key.to_string());
            }
        }
    }

    Some(ProviderView {
        id: id.to_string(),
        name: get_str("name"),
        base_url: get_str("base_url"),
        wire_api: {
            let value = get_str("wire_api");
            if value.is_empty() {
                "responses".to_string()
            } else {
                value
            }
        },
        env_key: get_str("env_key"),
        env_key_instructions: get_str("env_key_instructions"),
        bearer_token: get_str("experimental_bearer_token"),
        chat_stream: get_bool("chat_stream"),
        requires_openai_auth: get_bool("requires_openai_auth"),
        supports_websockets: get_bool("supports_websockets"),
        request_max_retries: get_int("request_max_retries"),
        stream_max_retries: get_int("stream_max_retries"),
        stream_idle_timeout_ms: get_int("stream_idle_timeout_ms"),
        headers: read_string_map(config.item_at(&["model_providers", id, "http_headers"])),
        query_params: read_string_map(config.item_at(&["model_providers", id, "query_params"])),
        env_http_headers: read_string_map(config.item_at(&["model_providers", id, "env_http_headers"])),
        extra_keys,
    })
}

pub fn all(config: &DocumentMut) -> Vec<ProviderView> {
    ids(config)
        .iter()
        .filter_map(|id| view(config, id))
        .collect()
}

pub fn exists(config: &DocumentMut, id: &str) -> bool {
    config
        .table_at(&["model_providers", id])
        .is_some()
}

/// Create an empty `[model_providers.<id>]` table.
pub fn create(config: &mut DocumentMut, id: &str) {
    config.ensure_table_at(&["model_providers", id]);
}

pub fn remove(config: &mut DocumentMut, id: &str) {
    config.remove_at(&["model_providers", id]);
}

pub fn rename(config: &mut DocumentMut, old_id: &str, new_id: &str) -> bool {
    if old_id == new_id || exists(config, new_id) {
        return false;
    }
    let Some(table) = config
        .table_at(&["model_providers", old_id])
        .cloned()
    else {
        return false;
    };
    let table = {
        let mut t = Table::new();
        for (key, item) in table.iter() {
            t.insert(key, item.clone());
        }
        t
    };
    remove(config, old_id);
    config.set_item_at(&["model_providers", new_id], Item::Table(table));
    true
}

/// Set a string key, or delete it when the user cleared the field.
pub fn set_str(config: &mut DocumentMut, id: &str, key: &str, value: &str) {
    let path = ["model_providers", id, key];
    let trimmed = value.trim();
    if trimmed.is_empty() {
        config.remove_at(&path);
    } else {
        config.set_value_at(&path, Value::from(trimmed.to_string()));
    }
}

pub fn set_bool(config: &mut DocumentMut, id: &str, key: &str, value: bool) {
    let path = ["model_providers", id, key];
    if !value {
        config.remove_at(&path);
    } else {
        config.set_value_at(&path, Value::from(true));
    }
}

pub fn set_opt_int(config: &mut DocumentMut, id: &str, key: &str, value: Option<i64>) {
    let path = ["model_providers", id, key];
    match value {
        Some(v) => config.set_value_at(&path, Value::from(v)),
        None => {
            config.remove_at(&path);
        }
    }
}

pub fn set_map(config: &mut DocumentMut, id: &str, key: &str, entries: &[(String, String)]) {
    let path = ["model_providers", id, key];
    let cleaned: Vec<&(String, String)> = entries
        .iter()
        .filter(|(k, _)| !k.trim().is_empty())
        .collect();
    if cleaned.is_empty() {
        config.remove_at(&path);
        return;
    }
    let mut table = InlineTable::new();
    for (name, value) in cleaned {
        table.insert(name.trim(), Value::from(value.clone()).into());
    }
    config.set_value_at(&path, Value::InlineTable(table));
}

/// A free id like `openai-relay`, `my-relay-2`.
pub fn suggest_id(config: &DocumentMut, wanted: &str) -> String {
    let base = slugify(wanted);
    let base = if base.is_empty() { "provider".to_string() } else { base };
    if !exists(config, &base) {
        return base;
    }
    for n in 2..100 {
        let candidate = format!("{base}-{n}");
        if !exists(config, &candidate) {
            return candidate;
        }
    }
    base
}

pub fn slugify(raw: &str) -> String {
    raw.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else if c == '-' || c == '_' || c == '.' {
                c
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}
