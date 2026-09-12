//! 「高级选项」— feature flags, TUI tweaks, extra instructions and MCP servers.

use egui::{Context, RichText, Ui};
use toml_edit::{Array, Item, Value as TomlValue};

use crate::app::App;
use crate::doc::schema::Choice;
use crate::doc::toml_ext::{TomlPathExt, read_string_map, value_str_array};
use crate::ui::icons;
use crate::ui::pages::settings;
use crate::ui::theme;
use crate::ui::widgets::{self, FieldSpec};

/// `(key, 中文名, 说明)`
const KNOWN_FEATURES: &[(&str, &str, &str)] = &[
    (
        "hooks",
        "生命周期钩子",
        "在特定时机（回合开始/结束等）运行你自己的脚本。",
    ),
    (
        "apply_patch_freeform",
        "自由格式补丁",
        "让模型用更宽松的补丁格式改文件，成功率更高。",
    ),
    (
        "terminal_resize_reflow",
        "终端缩放重排",
        "终端窗口大小变化时重新排版已输出的内容。",
    ),
    ("memories", "长期记忆", "跨会话记住你的偏好和项目信息。"),
    (
        "statistics",
        "使用统计",
        "在界面上显示 token 用量等统计信息。",
    ),
    ("plugin_hooks", "插件钩子", "允许插件注册钩子。"),
    ("goals", "目标管理", "支持长任务的目标与预算跟踪。"),
    ("chronicle", "编年史", "记录会话时间线。"),
    (
        "js_repl",
        "JavaScript REPL",
        "允许模型运行 JavaScript 代码。",
    ),
    (
        "code_mode",
        "代码模式",
        "让模型通过写代码来调用工具，而不是直接调用。",
    ),
    (
        "context_management",
        "上下文管理",
        "自动裁剪上下文以节省 token。",
    ),
    (
        "token_budget",
        "Token 预算",
        "支持给任务设置 token 预算上限。",
    ),
    (
        "multi_agent_v2",
        "多代理 v2",
        "支持把任务拆分给子代理并行处理。",
    ),
    ("guardianv2", "Guardian v2", "更细粒度的权限审查机制。"),
    ("network_proxy", "网络代理", "为子进程提供受控的网络代理。"),
    ("sleep_tool", "睡眠工具", "允许模型主动等待一段时间。"),
    (
        "current_time_reminder",
        "当前时间提醒",
        "在上下文里注入当前时间。",
    ),
    ("rollout_budget", "会话用量预算", "限制单个会话的总用量。"),
    ("tool_registry", "工具注册表", "启用工具注册表配置。"),
];

const SESSION_PICKER_VIEWS: &[Choice] = &[
    Choice {
        value: "dense",
        label: "dense 紧凑（默认）",
        help: "会话列表更紧凑，一屏能看到更多。",
    },
    Choice {
        value: "comfortable",
        label: "comfortable 宽松",
        help: "每条会话占更多空间，更好读。",
    },
];

const STATUS_LINE_ITEMS: &[(&str, &str)] = &[
    ("model-with-reasoning", "模型+推理档位"),
    ("model", "模型名"),
    ("reasoning", "推理档位"),
    ("current-dir", "当前目录"),
    ("project-name", "项目名"),
    ("git-branch", "Git 分支"),
    ("pull-request-number", "PR 编号"),
    ("branch-changes", "分支改动量"),
    ("context-remaining", "剩余上下文"),
    ("context-used", "已用上下文"),
    ("context-window-size", "上下文总大小"),
    ("total-input-tokens", "输入 token"),
    ("total-output-tokens", "输出 token"),
    ("used-tokens", "已用 token"),
    ("five-hour-limit", "5 小时额度"),
    ("weekly-limit", "每周额度"),
    ("permissions", "权限档位"),
    ("approval-mode", "审批模式"),
    ("run-state", "运行状态"),
    ("hostname", "主机名"),
    ("codex-version", "版本号"),
    ("thread-name", "会话名"),
    ("thread-id", "会话 ID"),
    ("fast-mode", "Fast 模式"),
    ("raw-output", "原始输出模式"),
    ("estimated-thread-cost", "预估花费"),
];

