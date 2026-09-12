//! 「基础设置」— the screen a beginner should never have to leave.

use egui::{Context, RichText, Ui};

use crate::app::{App, Dialog};
use crate::doc::catalog;
use crate::doc::schema::{
    APPROVAL_POLICIES, FILE_OPENERS, PERSONALITIES, REASONING_EFFORTS, REASONING_SUMMARIES,
    SANDBOX_MODES, VERBOSITIES, WEB_SEARCH_MODES,
};
use crate::doc::toml_ext::TomlPathExt;
use crate::doc::validate::{Issue, Severity};
use crate::page::Page;
use crate::ui::pages::settings;
use crate::ui::icons;
use crate::ui::theme;
use crate::ui::widgets;

pub fn show(app: &mut App, ui: &mut Ui, _ctx: &Context) {
    let Some(doc) = &app.doc else { return };
    let provider_count = doc.provider_ids().len();
    let model_count = doc.catalog.as_ref().map(catalog::model_count).unwrap_or(0);
    let active_model = doc.config.str_at(&["model"]).unwrap_or_default();
    let catalog_missing = doc.catalog_path.is_some() && doc.catalog.is_none();

    if provider_count == 0 || active_model.is_empty() || catalog_missing {
        quick_start(app, ui, provider_count, model_count, &active_model);
    }

    active_model_card(app, ui);
    safety_card(app, ui);
    files_card(app, ui);
    output_card(app, ui);
    health_card(app, ui);
}

fn quick_start(app: &mut App, ui: &mut Ui, provider_count: usize, model_count: usize, active_model: &str) {
    widgets::section(ui, icons::STEPS, "三步开始用", "第一次配置 Codex？按下面的顺序点就行", |ui| {
        let steps = [
            (
                "1",
                "添加模型服务商",
                if provider_count == 0 { "还没有服务商，Codex 不知道去哪里取模型" } else { "已添加" },
                provider_count > 0,
                Page::Providers,
            ),
            (
                "2",
                "添加模型",
                if model_count == 0 { "模型目录里还没有模型" } else { "已添加" },
                model_count > 0,
                Page::Models,
            ),
            (
                "3",
                "设为当前模型",
                if active_model.is_empty() { "还没有选择默认使用哪个模型" } else { "已选择" },
                !active_model.is_empty(),
                Page::Overview,
            ),
        ];
        for (number, title, status, done, page) in steps {
            ui.horizontal(|ui| {
                widgets::badge(
                    ui,
                    number,
                    if done { theme::OK } else { theme::ACCENT_TEXT },
                    if done { theme::OK_WEAK } else { theme::ACCENT_WEAK },
                );
                ui.label(RichText::new(title).size(13.5).strong().color(theme::TEXT));
                ui.label(RichText::new(status).size(12.0).color(if done { theme::OK } else { theme::TEXT_MUTED }));
                if !done {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if widgets::ghost_button(ui, "去完成 \u{2192}").clicked() {
                            app.page = page;
                        }
                    });
                }
            });
            ui.add_space(2.0);
        }
        ui.add_space(6.0);
        widgets::hint(ui, "全部完成后点右上角「保存」才会真正写入文件；保存前会自动备份，随时可以还原。");
    });
    ui.add_space(6.0);
}

