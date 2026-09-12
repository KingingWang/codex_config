//! Human friendly metadata for every setting this GUI can edit.
//!
//! The goal is that a first time user can understand a setting from the label +
//! help text alone, without reading the Codex source.

/// One selectable value of an enum setting.
#[derive(Debug, Clone, Copy)]
pub struct Choice {
    /// Value written to the config file.
    pub value: &'static str,
    /// What the user sees in the dropdown.
    pub label: &'static str,
    /// One or two sentences shown under the dropdown / in the tooltip.
    pub help: &'static str,
}

impl Choice {
    pub fn find(list: &[Choice], value: &str) -> Option<&'static str> {
        list.iter().find(|c| c.value == value).map(|c| c.label)
    }
}

pub fn choice_label(list: &[Choice], value: &str) -> String {
    Choice::find(list, value)
        .map(|label| format!("{label} ({value})"))
        .unwrap_or_else(|| value.to_string())
}

pub fn choice_help(list: &[Choice], value: &str) -> &'static str {
    list.iter()
        .find(|c| c.value == value)
        .map(|c| c.help)
        .unwrap_or("")
}

/// Sentinel shown in dropdowns when a key is absent from the file.
pub const UNSET: &str = "\u{2014} 未设置（使用 Codex 默认值）";

// ---------------------------------------------------------------------------
// Top level settings
// ---------------------------------------------------------------------------

pub const SANDBOX_MODES: &[Choice] = &[
    Choice {
        value: "read-only",
        label: "只读模式（最安全）",
        help: "Codex 只能读文件，不能改文件、不能执行命令。适合第一次试用。",
    },
    Choice {
        value: "workspace-write",
        label: "工作区可写（推荐）",
        help: "可以修改当前项目里的文件并运行命令，但不能碰项目外的东西。日常使用推荐。",
    },
    Choice {
        value: "danger-full-access",
        label: "完全访问（危险）",
        help: "不做任何限制，可以改任何文件、执行任何命令。只有在完全信任的环境里才用。",
    },
];

pub const APPROVAL_POLICIES: &[Choice] = &[
    Choice {
        value: "on-request",
        label: "模型决定何时询问（默认）",
        help: "模型自己判断哪些操作需要征求你的同意，平衡安全和效率。",
    },
    Choice {
        value: "untrusted",
        label: "不信任的命令都要问",
        help: "除了明确允许的命令，其它命令执行前都会弹窗问你。更保守。",
    },
    Choice {
        value: "never",
        label: "从不询问",
        help: "完全不弹窗，出错就直接返回给模型。适合自动化/无人值守场景。",
    },
];

pub const REASONING_EFFORTS: &[Choice] = &[
    Choice {
        value: "none",
        label: "none 不推理",
        help: "不做额外思考，响应最快。",
    },
    Choice {
        value: "minimal",
        label: "minimal 极简",
        help: "几乎不增加延迟的轻量思考。",
    },
    Choice {
        value: "low",
        label: "low 低",
        help: "轻量推理，日常小任务足够快。",
    },
    Choice {
        value: "medium",
        label: "medium 中（常用）",
        help: "平衡速度和思考深度。",
    },
    Choice {
        value: "high",
        label: "high 高",
        help: "更深入的思考，适合复杂问题。",
    },
    Choice {
        value: "xhigh",
        label: "xhigh 超高",
        help: "超高强度推理，适合难题。",
    },
    Choice {
        value: "max",
        label: "max 最大",
        help: "最大推理强度，最难的问题。",
    },
    Choice {
        value: "ultra",
        label: "ultra 极致",
        help: "最大推理强度并自动拆分子任务给子代理。",
    },
    Choice {
        value: "persistent",
        label: "persistent 持续",
        help: "持续任务模式使用的推理强度。",
    },
];

pub const REASONING_SUMMARIES: &[Choice] = &[
    Choice {
        value: "auto",
        label: "auto 自动",
        help: "由 Codex 决定是否显示思考摘要（推荐）。",
    },
    Choice {
        value: "concise",
        label: "concise 简洁",
        help: "只显示一句话的思考摘要。",
    },
    Choice {
        value: "detailed",
        label: "detailed 详细",
        help: "显示更完整的思考过程。",
    },
    Choice {
        value: "none",
        label: "none 不显示",
        help: "完全不显示模型的思考摘要。",
    },
];