pub fn show(app: &mut App, ui: &mut Ui, _ctx: &Context) {
    features_card(app, ui);
    ui.add_space(6.0);
    tui_card(app, ui);
    ui.add_space(6.0);
    behaviour_card(app, ui);
    ui.add_space(6.0);
    instructions_card(app, ui);
    ui.add_space(6.0);
    mcp_card(app, ui);
}

fn features_card(app: &mut App, ui: &mut Ui) {
    let mut keys: Vec<(String, String, String)> = Vec::new();
    for (key, label, help) in KNOWN_FEATURES {
        keys.push((
            (*key).to_string(),
            (*label).to_string(),
            (*help).to_string(),
        ));
    }
    if let Some(doc) = &app.doc {
        for key in doc.config.keys_at(&["features"]) {
            if !keys.iter().any(|(known, _, _)| *known == key) {
                keys.push((
                    key.clone(),
                    key.clone(),
                    "配置文件里已有、但本工具没有中文说明的开关。".to_string(),
                ));
            }
        }
    }

    widgets::section(
        ui,
        icons::FLASK,
        "实验功能开关",
        "对应 [features]。这些功能还在试验阶段：不确定就保持「默认」",
        |ui| {
            widgets::hint(
                ui,
                "「默认」= 不写进配置文件，由 Codex 自己决定；「开」= true；「关」= false。",
            );
            ui.add_space(8.0);
            for (key, label, help) in keys {
                let path = vec!["features", key.as_str()];
                settings::tri_row(app, ui, &path, &label, &help);
            }
            ui.add_space(4.0);
            settings::tri_row(
                app,
                ui,
                &["suppress_unstable_features_warning"],
                "关闭「实验功能」启动警告",
                "开了实验功能时 Codex 每次启动都会提示一句，嫌烦可以关掉。",
            );
        },
    );
}

fn tui_card(app: &mut App, ui: &mut Ui) {
    widgets::section(
        ui,
        icons::TERMINAL,
        "终端界面 (TUI)",
        "对应 [tui]。只影响终端里 Codex 的样子，不影响模型能力",
        |ui| {
            settings::str_field(
                app,
                ui,
                &["tui", "theme"],
                "配色主题",
                "终端界面的配色方案名字。留空使用默认主题。",
                "two-dark",
                true,
            );
            settings::choice_field(
                app,
                ui,
                &["tui", "session_picker_view"],
                SESSION_PICKER_VIEWS,
                "会话列表样式",
                "codex resume 里选历史会话时的排版密度。",
            );

            // status line: multi select chips backed by an array in the config
            let mut selected: Vec<String> = app
                .doc
                .as_ref()
                .and_then(|doc| doc.config.str_array_at(&["tui", "status_line"]))
                .unwrap_or_default();
            let options: Vec<(String, String, String)> = STATUS_LINE_ITEMS
                .iter()
                .map(|(value, label)| {
                    (
                        (*value).to_string(),
                        (*label).to_string(),
                        format!("状态栏显示：{label}（{value}）"),
                    )
                })
                .collect();
            let changed = widgets::field(
                ui,
                FieldSpec::new(
                    "底部状态栏显示什么",
                    "tui.status_line",
                    "按勾选顺序从左到右显示。不勾任何一项 = 使用 Codex 默认（模型、目录、会话名）。",
                ),
                |ui| widgets::chip_toggles(ui, &options, &mut selected),
            );
            if changed && let Some(doc) = &mut app.doc {
                if selected.is_empty() {
                    doc.config.remove_at(&["tui", "status_line"]);
                } else {
                    doc.config
                        .set_value_at(&["tui", "status_line"], value_str_array(&selected));
                }
            }

            ui.add_space(6.0);
            settings::tri_row(
                app,
                ui,
                &["tui", "animations"],
                "动画效果",
                "欢迎页、加载动画等。关掉更省资源。",
            );
            settings::tri_row(
                app,
                ui,
                &["tui", "whimsy"],
                "装饰特效",
                "输入框的小星星等装饰效果。",
            );
            settings::tri_row(
                app,
                ui,
                &["tui", "show_tooltips"],
                "启动提示",
                "第一次进入时显示的小贴士。",
            );
            settings::tri_row(
                app,
                ui,
                &["tui", "auto_recap"],
                "自动回顾",
                "终端失去焦点时自动生成对话回顾。",
            );
            settings::tri_row(
                app,
                ui,
                &["tui", "vim_mode_default"],
                "默认 Vim 模式",
                "启动就用 Vim 键位编辑输入框。",
            );
            settings::tri_row(
                app,
                ui,
                &["tui", "raw_output_mode"],
                "原始输出模式",
                "方便复制终端里的内容。",
            );
        },
    );
}