fn active_model_card(app: &mut App, ui: &mut Ui) {
    let model_options: Vec<(String, String)> = app
        .doc
        .as_ref()
        .and_then(|doc| doc.catalog.as_ref())
        .map(|value| {
            catalog::models(value)
                .map(|list| {
                    list.iter()
                        .map(|model| {
                            let slug = catalog::slug_of(model);
                            let provider = model
                                .get("provider")
                                .and_then(serde_json::Value::as_str)
                                .unwrap_or("")
                                .to_string();
                            let label = if provider.is_empty() {
                                format!("{}  ({})", catalog::display_name(model), slug)
                            } else {
                                format!("{}  ({}) · {}", catalog::display_name(model), slug, provider)
                            };
                            (slug, label)
                        })
                        .collect()
                })
                .unwrap_or_default()
        })
        .unwrap_or_default();

    let provider_options = app.provider_options();
    let profile_options: Vec<(String, String)> = app
        .doc
        .as_ref()
        .map(|doc| {
            doc.profile_ids()
                .into_iter()
                .map(|name| (name.clone(), format!("配置档 {name}")))
                .collect()
        })
        .unwrap_or_default();

    widgets::section(ui, icons::ACTIVE_MODEL, "当前使用的模型", "决定 Codex 每次启动默认用哪个模型、走哪个服务商、想多深", |ui| {
        if model_options.is_empty() {
            widgets::note(
                ui,
                "模型目录还是空的。去「模型管理」页创建模型目录并添加模型，或者直接指定一个服务商支持的模型名。",
                theme::WARN,
            );
            ui.add_space(6.0);
            settings::str_field(
                app,
                ui,
                &["model"],
                "当前模型（手动填写）",
                "模型的唯一标识 slug，必须和服务商那边的名字一致。",
                "gpt-5.2-codex",
                true,
            );
        } else {
            settings::dynamic_choice_field(
                app,
                ui,
                &["model"],
                model_options,
                "当前模型",
                "Codex 启动时默认使用的模型。列表来自你的模型目录文件。",
                "（还没选）",
            );
        }

        settings::dynamic_choice_field(
            app,
            ui,
            &["model_provider"],
            provider_options,
            "当前服务商",
            "模型请求发到哪个服务商。留空表示用 Codex 内置的默认服务商。",
            "（使用内置默认）",
        );

        settings::choice_field(
            app,
            ui,
            &["model_reasoning_effort"],
            REASONING_EFFORTS,
            "推理强度",
            "模型在回答前「想多久」。越高越聪明但越慢越贵。日常用 medium，难题用 high 以上。",
        );
        settings::choice_field(
            app,
            ui,
            &["model_reasoning_summary"],
            REASONING_SUMMARIES,
            "思考摘要",
            "是否在界面上显示模型的思考过程摘要。",
        );
        settings::choice_field(
            app,
            ui,
            &["model_verbosity"],
            VERBOSITIES,
            "回答详细程度",
            "控制回答的长短，只对支持该参数的模型有效（例如 GPT-5 系列）。",
        );
        settings::choice_field(
            app,
            ui,
            &["personality"],
            PERSONALITIES,
            "语气人格",
            "注入到系统提示词里的性格设定，影响说话风格，不影响能力。",
        );
        settings::dynamic_choice_field(
            app,
            ui,
            &["profile"],
            profile_options,
            "启用的配置档",
            "配置档是一组预设（模型+服务商+权限）。选中后会覆盖上面的同名设置。",
            "（不使用配置档）",
        );
    });
    ui.add_space(6.0);
}

fn safety_card(app: &mut App, ui: &mut Ui) {
    widgets::section(ui, icons::SECURITY, "权限与安全", "Codex 能改哪些文件、执行命令前要不要问你", |ui| {
        settings::choice_field(
            app,
            ui,
            &["sandbox_mode"],
            SANDBOX_MODES,
            "沙箱模式",
            "限制 Codex 能碰到的文件范围。新手建议先用 read-only 或 workspace-write。",
        );
        settings::choice_field(
            app,
            ui,
            &["approval_policy"],
            APPROVAL_POLICIES,
            "审批策略",
            "执行命令 / 改文件之前要不要先征求你的同意。",
        );
        settings::bool_field(
            app,
            ui,
            &["sandbox_workspace_write", "network_access"],
            "允许联网",
            "workspace-write 模式下是否允许 Codex 联网（比如 git clone、npm install）。",
        );
        let mode = settings::read_str(app, &["sandbox_mode"]);
        if mode == "danger-full-access" {
            widgets::note(
                ui,
                "\u{26A0} 现在是「完全访问」：Codex 可以修改任何文件、执行任何命令，且不会二次确认。请确保你确实需要它。",
                theme::DANGER,
            );
        }
    });
    ui.add_space(6.0);
}

fn files_card(app: &mut App, ui: &mut Ui) {
    let Some(doc) = &app.doc else { return };
    let home = doc.codex_home.clone();
    let config_path = doc.config_path.clone();
    let catalog_path = doc.catalog_path.clone();
    let catalog_count = doc.catalog.as_ref().map(catalog::model_count).unwrap_or(0);
    let catalog_broken = catalog_path.is_some() && doc.catalog.is_none();

    widgets::section(ui, icons::FILES, "文件位置", "本工具编辑的就是这两个文件", |ui| {
        widgets::kv_row(ui, "CODEX_HOME", &home.display().to_string());
        ui.horizontal(|ui| {
            if widgets::ghost_button(ui, "切换文件夹").clicked() {
                app.dialog = Some(Dialog::ChangeHome { buffer: home.display().to_string() });
            }
            if widgets::ghost_button(ui, "在访达中打开").clicked() {
                let _ = open::that(&home);
            }
            if widgets::ghost_button(ui, "重新从磁盘载入").clicked() {
                app.load(home.clone());
            }
        });
        ui.add_space(8.0);
        widgets::kv_row(ui, "config.toml", &config_path.display().to_string());
        ui.horizontal(|ui| {
            if widgets::ghost_button(ui, "用默认程序打开").clicked() {
                app.open_in_editor(config_path.clone());
            }
        });
        ui.add_space(8.0);
        match &catalog_path {
            Some(path) => {
                widgets::kv_row(ui, "模型目录", &path.display().to_string());
                if catalog_broken {
                    widgets::note(ui, "这个文件不存在或者不是合法 JSON，模型页会显示为空。", theme::DANGER);
                } else {
                    widgets::kv_row(ui, "模型数量", &format!("{catalog_count} 个"));
                }
                ui.horizontal(|ui| {
                    if widgets::ghost_button(ui, "换一个文件").clicked() {
                        if let Some(picked) = rfd::FileDialog::new()
                            .add_filter("JSON", &["json"])
                            .set_directory(&home)
                            .pick_file()
                        {
                            app.set_catalog_path(picked);
                        }
                    }
                    if path.exists() && widgets::ghost_button(ui, "用默认程序打开").clicked() {
                        app.open_in_editor(path.clone());
                    }
                });
            }
            None => {
                widgets::kv_row(ui, "模型目录", "（未设置，使用 Codex 内置模型列表）");
                ui.horizontal(|ui| {
                    if widgets::primary_button(ui, "创建模型目录").clicked() {
                        app.dialog = Some(Dialog::CreateCatalog {
                            filename: "model-catalog.json".to_string(),
                        });
                    }
                    if widgets::ghost_button(ui, "选择已有文件…").clicked() {
                        if let Some(picked) = rfd::FileDialog::new()
                            .add_filter("JSON", &["json"])
                            .set_directory(&home)
                            .pick_file()
                        {
                            app.set_catalog_path(picked);
                        }
                    }
                });
            }
        }
    });
    ui.add_space(6.0);
}

