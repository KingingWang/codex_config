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
use crate::ui::icons;
use crate::ui::pages::settings;
use crate::ui::theme;
use crate::ui::widgets;

pub fn show(app: &mut App, ui: &mut Ui, _ctx: &Context) {
    welcome(app, ui);
    ui.add_space(16.0);
    let profile = settings::read_str(app, &["profile"]);
    if !profile.is_empty() {
        widgets::note(
            ui,
            &format!("正在使用配置档「{profile}」，它会覆盖下面的同名设置。"),
            theme::WARN,
        );
        ui.horizontal(|ui| {
            if widgets::ghost_button(ui, "管理当前配置档").clicked() {
                app.page = Page::Profiles;
                app.select_profile(&profile);
            }
            if widgets::link_button(ui, "停用配置档，使用下面的设置", theme::ACCENT).clicked()
                && let Some(doc) = &mut app.doc
            {
                doc.config.remove_at(&["profile"]);
            }
        });
        ui.add_space(12.0);
    }
    ui.add_enabled_ui(profile.is_empty(), |ui| {
        if ui.available_width() >= 900.0 {
            ui.columns(2, |columns| {
                active_model_card(app, &mut columns[0]);
                safety_card(app, &mut columns[1]);
            });
        } else {
            active_model_card(app, ui);
            safety_card(app, ui);
        }
    });
    ui.add_space(8.0);
    ui.scope(|ui| {
        ui.set_width(ui.available_width());
        ui.separator();
        egui::CollapsingHeader::new("更多偏好设置").show(ui, |ui| {
            preferences(app, ui);
            output_card(app, ui);
        });
        let issues = app.issues();
        let attention = issues
            .iter()
            .filter(|issue| issue.severity != Severity::Info)
            .count();
        egui::CollapsingHeader::new(format!("配置检查 · {attention} 项需要留意"))
            .id_salt("overview-health")
            .show(ui, |ui| health_card(app, ui));
        egui::CollapsingHeader::new("文件与配置目录").show(ui, |ui| files_card(app, ui));
    });
    ui.add_space(8.0);
    widgets::hint(
        ui,
        "所有修改只在点击「保存配置」后写入当前环境的文件。保存前会自动备份。",
    );
}

