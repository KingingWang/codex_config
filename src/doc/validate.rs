//! "Health check" for the current (possibly unsaved) configuration.

use std::collections::BTreeSet;

use crate::doc::catalog;
use crate::doc::providers;
use crate::doc::toml_ext::TomlPathExt;
use crate::doc::{Document, BUILTIN_PROVIDER_IDS};
use crate::page::Page;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Error,
    Warning,
    Info,
}

impl Severity {
    pub fn label(self) -> &'static str {
        match self {
            Severity::Error => "错误",
            Severity::Warning => "警告",
            Severity::Info => "提示",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Issue {
    pub severity: Severity,
    pub page: Page,
    pub title: String,
    pub detail: String,
}

impl Issue {
    fn error(page: Page, title: impl Into<String>, detail: impl Into<String>) -> Self {
        Self { severity: Severity::Error, page, title: title.into(), detail: detail.into() }
    }
    fn warn(page: Page, title: impl Into<String>, detail: impl Into<String>) -> Self {
        Self { severity: Severity::Warning, page, title: title.into(), detail: detail.into() }
    }
    fn info(page: Page, title: impl Into<String>, detail: impl Into<String>) -> Self {
        Self { severity: Severity::Info, page, title: title.into(), detail: detail.into() }
    }
}

pub fn validate(doc: &Document) -> Vec<Issue> {
    let mut issues: Vec<Issue> = doc
        .load_notes
        .iter()
        .map(|note| Issue::warn(Page::Overview, "载入时的小问题", note.clone()))
        .collect();

    let provider_ids: BTreeSet<String> = doc.provider_ids().into_iter().collect();
    let known_providers = |id: &str| provider_ids.contains(id) || BUILTIN_PROVIDER_IDS.contains(&id);

    // --- providers ---------------------------------------------------------
    if provider_ids.is_empty() {
        issues.push(Issue::warn(
            Page::Providers,
            "还没有配置任何服务商",
            "Codex 需要知道从哪里取模型。去「服务商」页选一个模板（比如 OpenAI 官方或本地 Ollama）一键添加。",
        ));
    }

    for provider in providers::all(&doc.config) {
        if provider.base_url.is_empty() && !BUILTIN_PROVIDER_IDS.contains(&provider.id.as_str()) {
            issues.push(Issue::error(
                Page::Providers,
                format!("服务商 {} 没有填 base_url", provider.id),
                "base_url 是模型服务的接口地址，没有它 Codex 不知道往哪里发请求。",
            ));
        } else if !provider.base_url.is_empty()
            && !provider.base_url.starts_with("http://")
            && !provider.base_url.starts_with("https://")
        {
            issues.push(Issue::error(
                Page::Providers,
                format!("服务商 {} 的地址看起来不对", provider.id),
                format!(
                    "「{}」应该以 http:// 或 https:// 开头，例如 https://api.openai.com/v1",
                    provider.base_url
                ),
            ));
        }

        if !provider.has_known_wire_api() {
            issues.push(Issue::error(
                Page::Providers,
                format!("服务商 {} 的 wire_api 无法识别", provider.id),
                format!(
                    "「{}」不是合法值，只能是 responses、chat 或 anthropic。",
                    provider.wire_api
                ),
            ));
        }

        if provider.wire() == crate::doc::schema::WireApi::Anthropic
            && provider.base_url.ends_with("/v1")
        {
            issues.push(Issue::warn(
                Page::Providers,
                format!("服务商 {} 的地址可能多了 /v1", provider.id),
                "Anthropic 协议会自己拼上 /v1/messages，base_url 一般填到域名即可，例如 https://api.anthropic.com",
            ));
        }

        if provider.wire() == crate::doc::schema::WireApi::Chat && !provider.chat_stream {
            issues.push(Issue::info(
                Page::Providers,
                format!("服务商 {} 没开流式输出", provider.id),
                "Chat 协议下 chat_stream = false 时回答会等全部生成完才显示，看起来像卡住了。一般建议打开。",
            ));
        }

        if !provider.env_key.is_empty() && std::env::var(&provider.env_key).is_err() {
            issues.push(Issue::info(
                Page::Providers,
                format!("环境变量 {} 当前没有值", provider.env_key),
                format!(
                    "服务商 {} 会从环境变量 {key} 读密钥。如果你已经在终端里 export 过就可以忽略；\
否则 Codex 启动时会提示你输入。也可以改用「直接填写 Token」。",
                    provider.id,
                    key = provider.env_key
                ),
            ));
        }

        if provider.bearer_token.is_empty()
            && provider.env_key.is_empty()
            && !provider.requires_openai_auth
            && !provider.base_url.contains("localhost")
            && !provider.base_url.contains("127.0.0.1")
            && !provider.base_url.is_empty()
        {
            issues.push(Issue::info(
                Page::Providers,
                format!("服务商 {} 没有配置任何密钥", provider.id),
                "如果是本地服务通常不需要密钥；如果是线上服务，记得填 API Key 或勾选「使用 Codex 登录态」。",
            ));
        }
    }

    // --- active model / provider -------------------------------------------
    let active_provider = doc.config.str_at(&["model_provider"]).unwrap_or_default();
    if !active_provider.is_empty() && !known_providers(&active_provider) {
        issues.push(Issue::error(
            Page::Overview,
            "当前选中的服务商不存在",
            format!(
                "config.toml 里 model_provider = \"{active_provider}\"，但 [model_providers.{active_provider}] 没有定义。\
去「服务商」页添加它，或者在「基础设置」里换一个。"
            ),
        ));
    }

    let active_model = doc.config.str_at(&["model"]).unwrap_or_default();
    let catalog_models = doc.catalog.as_ref();
    if let Some(catalog) = catalog_models {
        let slugs = catalog::slugs(catalog);
        if !active_model.is_empty() && !slugs.iter().any(|s| s == &active_model) {
            issues.push(Issue::warn(
                Page::Overview,
                "当前模型不在模型目录里",
                format!(
                    "config.toml 里 model = \"{active_model}\"，但模型目录中没有这个 slug。\
Codex 会用内置的兜底信息，可能表现不正常。建议去「模型管理」页添加或改回已有的模型。"
                ),
            ));
        }

        // duplicate slugs
        let mut seen: BTreeSet<String> = BTreeSet::new();
        for slug in &slugs {
            if !seen.insert(slug.clone()) {
                issues.push(Issue::error(
                    Page::Models,
                    "模型目录里有重复的 slug",
                    format!("「{slug}」出现了多次，Codex 只会用其中一个。请删掉多余的。"),
                ));
            }
        }

        // every model's provider must exist
        if let Some(list) = catalog::models(catalog) {
            for model in list {
                let slug = catalog::slug_of(model);
                if let Some(provider) = model.get("provider").and_then(|v| v.as_str()) {
                    if !provider.is_empty() && !known_providers(provider) {
                        issues.push(Issue::warn(
                            Page::Models,
                            format!("模型 {slug} 绑定的服务商不存在"),
                            format!(
                                "模型 {slug} 写着 provider = \"{provider}\"，但配置里没有这个服务商。\
选中它时会连不上，请去「服务商」页添加 {provider}，或在模型编辑里换一个。"
                            ),
                        ));
                    }
                }
            }
        }

        // reasoning effort compatibility
        if let Some(effort) = doc.config.str_at(&["model_reasoning_effort"])
            && let Some(index) = catalog::find_index(catalog, &active_model)
            && let Some(model) = catalog::models(catalog).and_then(|list| list.get(index))
        {
            let levels = catalog::reasoning_levels(model);
            if !levels.is_empty() && !levels.iter().any(|l| *l == effort) {
                issues.push(Issue::warn(
                    Page::Overview,
                    "推理强度和模型不匹配",
                    format!(
                        "model_reasoning_effort = \"{effort}\"，但模型 {active_model} 只支持 {}。\
切到别的强度，或者在模型编辑里勾上这一档。",
                        levels.join(" / ")
                    ),
                ));
            }
        }
    } else if doc.catalog_path.is_some() {
        issues.push(Issue::error(
            Page::Models,
            "模型目录读不出来",
            "config.toml 指定了 model_catalog_json，但文件不存在或不是合法 JSON。\
去「模型管理」页可以新建一个空的模型目录。",
        ));
    } else {
        issues.push(Issue::info(
            Page::Models,
            "没有使用自定义模型目录",
            "config.toml 里没有设置 model_catalog_json，Codex 会用内置的模型列表。\
想自己加模型的话，去「模型管理」页点「创建模型目录」。",
        ));
    }

    // --- safety -------------------------------------------------------------
    if doc.config.str_at(&["sandbox_mode"]).as_deref() == Some("danger-full-access") {
        issues.push(Issue::warn(
            Page::Overview,
            "当前是完全访问模式",
            "sandbox_mode = \"danger-full-access\" 表示 Codex 可以修改任何文件、执行任何命令。\
只在你完全信任的机器上这样用；日常推荐 workspace-write。",
        ));
    }
    if doc.config.str_at(&["approval_policy"]).as_deref() == Some("never") {
        issues.push(Issue::warn(
            Page::Overview,
            "审批策略是「从不询问」",
            "approval_policy = \"never\" 时 Codex 不会在执行敏感操作前征求同意。",
        ));
    }

    issues.sort_by_key(|issue| issue.severity);
    issues
}