pub const VERBOSITIES: &[Choice] = &[
    Choice {
        value: "low",
        label: "low 简洁",
        help: "回答尽量简短。",
    },
    Choice {
        value: "medium",
        label: "medium 适中",
        help: "默认的详细程度。",
    },
    Choice {
        value: "high",
        label: "high 详细",
        help: "回答更详细、解释更多。",
    },
];

pub const PERSONALITIES: &[Choice] = &[
    Choice {
        value: "friendly",
        label: "friendly 友好",
        help: "语气更亲切，重视团队协作氛围。",
    },
    Choice {
        value: "pragmatic",
        label: "pragmatic 务实",
        help: "直接、注重把事做完的工程师风格。",
    },
    Choice {
        value: "none",
        label: "none 无人格",
        help: "使用模型自带的默认提示词，不注入人格。",
    },
];

pub const FILE_OPENERS: &[Choice] = &[
    Choice {
        value: "vscode",
        label: "VS Code",
        help: "点击文件链接时用 VS Code 打开。",
    },
    Choice {
        value: "vscode-insiders",
        label: "VS Code Insiders",
        help: "用 VS Code Insiders 打开。",
    },
    Choice {
        value: "windsurf",
        label: "Windsurf",
        help: "用 Windsurf 打开。",
    },
    Choice {
        value: "cursor",
        label: "Cursor",
        help: "用 Cursor 打开。",
    },
    Choice {
        value: "none",
        label: "none 不打开",
        help: "文件链接不可点击。",
    },
];

pub const WEB_SEARCH_MODES: &[Choice] = &[
    Choice {
        value: "disabled",
        label: "disabled 关闭",
        help: "不允许联网搜索。",
    },
    Choice {
        value: "cached",
        label: "cached 缓存",
        help: "默认值：允许使用缓存的搜索结果。",
    },
    Choice {
        value: "indexed",
        label: "indexed 索引",
        help: "使用索引搜索。",
    },
    Choice {
        value: "live",
        label: "live 实时",
        help: "实时联网搜索，最新但更慢。",
    },
];

// ---------------------------------------------------------------------------
// Model catalog enums
// ---------------------------------------------------------------------------

pub const MODEL_VISIBILITY: &[Choice] = &[
    Choice {
        value: "list",
        label: "list 显示",
        help: "在 /model 列表里显示，可正常选择（推荐）。",
    },
    Choice {
        value: "hide",
        label: "hide 隐藏",
        help: "不在列表里显示，但仍可以用名字选到。",
    },
    Choice {
        value: "none",
        label: "none 不可用",
        help: "完全禁用这个模型。",
    },
];

pub const SHELL_TYPES: &[Choice] = &[
    Choice {
        value: "shell_command",
        label: "shell_command 允许执行命令",
        help: "模型可以运行终端命令（正常用法）。",
    },
    Choice {
        value: "disabled",
        label: "disabled 禁用命令",
        help: "禁止模型执行任何命令，只对话和改文件。",
    },
];

pub const WEB_SEARCH_TOOL_TYPES: &[Choice] = &[
    Choice {
        value: "text",
        label: "text 只搜文本",
        help: "联网搜索只返回文本结果。",
    },
    Choice {
        value: "text_and_image",
        label: "text_and_image 文本+图片",
        help: "搜索结果里可以包含图片。",
    },
];

pub const TOOL_MODES: &[Choice] = &[
    Choice {
        value: "direct",
        label: "direct 直接调用",
        help: "模型直接调用工具（最常见）。",
    },
    Choice {
        value: "code_mode",
        label: "code_mode 代码模式",
        help: "模型通过写代码来调用工具。",
    },
    Choice {
        value: "code_mode_only",
        label: "code_mode_only 仅代码模式",
        help: "只允许通过写代码调用工具。",
    },
];

