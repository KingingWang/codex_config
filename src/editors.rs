//! Edit buffers for the three things users change most: a model, a provider and
//! a profile. Buffers are loaded from the document when something is selected and
//! written straight back on every keystroke, so the top bar "保存" is the only
//! action the user has to remember.

use serde_json::{Value, json};

use crate::doc::catalog;
use crate::doc::providers::{self, ProviderView};
use crate::doc::schema::WireApi;
use crate::doc::toml_ext::TomlPathExt;
use crate::doc::providers as providers_mod;
use toml_edit::{DocumentMut, Value as TomlValue};

fn num_str(value: Option<i64>) -> String {
    value.map(|v| v.to_string()).unwrap_or_default()
}

fn opt_str(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn set_or_remove_str(model: &mut Value, key: &str, value: &str) {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        catalog::remove(model, &[key]);
    } else {
        catalog::set(model, &[key], Value::String(trimmed.to_string()));
    }
}

fn set_or_remove_int(model: &mut Value, key: &str, buffer: &str) {
    match crate::ui::widgets::parse_opt_int(buffer) {
        Some(v) => catalog::set(model, &[key], json!(v)),
        None => {
            catalog::remove(model, &[key]);
        }
    }
}

// ---------------------------------------------------------------------------
// Model editor
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct ModelEditor {
    pub slug: String,
    pub display_name: String,
    pub description: String,
    pub provider: String,
    pub visibility: String,
    pub priority: String,
    pub shell_type: String,
    pub apply_patch_tool_type: String,
    pub web_search_tool_type: String,
    pub tool_mode: String,
    pub context_window: String,
    pub max_context_window: String,
    pub auto_compact_token_limit: String,
    pub truncation_mode: String,
    pub truncation_limit: String,
    pub input_modalities: Vec<String>,
    pub supported_reasoning_levels: Vec<String>,
    pub default_reasoning_level: String,
    pub default_reasoning_summary: String,
    pub support_verbosity: bool,
    pub default_verbosity: String,
    pub supports_parallel_tool_calls: bool,
    pub supports_image_detail_original: bool,
    pub use_responses_lite: bool,
    pub include_skills_usage_instructions: bool,
    pub prefer_websockets: bool,
    pub supported_in_api: bool,
    pub comp_hash: String,
    pub auto_review_model_override: String,
    pub nux_message: String,
    pub base_instructions: String,
    pub open_advanced: bool,
    pub open_json: bool,
}

impl ModelEditor {
    pub fn from_value(model: &Value) -> Self {
        let get = |key: &str| model.get(key).cloned().unwrap_or(Value::Null);
        Self {
            slug: opt_str(model, "slug"),
            display_name: opt_str(model, "display_name"),
            description: opt_str(model, "description"),
            provider: model
                .get("provider")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            visibility: if get("visibility").is_null() {
                "list".into()
            } else {
                opt_str(model, "visibility")
            },
            priority: model
                .get("priority")
                .and_then(Value::as_i64)
                .map(|v| v.to_string())
                .unwrap_or_default(),
            shell_type: if get("shell_type").is_null() {
                "shell_command".into()
            } else {
                opt_str(model, "shell_type")
            },
            apply_patch_tool_type: opt_str(model, "apply_patch_tool_type"),
            web_search_tool_type: if get("web_search_tool_type").is_null() {
                "text".into()
            } else {
                opt_str(model, "web_search_tool_type")
            },
            tool_mode: opt_str(model, "tool_mode"),
            context_window: num_str(catalog::i64_at(model, &["context_window"])),
            max_context_window: num_str(catalog::i64_at(model, &["max_context_window"])),
            auto_compact_token_limit: num_str(catalog::i64_at(model, &["auto_compact_token_limit"])),
            truncation_mode: catalog::str_at(model, &["truncation_policy", "mode"])
                .unwrap_or_else(|| "tokens".into()),
            truncation_limit: num_str(catalog::i64_at(model, &["truncation_policy", "limit"])),
            input_modalities: catalog::str_vec_at(model, &["input_modalities"]),
            supported_reasoning_levels: catalog::reasoning_levels(model),
            default_reasoning_level: opt_str(model, "default_reasoning_level"),
            default_reasoning_summary: if get("default_reasoning_summary").is_null() {
                "auto".into()
            } else {
                opt_str(model, "default_reasoning_summary")
            },
            support_verbosity: catalog::bool_at(model, &["support_verbosity"]).unwrap_or(false),
            default_verbosity: opt_str(model, "default_verbosity"),
            supports_parallel_tool_calls: catalog::bool_at(model, &["supports_parallel_tool_calls"])
                .unwrap_or(true),
            supports_image_detail_original: catalog::bool_at(model, &["supports_image_detail_original"])
                .unwrap_or(false),
            use_responses_lite: catalog::bool_at(model, &["use_responses_lite"]).unwrap_or(false),
            include_skills_usage_instructions: catalog::bool_at(model, &["include_skills_usage_instructions"])
                .unwrap_or(false),
            prefer_websockets: catalog::bool_at(model, &["prefer_websockets"]).unwrap_or(false),
            supported_in_api: catalog::bool_at(model, &["supported_in_api"]).unwrap_or(true),
            comp_hash: opt_str(model, "comp_hash"),
            auto_review_model_override: opt_str(model, "auto_review_model_override"),
            nux_message: catalog::str_at(model, &["availability_nux", "message"]).unwrap_or_default(),
            base_instructions: opt_str(model, "base_instructions"),
            open_advanced: false,
            open_json: false,
        }
    }

