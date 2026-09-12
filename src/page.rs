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
            Page::Overview => "开始使用",
            Page::Models => "模型管理",
            Page::Providers => "服务商",
            Page::Profiles => "配置档",
            Page::Advanced => "高级选项",
            Page::Raw => "源文件编辑",
        }
    }

    pub fn subtitle(self) -> &'static str {
        match self {
            Page::Overview => "选好模型，放心开始。复杂的设置，交给我们整理。",
            Page::Models => "挑一个适合你的 AI。可以随时切换，也可以添加新模型。",
            Page::Providers => "连接你的 AI 服务。选好模板，再填入自己的地址和密钥。",
            Page::Profiles => "工作、学习、日常使用，把常用设置存成一组。",
            Page::Advanced => "按需调整进阶功能。不确定的选项，保持默认就好。",
            Page::Raw => "为熟悉配置文件的你保留。修改后先应用，再保存。",
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