pub const INPUT_MODALITIES: &[Choice] = &[
    Choice {
        value: "text",
        label: "text 文本",
        help: "支持发送文字。几乎所有模型都支持。",
    },
    Choice {
        value: "image",
        label: "image 图片",
        help: "支持发送截图/图片。做前端或调试 UI 时很有用。",
    },
    Choice {
        value: "audio",
        label: "audio 音频",
        help: "支持发送音频。",
    },
];

pub const TRUNCATION_MODES: &[Choice] = &[
    Choice {
        value: "tokens",
        label: "tokens 按 token 截断",
        help: "工具输出超过多少 token 就截断。",
    },
    Choice {
        value: "bytes",
        label: "bytes 按字节截断",
        help: "工具输出超过多少字节就截断。",
    },
];

pub const APPLY_PATCH_TOOL_TYPES: &[Choice] = &[Choice {
    value: "freeform",
    label: "freeform 自由格式补丁",
    help: "让模型直接输出补丁文本，兼容性最好（推荐）。",
}];

// ---------------------------------------------------------------------------
// Wire protocols (the "三种协议" the user asked for)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WireApi {
    Responses,
    Chat,
    Anthropic,
}

impl WireApi {
    pub const ALL: [WireApi; 3] = [WireApi::Responses, WireApi::Chat, WireApi::Anthropic];

