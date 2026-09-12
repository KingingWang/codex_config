//! Which screen of the app is currently visible.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Page {
    Overview,
    Models,
    Providers,
    Profiles,
    Advanced,
    Raw,
}

impl Page {
    pub const ALL: [Page; 6] = [
        Page::Overview,
        Page::Models,
        Page::Providers,
        Page::Profiles,
        Page::Advanced,
        Page::Raw,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Page::Overview => "基础设置",
            Page::Models => "模型管理",
            Page::Providers => "服务商",
            Page::Profiles => "配置档",
            Page::Advanced => "高级选项",
            Page::Raw => "源文件编辑",
        }
    }

    pub fn subtitle(self) -> &'static str {
        match self {
            Page::Overview => "最常用的几个开关：用哪个模型、连哪个服务商、权限多大",
            Page::Models => "添加、修改、删除 Codex 能选择的模型（写入模型目录 JSON）",
            Page::Providers => "配置模型服务商：地址、协议、密钥（写入 config.toml）",
            Page::Profiles => "把「模型 + 服务商 + 权限」存成一组，一键切换",
            Page::Advanced => "实验功能、终端界面、MCP 服务等进阶开关",
            Page::Raw => "直接看和改原始文件，保存前可以对比差异",
        }
    }

    pub fn icon(self) -> &'static str {
        use crate::ui::icons;
        match self {
            Page::Overview => icons::OVERVIEW,
            Page::Models => icons::MODELS,
            Page::Providers => icons::PROVIDERS,
            Page::Profiles => icons::PROFILES,
            Page::Advanced => icons::ADVANCED,
            Page::Raw => icons::RAW,
        }
    }
}