    /// Write the buffer back into a catalog entry, leaving unknown fields alone.
    pub fn write_to(&self, model: &mut Value) {
        set_or_remove_str(model, "slug", &self.slug);
        if self.display_name.trim().is_empty() {
            catalog::set(model, &["display_name"], Value::String(self.slug.trim().to_string()));
        } else {
            set_or_remove_str(model, "display_name", &self.display_name);
        }
        set_or_remove_str(model, "description", &self.description);
        set_or_remove_str(model, "provider", &self.provider);
        set_or_remove_str(model, "visibility", &self.visibility);
        set_or_remove_int(model, "priority", &self.priority);
        set_or_remove_str(model, "shell_type", &self.shell_type);
        set_or_remove_str(model, "apply_patch_tool_type", &self.apply_patch_tool_type);
        set_or_remove_str(model, "web_search_tool_type", &self.web_search_tool_type);
        set_or_remove_str(model, "tool_mode", &self.tool_mode);
        set_or_remove_int(model, "context_window", &self.context_window);
        set_or_remove_int(model, "max_context_window", &self.max_context_window);
        set_or_remove_int(model, "auto_compact_token_limit", &self.auto_compact_token_limit);

        let mode = if self.truncation_mode.is_empty() {
            "tokens"
        } else {
            self.truncation_mode.as_str()
        };
        catalog::set(model, &["truncation_policy", "mode"], Value::String(mode.to_string()));
        match crate::ui::widgets::parse_opt_int(&self.truncation_limit) {
            Some(limit) => catalog::set(model, &["truncation_policy", "limit"], json!(limit)),
            None => {
                catalog::remove(model, &["truncation_policy", "limit"]);
            }
        }

        catalog::set(
            model,
            &["input_modalities"],
            Value::Array(
                self.input_modalities
                    .iter()
                    .map(|m| Value::String(m.clone()))
                    .collect(),
            ),
        );
        catalog::set_reasoning_levels(model, &self.supported_reasoning_levels);
        set_or_remove_str(model, "default_reasoning_level", &self.default_reasoning_level);
        set_or_remove_str(model, "default_reasoning_summary", &self.default_reasoning_summary);
        catalog::set(model, &["support_verbosity"], json!(self.support_verbosity));
        set_or_remove_str(model, "default_verbosity", &self.default_verbosity);
        catalog::set(
            model,
            &["supports_parallel_tool_calls"],
            json!(self.supports_parallel_tool_calls),
        );
        catalog::set(
            model,
            &["supports_image_detail_original"],
            json!(self.supports_image_detail_original),
        );
        catalog::set(model, &["use_responses_lite"], json!(self.use_responses_lite));
        catalog::set(
            model,
            &["include_skills_usage_instructions"],
            json!(self.include_skills_usage_instructions),
        );
        catalog::set(model, &["prefer_websockets"], json!(self.prefer_websockets));
        catalog::set(model, &["supported_in_api"], json!(self.supported_in_api));
        set_or_remove_str(model, "comp_hash", &self.comp_hash);
        set_or_remove_str(model, "auto_review_model_override", &self.auto_review_model_override);

        if self.nux_message.trim().is_empty() {
            catalog::remove(model, &["availability_nux"]);
        } else {
            catalog::set(
                model,
                &["availability_nux", "message"],
                Value::String(self.nux_message.clone()),
            );
        }
        set_or_remove_str(model, "base_instructions", &self.base_instructions);
    }