    pub fn value(self) -> &'static str {
        match self {
            WireApi::Responses => "responses",
            WireApi::Chat => "chat",
            WireApi::Anthropic => "anthropic",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "responses" => Some(Self::Responses),
            "chat" => Some(Self::Chat),
            "anthropic" => Some(Self::Anthropic),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Responses => "Responses 协议",
            Self::Chat => "Chat Completions 协议",
            Self::Anthropic => "Anthropic 协议",
        }
    }

    pub fn endpoint(self) -> &'static str {
        match self {
            Self::Responses => "POST {base_url}/responses",
            Self::Chat => "POST {base_url}/chat/completions",
            Self::Anthropic => "POST {base_url}/v1/messages",
        }
    }

    pub fn help(self) -> &'static str {
        match self {
            Self::Responses => {
                "OpenAI 官方最新的接口协议，功能最完整（推理摘要、并行工具调用等）。\n只有 OpenAI 官方、Azure OpenAI 以及少数中转站支持。"
            }
            Self::Chat => {
                "最通用的 OpenAI 兼容协议，绝大多数第三方服务、中转站、本地推理引擎都用它。\n不确定选哪个时，先试这个。"
            }
            Self::Anthropic => {
                "Anthropic Claude 的 Messages 接口协议。\n只有在接 Claude 系列模型（官方或代理）时才选它。"
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Provider templates
// ---------------------------------------------------------------------------

/// A one click starting point for `[model_providers.*]`.
#[derive(Debug, Clone)]
pub struct ProviderTemplate {
    pub key: &'static str,
    pub title: &'static str,
    pub summary: &'static str,
    pub name: &'static str,
    pub base_url: &'static str,
    pub wire_api: WireApi,
    pub env_key: Option<&'static str>,
    pub chat_stream: bool,
    pub requires_openai_auth: bool,
    pub headers: &'static [(&'static str, &'static str)],
    pub query_params: &'static [(&'static str, &'static str)],
}

const EMPTY: &[(&str, &str)] = &[];

pub const PROVIDER_TEMPLATES: &[ProviderTemplate] = &[
    ProviderTemplate {
        key: "openai",
        title: "OpenAI 官方",
        summary: "用 OpenAI 官方 API Key 直连 GPT / Codex 模型。",
        name: "OpenAI",
        base_url: "https://api.openai.com/v1",
        wire_api: WireApi::Responses,
        env_key: Some("OPENAI_API_KEY"),
        chat_stream: false,
        requires_openai_auth: false,
        headers: EMPTY,
        query_params: EMPTY,
    },
    ProviderTemplate {
        key: "anthropic",
        title: "Anthropic 官方 (Claude)",
        summary: "用 Anthropic API Key 直连 Claude 模型。",
        name: "Anthropic",
        base_url: "https://api.anthropic.com",
        wire_api: WireApi::Anthropic,
        env_key: Some("ANTHROPIC_API_KEY"),
        chat_stream: false,
        requires_openai_auth: false,
        headers: EMPTY,
        query_params: EMPTY,
    },
    ProviderTemplate {
        key: "relay-chat",
        title: "OpenAI 兼容中转站（Chat 协议）",
        summary: "第三方中转 / 公司内部代理，最常见的一种。",
        name: "中转站",
        base_url: "https://your-relay.example.com/v1",
        wire_api: WireApi::Chat,
        env_key: None,
        chat_stream: true,
        requires_openai_auth: false,
        headers: EMPTY,
        query_params: EMPTY,
    },
    ProviderTemplate {
        key: "relay-responses",
        title: "OpenAI 兼容中转站（Responses 协议）",
        summary: "中转站明确支持 /responses 接口时使用，功能更全。",
        name: "中转站 (Responses)",
        base_url: "https://your-relay.example.com/v1",
        wire_api: WireApi::Responses,
        env_key: None,
        chat_stream: false,
        requires_openai_auth: false,
        headers: EMPTY,
        query_params: EMPTY,
    },
    ProviderTemplate {
        key: "ollama",
        title: "本地 Ollama",
        summary: "在本机跑开源模型，完全离线、不花钱。",
        name: "Ollama (本地)",
        base_url: "http://localhost:11434/v1",
        wire_api: WireApi::Responses,
        env_key: None,
        chat_stream: true,
        requires_openai_auth: false,
        headers: EMPTY,
        query_params: EMPTY,
    },
    ProviderTemplate {
        key: "lmstudio",
        title: "本地 LM Studio",
        summary: "LM Studio 自带的本地 OpenAI 兼容服务。",
        name: "LM Studio (本地)",
        base_url: "http://localhost:1234/v1",
        wire_api: WireApi::Responses,
        env_key: None,
        chat_stream: true,
        requires_openai_auth: false,
        headers: EMPTY,
        query_params: EMPTY,
    },
    ProviderTemplate {
        key: "deepseek",
        title: "DeepSeek 深度求索",
        summary: "国产模型，OpenAI 兼容，价格便宜。",
        name: "DeepSeek",
        base_url: "https://api.deepseek.com/v1",
        wire_api: WireApi::Chat,
        env_key: Some("DEEPSEEK_API_KEY"),
        chat_stream: true,
        requires_openai_auth: false,
        headers: EMPTY,
        query_params: EMPTY,
    },
    ProviderTemplate {
        key: "moonshot",
        title: "Moonshot (Kimi)",
        summary: "月之暗面 Kimi，OpenAI 兼容协议。",
        name: "Moonshot",
        base_url: "https://api.moonshot.cn/v1",
        wire_api: WireApi::Chat,
        env_key: Some("MOONSHOT_API_KEY"),
        chat_stream: true,
        requires_openai_auth: false,
        headers: EMPTY,
        query_params: EMPTY,
    },
    ProviderTemplate {
        key: "gemini",
        title: "Google Gemini（OpenAI 兼容层）",
        summary: "通过 Gemini 的 OpenAI 兼容端点使用 Google 模型。",
        name: "Gemini",
        base_url: "https://generativelanguage.googleapis.com/v1beta/openai",
        wire_api: WireApi::Chat,
        env_key: Some("GEMINI_API_KEY"),
        chat_stream: true,
        requires_openai_auth: false,
        headers: EMPTY,
        query_params: EMPTY,
    },
    ProviderTemplate {
        key: "azure_openai",
        title: "Azure OpenAI",
        summary: "公司部署在 Azure 上的 OpenAI 服务。",
        name: "Azure OpenAI",
        base_url: "https://YOUR-RESOURCE.openai.azure.com/openai",
        wire_api: WireApi::Responses,
        env_key: Some("AZURE_OPENAI_API_KEY"),
        chat_stream: false,
        requires_openai_auth: false,
        headers: EMPTY,
        query_params: &[("api-version", "2025-04-01-preview")],
    },
];