fn welcome(app: &mut App, ui: &mut Ui) {
    let Some(doc) = &app.doc else { return };
    let has_custom = !doc.provider_ids().is_empty();
    let home = doc.codex_home.display().to_string();
    let model_count = doc.catalog.as_ref().map(catalog::model_count).unwrap_or(0);
    let provider_count = doc.provider_ids().len();
    let issues = app.issues();
    let repair = issues
        .iter()
        .find(|issue| issue.severity == Severity::Error);
    let (action, destination) = if let Some(issue) = repair {
        ("查看需要修复的问题", issue.page)
    } else if !has_custom {
        ("连接一个服务商", Page::Providers)
    } else {
        ("管理我的模型", Page::Models)
    };
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.y = 4.0;
        ui.label(RichText::new("WORKSPACE").size(11.0).color(theme::ACCENT));
        ui.label(
            RichText::new(if has_custom {
                "你的 AI 工作空间"
            } else {
                "从这里，开启你的工作空间"
            })
            .size(30.0)
            .strong()
            .color(theme::TEXT),
        );
        ui.label(
            RichText::new(if has_custom {
                "模型、权限与偏好。让工具顺应你的工作方式。"
            } else {
                "连接服务商，选择模型，再保存你的第一份配置。"
            })
            .size(13.0)
            .color(theme::TEXT_MUTED),
        );
    });
    ui.add_space(12.0);
    let mut frame = theme::card_frame()
        .inner_margin(egui::Margin::same(16))
        .begin(ui);
    {
        let ui = &mut frame.content_ui;
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            widgets::icon_tile(
                ui,
                if app.is_remote() {
                    icons::SERVER
                } else {
                    icons::TERMINAL
                },
                40.0,
                23.0,
                theme::ACCENT,
            );
            ui.add_space(6.0);
            let summary_width = (ui.available_width() - 172.0).max(160.0);
            ui.vertical(|ui| {
                ui.set_width(summary_width);
                ui.spacing_mut().item_spacing.y = 5.0;
                ui.add(
                    egui::Label::new(
                        RichText::new(app.target_label())
                            .size(17.0)
                            .strong()
                            .color(theme::TEXT),
                    )
                    .truncate(),
                )
                .on_hover_text(app.target_label());
                ui.add(
                    egui::Label::new(
                        RichText::new(&home)
                            .monospace()
                            .size(12.0)
                            .color(theme::TEXT_MUTED),
                    )
                    .truncate(),
                )
                .on_hover_text(&home);
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let response = if has_custom && repair.is_none() {
                    widgets::ghost_button(ui, action)
                } else {
                    widgets::primary_button(ui, action)
                };
                if response.clicked() {
                    app.page = destination;
                }
            });
        });
        ui.add_space(4.0);
        ui.scope(|ui| {
            ui.spacing_mut().interact_size.y = 22.0;
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.y = 4.0;
                let catalog_label = if model_count == 0 {
                    "内置模型目录".to_owned()
                } else {
                    format!("{model_count} 个自定义模型")
                };
                widgets::hint(
                    ui,
                    &format!("{catalog_label}    /    {provider_count} 个服务商"),
                );
                ui.add_space(12.0);
                let attention = issues
                    .iter()
                    .filter(|issue| issue.severity != Severity::Info)
                    .count();
                if attention > 0 {
                    widgets::badge(
                        ui,
                        &format!("{attention} 项配置需留意"),
                        theme::WARN,
                        theme::WARN_WEAK,
                    );
                } else {
                    widgets::hint(ui, "配置检查无异常");
                }
                if app.is_remote() {
                    widgets::hint(ui, "远程快照 · 非实时同步");
                }
            });
        });
    }
    frame.frame.stroke = egui::Stroke::new(1.0, theme::BORDER_STRONG.gamma_multiply(0.65));
    let response = frame.end(ui);
    ui.painter().rect_filled(
        egui::Rect::from_min_size(
            response.rect.left_top() + egui::vec2(0.0, 20.0),
            egui::vec2(3.0, response.rect.height() - 40.0),
        ),
        2,
        theme::ACCENT,
    );
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
                                catalog::display_name(model)
                            } else {
                                format!("{} · {}", catalog::display_name(model), provider)
                            };
                            (slug, label)
                        })
                        .collect()
                })
                .unwrap_or_default()
        })
        .unwrap_or_default();

    let provider_options = app.provider_options();
    widgets::section(
        ui,
        icons::ACTIVE_MODEL,
        "选择你的 AI",
        "日常只需要关心这三个选项",
        |ui| {
            ui.spacing_mut().item_spacing.y = 5.0;
            if model_options.is_empty() {
                widgets::note(
                    ui,
                    "正在使用内置模型列表。保持留空即可用默认模型，也可以填写服务商提供的模型名称。",
                    theme::TEXT_DIM,
                );
                ui.add_space(6.0);
                settings::str_field(
                    app,
                    ui,
                    &["model"],
                    "模型名称",
                    "可选。只在需要指定模型时填写。",
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
                    "启动时默认使用的模型。",
                    "（还没选）",
                );
            }

            settings::dynamic_choice_field(
                app,
                ui,
                &["model_provider"],
                provider_options,
                "当前服务商",
                "请求发送到这里；留空使用内置服务商。",
                "（使用内置默认）",
            );

            settings::choice_field(
                app,
                ui,
                &["model_reasoning_effort"],
                REASONING_EFFORTS,
                "思考深度",
                "越深入，等待时间可能越长。",
            );
        },
    );
}

fn preferences(app: &mut App, ui: &mut Ui) {
    let profile_options: Vec<(String, String)> = app
        .doc
        .as_ref()
        .map(|doc| {
            doc.profile_ids()
                .into_iter()
                .map(|name| (name.clone(), name))
                .collect()
        })
        .unwrap_or_default();
    widgets::section(
        ui,
        icons::OUTPUT,
        "回答偏好",
        "可选。不确定时保持默认即可",
        |ui| {
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
        },
    );
    ui.add_space(6.0);
}