    /// Problems with this specific model, shown inline in the editor.
    pub fn problems(&self) -> Vec<String> {
        let mut problems = Vec::new();
        let slug = self.slug.trim();
        if slug.is_empty() {
            problems.push("slug（模型标识）不能为空，Codex 用它来找到这个模型。".into());
        } else if slug.contains(char::is_whitespace) {
            problems.push("slug 里不能有空格，建议用小写字母、数字、点和短横线，例如 gpt-5.2-codex。".into());
        }
        if !self.context_window.is_empty()
            && crate::ui::widgets::parse_opt_int(&self.context_window).is_none()
        {
            problems.push("上下文窗口必须是数字，例如 200000。".into());
        }
        if self.supported_reasoning_levels.is_empty() {
            problems.push("没有勾选任何推理强度，Codex 里将无法选择 /model 的推理档位。".into());
        } else if !self.default_reasoning_level.is_empty()
            && !self
                .supported_reasoning_levels
                .iter()
                .any(|level| *level == self.default_reasoning_level)
        {
            problems.push("默认推理强度不在已勾选的档位里。".into());
        }
        problems
    }
}

// ---------------------------------------------------------------------------
// Provider editor
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthMode {
    EnvKey,
    Bearer,
    CodexLogin,
    None,
}

impl AuthMode {
    pub const ALL: [AuthMode; 4] = [AuthMode::EnvKey, AuthMode::Bearer, AuthMode::CodexLogin, AuthMode::None];