fn output_card(app: &mut App, ui: &mut Ui) {
    widgets::section(ui, icons::OUTPUT, "输出与其它", "命令输出截断、文件打开方式、联网搜索等常用开关", |ui| {
        settings::int_field(
            app,
            ui,
            &["tool_output_token_limit"],
            "命令输出上限 (token)",
            "运行命令后最多把多少 token 的输出交给模型。太大容易撑爆上下文，太小会丢信息。常用 32768。",
        );
        settings::int_field(
            app,
            ui,
            &["project_doc_max_bytes"],
            "项目说明文件上限 (字节)",
            "读取 AGENTS.md / README 这类项目说明文件时的最大字节数。",
        );
        settings::choice_field(
            app,
            ui,
            &["file_opener"],
            FILE_OPENERS,
            "文件链接用什么打开",
            "终端里点击文件路径时用哪个编辑器打开。",
        );
        settings::choice_field(
            app,
            ui,
            &["web_search"],
            WEB_SEARCH_MODES,
            "联网搜索",
            "是否允许模型联网搜索资料。",
        );
        settings::bool_field(
            app,
            ui,
            &["hide_agent_reasoning"],
            "隐藏模型思考过程",
            "在终端里不显示模型的推理内容，界面更干净。",
        );
        settings::bool_field(
            app,
            ui,
            &["check_for_update_on_startup"],
            "启动时检查更新",
            "关掉可以省一点启动时间，也不会联网检查新版本。",
        );
    });
    ui.add_space(6.0);
}

fn health_card(app: &mut App, ui: &mut Ui) {
    let issues = app.issues();
    widgets::section(
        ui,
        icons::CHECK,
        "配置健康检查",
        "保存前自动检查常见问题，点一条可以直接跳到对应页面",
        |ui| {
            if issues.is_empty() {
                widgets::note(ui, &format!("{} 没有发现问题，配置看起来很健康。", icons::CHECK), theme::OK);
                return;
            }
            for issue in &issues {
                issue_row(app, ui, issue);
            }
        },
    );
}

fn issue_row(app: &mut App, ui: &mut Ui, issue: &Issue) {
    let (color, weak) = match issue.severity {
        Severity::Error => (theme::DANGER, theme::DANGER_WEAK),
        Severity::Warning => (theme::WARN, theme::WARN_WEAK),
        Severity::Info => (theme::ACCENT_TEXT, theme::ACCENT_WEAK),
    };
    let page = issue.page;
    let frame = egui::Frame::new()
        .fill(theme::INPUT_BG)
        .stroke(egui::Stroke::new(1.0, color.gamma_multiply(0.35)))
        .corner_radius(egui::CornerRadius::same(10))
        .inner_margin(egui::Margin::same(12));
    let response = frame
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_top(|ui| {
                widgets::badge(ui, issue.severity.label(), color, weak);
                ui.vertical(|ui| {
                    ui.label(RichText::new(issue.title.clone()).size(13.0).strong().color(theme::TEXT));
                    ui.label(
                        RichText::new(issue.detail.clone())
                            .size(12.0)
                            .color(theme::TEXT_DIM)
                            ,
                    );
                });
            });
        })
        .response
        .interact(egui::Sense::click())
        .on_hover_text(format!("跳到「{}」页处理", page.title()));
    if response.clicked() {
        app.page = page;
    }
    ui.add_space(4.0);
}