fn behaviour_card(app: &mut App, ui: &mut Ui) {
    widgets::section(
        ui,
        icons::GEAR,
        "子代理、记忆与提示",
        "对应 [agents] / [memories] / [notice]",
        |ui| {
            settings::int_field(
                app,
                ui,
                &["agents", "max_depth"],
                "子代理最大层级",
                "子代理还能再派生子代理的层数。1~2 比较稳妥，太深容易失控。",
            );
            settings::int_field(
                app,
                ui,
                &["agents", "max_threads"],
                "最多并行子代理数",
                "同时运行的子代理数量上限，太大很吃 token。",
            );
            settings::str_field(
                app,
                ui,
                &["memories", "extract_model"],
                "记忆提取用的模型",
                "开启 memories 功能时，用哪个模型来总结记忆。一般填一个便宜快的模型。",
                "例如 gpt-5-mini",
                true,
            );
            ui.add_space(4.0);
            settings::tri_row(
                app,
                ui,
                &["notice", "hide_full_access_warning"],
                "隐藏「完全访问」警告",
                "使用 danger-full-access 时启动不再弹警告。",
            );
            settings::tri_row(
                app,
                ui,
                &["notice", "fast_default_opt_out"],
                "不参与 Fast 模式默认开启",
                "阻止 Codex 把 Fast 模式设为默认。",
            );
        },
    );
}

fn instructions_card(app: &mut App, ui: &mut Ui) {
    widgets::section(
        ui,
        icons::BASICS,
        "给 Codex 的额外说明",
        "会附加到系统提示词里，用来固定你的个人偏好",
        |ui| {
            widgets::hint(
                ui,
                "例如：「回答一律用中文」「提交信息用中文」「不要改动 tests 目录」。写得越具体越好。",
            );
            ui.add_space(6.0);
            multiline_setting(app, ui, &["instructions"], "补充说明", "instructions", 4);
            multiline_setting(
                app,
                ui,
                &["developer_instructions"],
                "开发者说明",
                "developer_instructions",
                3,
            );
        },
    );
}

fn multiline_setting(
    app: &mut App,
    ui: &mut Ui,
    path: &[&str],
    label: &str,
    key: &str,
    rows: usize,
) {
    let mut value = settings::read_str(app, path);
    let changed = widgets::field(ui, FieldSpec::new(label, key, ""), |ui| {
        widgets::multiline_field(ui, &format!("adv-{key}"), &mut value, rows)
    });
    if changed && let Some(doc) = &mut app.doc {
        if value.trim().is_empty() {
            doc.config.remove_at(path);
        } else {
            doc.config
                .set_value_at(path, TomlValue::from(value.clone()));
        }
    }
}

fn mcp_card(app: &mut App, ui: &mut Ui) {
    let names: Vec<String> = app
        .doc
        .as_ref()
        .map(|doc| doc.config.keys_at(&["mcp_servers"]))
        .unwrap_or_default();
    widgets::section(
        ui,
        icons::MCP,
        "MCP 服务器",
        "对应 [mcp_servers.*]。MCP 让 Codex 能调用外部工具（浏览器、数据库、内部系统…）",
        |ui| {
            if names.is_empty() {
                widgets::hint(
                    ui,
                    "还没有配置任何 MCP 服务器。不需要的话可以完全忽略这一节。",
                );
            }
            for name in &names {
                let header = format!("{name}  ·  {}", mcp_summary(app, name));
                let name = name.clone();
                egui::CollapsingHeader::new(RichText::new(header).size(13.0).color(theme::TEXT))
                    .id_salt(format!("mcp-{name}"))
                    .show(ui, |ui| {
                        mcp_editor(app, ui, &name);
                    });
                ui.add_space(4.0);
            }

            ui.add_space(6.0);
            ui.horizontal(|ui| {
                let mut buffer = app.dialog_buffer.clone();
                if widgets::text_field_w(
                    ui,
                    "new-mcp-name",
                    &mut buffer,
                    "名字，例如 github",
                    260.0,
                ) {
                    app.dialog_buffer = buffer.clone();
                }
                let id = buffer.trim().to_string();
                let taken = names.contains(&id);
                ui.add_enabled_ui(!id.is_empty() && !taken, |ui| {
                    if widgets::primary_button(ui, &format!("{} 添加 MCP 服务器", icons::ADD))
                        .clicked()
                    {
                        if let Some(doc) = &mut app.doc {
                            doc.config.ensure_table_at(&["mcp_servers", &id]);
                            doc.config.set_value_at(
                                &["mcp_servers", &id, "command"],
                                TomlValue::from(""),
                            );
                            doc.config.set_item_at(
                                &["mcp_servers", &id, "args"],
                                Item::Value(TomlValue::Array(Array::new())),
                            );
                        }
                        app.dialog_buffer.clear();
                        app.toast_info(format!("已添加 MCP 服务器 {id}，填上启动命令"));
                    }
                });
                if taken {
                    widgets::note(ui, "这个名字已经有了。", theme::WARN);
                }
            });
        },
    );
}