    pub fn label(self) -> &'static str {
        match self {
            AuthMode::EnvKey => "从环境变量读取（推荐）",
            AuthMode::Bearer => "直接写在配置文件里",
            AuthMode::CodexLogin => "使用 Codex 登录态",
            AuthMode::None => "不需要密钥",
        }
    }

    pub fn help(self) -> &'static str {
        match self {
            AuthMode::EnvKey => {
                "密钥存在环境变量里（例如 OPENAI_API_KEY），配置文件只记录变量名。\n好处：配置文件可以放心分享、提交到 git，不会泄露密钥。"
            }
            AuthMode::Bearer => {
                "密钥直接写进 config.toml 的 experimental_bearer_token。\n方便但要注意：任何能看到这个文件的人都能拿到你的密钥。"
            }
            AuthMode::CodexLogin => {
                "使用 codex login 登录后保存在 auth.json 里的凭证（requires_openai_auth = true）。\n公司内部代理经常这么用。"
            }
            AuthMode::None => "本地服务（Ollama、LM Studio）通常不需要任何密钥。",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProviderEditor {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub wire_api: WireApi,
    pub auth_mode: AuthMode,
    pub env_key: String,
    pub env_key_instructions: String,
    pub bearer: String,
    pub chat_stream: bool,
    pub requires_openai_auth: bool,
    pub supports_websockets: bool,
    pub request_max_retries: String,
    pub stream_max_retries: String,
    pub stream_idle_timeout_ms: String,
    pub headers: Vec<(String, String)>,
    pub query_params: Vec<(String, String)>,
    pub env_http_headers: Vec<(String, String)>,
    pub open_advanced: bool,
}

impl ProviderEditor {
    pub fn from_view(view: &ProviderView) -> Self {
        let auth_mode = if !view.env_key.is_empty() {
            AuthMode::EnvKey
        } else if !view.bearer_token.is_empty() {
            AuthMode::Bearer
        } else if view.requires_openai_auth {
            AuthMode::CodexLogin
        } else {
            AuthMode::None
        };
        Self {
            id: view.id.clone(),
            name: view.name.clone(),
            base_url: view.base_url.clone(),
            wire_api: view.wire(),
            auth_mode,
            env_key: if view.env_key.is_empty() {
                default_env_key(&view.base_url)
            } else {
                view.env_key.clone()
            },
            env_key_instructions: view.env_key_instructions.clone(),
            bearer: view.bearer_token.clone(),
            chat_stream: view.chat_stream,
            requires_openai_auth: view.requires_openai_auth,
            supports_websockets: view.supports_websockets,
            request_max_retries: num_str(view.request_max_retries),
            stream_max_retries: num_str(view.stream_max_retries),
            stream_idle_timeout_ms: num_str(view.stream_idle_timeout_ms),
            headers: view.headers.clone(),
            query_params: view.query_params.clone(),
            env_http_headers: view.env_http_headers.clone(),
            open_advanced: false,
        }
    }

    pub fn from_template(id: &str, template: &crate::doc::schema::ProviderTemplate) -> Self {
        Self {
            id: id.to_string(),
            name: template.name.to_string(),
            base_url: template.base_url.to_string(),
            wire_api: template.wire_api,
            auth_mode: match template.env_key {
                Some(_) => AuthMode::EnvKey,
                None if template.requires_openai_auth => AuthMode::CodexLogin,
                None => AuthMode::None,
            },
            env_key: template.env_key.unwrap_or("").to_string(),
            env_key_instructions: String::new(),
            bearer: String::new(),
            chat_stream: template.chat_stream,
            requires_openai_auth: template.requires_openai_auth,
            supports_websockets: false,
            request_max_retries: String::new(),
            stream_max_retries: String::new(),
            stream_idle_timeout_ms: String::new(),
            headers: template
                .headers
                .iter()
                .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
                .collect(),
            query_params: template
                .query_params
                .iter()
                .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
                .collect(),
            env_http_headers: Vec::new(),
            open_advanced: false,
        }
    }

    pub fn write_to(&self, config: &mut DocumentMut) {
        let id = self.id.clone();
        providers_mod::create(config, &id);
        providers::set_str(config, &id, "name", &self.name);
        providers::set_str(config, &id, "base_url", self.base_url.trim());
        providers::set_str(config, &id, "wire_api", self.wire_api.value());

        // Authentication
        match self.auth_mode {
            AuthMode::EnvKey => {
                providers::set_str(config, &id, "env_key", &self.env_key);
                providers::set_str(config, &id, "env_key_instructions", &self.env_key_instructions);
                config.remove_at(&["model_providers", &id, "experimental_bearer_token"]);
                providers::set_bool(config, &id, "requires_openai_auth", false);
            }
            AuthMode::Bearer => {
                config.remove_at(&["model_providers", &id, "env_key"]);
                config.remove_at(&["model_providers", &id, "env_key_instructions"]);
                providers::set_str(config, &id, "experimental_bearer_token", &self.bearer);
                providers::set_bool(config, &id, "requires_openai_auth", false);
            }
            AuthMode::CodexLogin => {
                config.remove_at(&["model_providers", &id, "env_key"]);
                config.remove_at(&["model_providers", &id, "env_key_instructions"]);
                config.remove_at(&["model_providers", &id, "experimental_bearer_token"]);
                providers::set_bool(config, &id, "requires_openai_auth", true);
            }
            AuthMode::None => {
                config.remove_at(&["model_providers", &id, "env_key"]);
                config.remove_at(&["model_providers", &id, "env_key_instructions"]);
                config.remove_at(&["model_providers", &id, "experimental_bearer_token"]);
                providers::set_bool(config, &id, "requires_openai_auth", false);
            }
        }

        providers::set_bool(config, &id, "chat_stream", self.chat_stream);
        providers::set_bool(config, &id, "supports_websockets", self.supports_websockets);
        providers::set_opt_int(
            config,
            &id,
            "request_max_retries",
            crate::ui::widgets::parse_opt_int(&self.request_max_retries),
        );
        providers::set_opt_int(
            config,
            &id,
            "stream_max_retries",
            crate::ui::widgets::parse_opt_int(&self.stream_max_retries),
        );
        providers::set_opt_int(
            config,
            &id,
            "stream_idle_timeout_ms",
            crate::ui::widgets::parse_opt_int(&self.stream_idle_timeout_ms),
        );
        providers::set_map(config, &id, "http_headers", &self.headers);
        providers::set_map(config, &id, "query_params", &self.query_params);
        providers::set_map(config, &id, "env_http_headers", &self.env_http_headers);
    }

    pub fn problems(&self) -> Vec<String> {
        let mut problems = Vec::new();
        if self.base_url.trim().is_empty() {
            problems.push("base_url 还没有填写。".into());
        } else if !self.base_url.starts_with("http://") && !self.base_url.starts_with("https://") {
            problems.push("base_url 需要以 http:// 或 https:// 开头。".into());
        }
        if self.auth_mode == AuthMode::EnvKey && self.env_key.trim().is_empty() {
            problems.push("选择了「从环境变量读取」，但还没有填变量名（例如 OPENAI_API_KEY）。".into());
        }
        if self.auth_mode == AuthMode::Bearer && self.bearer.trim().is_empty() {
            problems.push("选择了「直接写在配置文件里」，但 Token 是空的。".into());
        }
        problems
    }
}

fn default_env_key(base_url: &str) -> String {
    let lower = base_url.to_lowercase();
    if lower.contains("deepseek") {
        "DEEPSEEK_API_KEY".into()
    } else if lower.contains("anthropic") || lower.contains("claude") {
        "ANTHROPIC_API_KEY".into()
    } else if lower.contains("moonshot") {
        "MOONSHOT_API_KEY".into()
    } else if lower.contains("googleapis") || lower.contains("gemini") {
        "GEMINI_API_KEY".into()
    } else if lower.contains("azure") {
        "AZURE_OPENAI_API_KEY".into()
    } else {
        "OPENAI_API_KEY".into()
    }
}

// ---------------------------------------------------------------------------
// Profile editor
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct ProfileEditor {
    pub name: String,
    pub model: String,
    pub model_provider: String,
    pub model_reasoning_effort: String,
    pub model_reasoning_summary: String,
    pub model_verbosity: String,
    pub approval_policy: String,
    pub sandbox_mode: String,
    pub personality: String,
}

impl ProfileEditor {
    pub fn from_config(config: &DocumentMut, name: &str) -> Self {
        let at = |key: &str| config.str_at(&["profiles", name, key]).unwrap_or_default();
        Self {
            name: name.to_string(),
            model: at("model"),
            model_provider: at("model_provider"),
            model_reasoning_effort: at("model_reasoning_effort"),
            model_reasoning_summary: at("model_reasoning_summary"),
            model_verbosity: at("model_verbosity"),
            approval_policy: at("approval_policy"),
            sandbox_mode: at("sandbox_mode"),
            personality: at("personality"),
        }
    }

    pub fn write_to(&self, config: &mut DocumentMut) {
        let name = self.name.clone();
        config.ensure_table_at(&["profiles", &name]);
        for (key, value) in [
            ("model", &self.model),
            ("model_provider", &self.model_provider),
            ("model_reasoning_effort", &self.model_reasoning_effort),
            ("model_reasoning_summary", &self.model_reasoning_summary),
            ("model_verbosity", &self.model_verbosity),
            ("approval_policy", &self.approval_policy),
            ("sandbox_mode", &self.sandbox_mode),
            ("personality", &self.personality),
        ] {
            let path = ["profiles", name.as_str(), key];
            if value.trim().is_empty() {
                config.remove_at(&path);
            } else {
                config.set_value_at(&path, TomlValue::from(value.trim().to_string()));
            }
        }
    }
}