fn safety_card(app: &mut App, ui: &mut Ui) {
    widgets::section(
        ui,
        icons::SECURITY,
        "让操作更放心",
        "选择 Codex 可以做什么",
        |ui| {
            ui.spacing_mut().item_spacing.y = 6.0;
            let mode = settings::read_str(app, &["sandbox_mode"]);
            let approval = settings::read_str(app, &["approval_policy"]);
            for (title, description, value) in [
                (
                    "先看看，不改文件",
                    "只读查看与分析，适合先熟悉 Codex。",
                    "read-only",
                ),
                (
                    "在项目里帮我工作",
                    "允许修改当前项目，按需申请额外权限。",
                    "workspace-write",
                ),
            ] {
                let selected = mode == value && approval == "on-request";
                let response = ui.add_sized(
                    [ui.available_width(), 42.0],
                    egui::Button::new(RichText::new(title).size(14.0).color(if selected {
                        theme::ACCENT_TEXT
                    } else {
                        theme::TEXT
                    }))
                    .selected(selected)
                    .fill(if selected {
                        theme::ACCENT_WEAK
                    } else {
                        theme::INPUT_BG
                    })
                    .stroke(egui::Stroke::new(
                        1.0,
                        if selected {
                            theme::ACCENT
                        } else {
                            theme::BORDER
                        },
                    )),
                );
                if selected {
                    ui.painter().text(
                        response.rect.right_center() - egui::vec2(18.0, 0.0),
                        egui::Align2::CENTER_CENTER,
                        icons::CHECK,
                        egui::FontId::proportional(18.0),
                        theme::ACCENT,
                    );
                }
                if response.clicked()
                    && let Some(doc) = &mut app.doc
                {
                    doc.config
                        .set_value_at(&["sandbox_mode"], toml_edit::Value::from(value));
                    doc.config
                        .set_value_at(&["approval_policy"], toml_edit::Value::from("on-request"));
                }
                ui.label(RichText::new(description).size(12.0).color(theme::TEXT_DIM));
                ui.add_space(8.0);
            }
            if mode != "read-only" && mode != "workspace-write" || approval != "on-request" {
                widgets::hint(ui, "当前使用默认或自定义权限；点击上方选项才会更改。");
            }
            egui::CollapsingHeader::new("自定义权限").show(ui, |ui| {
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
            });
            if mode == "danger-full-access" {
                widgets::note(
                    ui,
                    "当前为完全访问：不限制文件访问范围。是否需要确认由审批策略单独决定，请谨慎使用。",
                    theme::DANGER,
                );
            }
        },
    );
}

fn files_card(app: &mut App, ui: &mut Ui) {
    let Some(doc) = &app.doc else { return };
    let home = doc.codex_home.clone();
    let config_path = doc.config_path.clone();
    let catalog_path = doc.catalog_path.clone();
    let catalog_count = doc.catalog.as_ref().map(catalog::model_count).unwrap_or(0);
    let catalog_broken = catalog_path.is_some() && doc.catalog.is_none();

    widgets::section(
        ui,
        icons::FILES,
        "文件位置",
        "本工具编辑的就是这两个文件",
        |ui| {
            widgets::kv_row(ui, "CODEX_HOME", &home.display().to_string());
            ui.horizontal(|ui| {
                if widgets::ghost_button(ui, "切换文件夹").clicked() {
                    app.open_environment();
                }
                if !app.is_remote() && widgets::ghost_button(ui, "打开文件夹").clicked() {
                    app.open_in_editor(home.clone());
                }
                if app.is_remote() && widgets::ghost_button(ui, "浏览远程目录").clicked() {
                    app.open_remote_browser(crate::remote_browser::BrowserPurpose::Catalog);
                }
                if widgets::ghost_button(ui, "重新从磁盘载入").clicked() {
                    app.request_discard();
                }
            });
            ui.add_space(8.0);
            widgets::kv_row(ui, "config.toml", &config_path.display().to_string());
            ui.horizontal(|ui| {
                if widgets::ghost_button(
                    ui,
                    if app.is_remote() {
                        "查看源文件"
                    } else {
                        "用默认程序打开"
                    },
                )
                .clicked()
                {
                    app.open_in_editor(config_path.clone());
                }
            });
            ui.add_space(8.0);
            match &catalog_path {
                Some(path) => {
                    widgets::kv_row(ui, "模型目录", &path.display().to_string());
                    if catalog_broken {
                        widgets::note(
                            ui,
                            "这个文件不存在或者不是合法 JSON，模型页会显示为空。",
                            theme::DANGER,
                        );
                    } else {
                        widgets::kv_row(ui, "模型数量", &format!("{catalog_count} 个"));
                    }
                    ui.horizontal(|ui| {
                        if widgets::ghost_button(ui, "选择已有文件…").clicked() {
                            app.select_catalog_file();
                        }
                        if !app.is_remote()
                            && path.exists()
                            && widgets::ghost_button(ui, "用默认程序打开").clicked()
                        {
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
                            app.select_catalog_file();
                        }
                    });
                }
            }
        },
    );
    ui.add_space(6.0);
}

fn output_card(app: &mut App, ui: &mut Ui) {
    widgets::section(
        ui,
        icons::OUTPUT,
        "输出与其它",
        "命令输出截断、文件打开方式、联网搜索等常用开关",
        |ui| {
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
        },
    );
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
                widgets::note(
                    ui,
                    &format!("{} 没有发现问题，配置看起来很健康。", icons::CHECK),
                    theme::OK,
                );
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
                    ui.label(
                        RichText::new(issue.title.clone())
                            .size(13.0)
                            .strong()
                            .color(theme::TEXT),
                    );
                    ui.label(
                        RichText::new(issue.detail.clone())
                            .size(12.0)
                            .color(theme::TEXT_DIM),
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