fn mcp_path<'a>(name: &'a str, key: &'a str) -> Vec<&'a str> {
    vec!["mcp_servers", name, key]
}

fn mcp_summary(app: &App, name: &str) -> String {
    let Some(doc) = &app.doc else {
        return String::new();
    };
    let command = doc
        .config
        .str_at(&["mcp_servers", name, "command"])
        .unwrap_or_default();
    if command.is_empty() {
        let url = doc
            .config
            .str_at(&["mcp_servers", name, "url"])
            .unwrap_or_default();
        if url.is_empty() {
            "（还没填命令或地址）".to_string()
        } else {
            format!("HTTP {url}")
        }
    } else {
        let args = doc
            .config
            .str_array_at(&["mcp_servers", name, "args"])
            .unwrap_or_default();
        format!("{command} {}", args.join(" ")).trim().to_string()
    }
}

fn mcp_editor(app: &mut App, ui: &mut Ui, name: &str) {
    settings::str_field(
        app,
        ui,
        &mcp_path(name, "command"),
        "启动命令",
        "本地 MCP 服务器：填可执行文件，例如 npx、uvx、node。",
        "npx",
        true,
    );
    settings::lines_field(
        app,
        ui,
        &mcp_path(name, "args"),
        "命令参数",
        "一行一个参数，按顺序拼在命令后面。",
        3,
    );
    settings::str_field(
        app,
        ui,
        &mcp_path(name, "url"),
        "远程地址",
        "HTTP(S) 类型的 MCP 服务器填这里，填了就不需要启动命令。",
        "https://example.com/mcp",
        true,
    );

    // env vars
    let mut env: Vec<(String, String)> = app
        .doc
        .as_ref()
        .map(|doc| read_string_map(doc.config.item_at(&mcp_path(name, "env"))))
        .unwrap_or_default();
    if widgets::field(
        ui,
        FieldSpec::new(
            "环境变量",
            "env",
            "启动这个 MCP 服务器时要设置的环境变量，通常用来传 API Key。",
        ),
        |ui| widgets::kv_editor(ui, &format!("mcp-env-{name}"), &mut env, "变量名", "值"),
    ) && let Some(doc) = &mut app.doc
    {
        let entries: Vec<(String, String)> = env
            .iter()
            .filter(|(k, _)| !k.trim().is_empty())
            .cloned()
            .collect();
        if entries.is_empty() {
            doc.config.remove_at(&mcp_path(name, "env"));
        } else {
            let mut table = toml_edit::InlineTable::new();
            for (k, v) in entries {
                table.insert(k.trim(), TomlValue::from(v));
            }
            doc.config
                .set_value_at(&mcp_path(name, "env"), TomlValue::InlineTable(table));
        }
    }

    settings::tri_row(
        app,
        ui,
        &mcp_path(name, "enabled"),
        "启用",
        "关掉后 Codex 不会启动这个服务器。",
    );

    ui.horizontal(|ui| {
        if widgets::danger_button(ui, "删除这个 MCP 服务器").clicked() {
            if let Some(doc) = &mut app.doc {
                doc.config.remove_at(&["mcp_servers", name]);
            }
            app.toast_info(format!("已删除 MCP 服务器 {name}"));
        }
    });
}
