//! 「配置档」— named bundles of settings under `[profiles.*]`.

use egui::{Context, RichText, Ui};
use toml_edit::Value as TomlValue;

use crate::app::{App, Dialog};
use crate::doc::catalog;
use crate::doc::schema::{APPROVAL_POLICIES, PERSONALITIES, REASONING_EFFORTS, SANDBOX_MODES};
use crate::doc::toml_ext::TomlPathExt;
use crate::ui::icons;
use crate::ui::theme;
use crate::ui::widgets::{self, FieldSpec};

pub fn show(app: &mut App, ui: &mut Ui, _ctx: &Context) {
    let (names, active_profile, provider_options, model_options) = {
        let Some(doc) = &app.doc else { return };
        let model_options: Vec<(String, String)> = doc
            .catalog
            .as_ref()
            .and_then(|value| catalog::models(value))
            .map(|list| {
                list.iter()
                    .map(|model| {
                        let slug = catalog::slug_of(model);
                        (
                            slug.clone(),
                            format!("{} ({})", catalog::display_name(model), slug),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        (
            doc.profile_ids(),
            doc.config.str_at(&["profile"]).unwrap_or_default(),
            doc.selectable_provider_ids()
                .into_iter()
                .map(|id| (id.clone(), doc.provider_display(&id)))
                .collect::<Vec<_>>(),
            model_options,
        )
    };

    widgets::section(ui, icons::PROFILES, "配置档是什么？", "", |ui| {
        widgets::hint(
            ui,
            "一个配置档 = 一组「模型 + 服务商 + 权限」的预设。比如你可以建一个 work（公司代理 + 严格权限）\n和一个 play（本地 Ollama + 完全放开），需要时在下面一键切换，或者用 codex --profile work 启动。\n切换配置档只是把 profile = \"名字\" 写进 config.toml，不会动你的其它设置。",
        );
    });
    ui.add_space(8.0);

    // create new
    widgets::card(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            let mut buffer = app.dialog_buffer2.clone();
            if widgets::text_field_w(
                ui,
                "new-profile-name",
                &mut buffer,
                "新配置档的名字，例如 work",
                280.0,
            ) {
                app.dialog_buffer2 = buffer.clone();
            }
            let name = buffer.trim().to_string();
            let taken = names.contains(&name);
            ui.add_enabled_ui(!name.is_empty() && !taken, |ui| {
                if widgets::primary_button(ui, &format!("{} 新建配置档", icons::ADD)).clicked()
                {
                    if let Some(doc) = &mut app.doc {
                        doc.config.ensure_table_at(&["profiles", &name]);
                    }
                    app.dialog_buffer2.clear();
                    app.editing_profile = None;
                    app.select_profile(&name);
                    app.toast_info(format!("已创建配置档 {name}，在下面填内容"));
                }
            });
            if taken {
                widgets::note(ui, "这个名字已经有了。", theme::WARN);
            }
        });
    });
    ui.add_space(8.0);

    if names.is_empty() {
        widgets::card(ui, |ui| {
            ui.set_width(ui.available_width());
            widgets::empty_state(
                ui,
                icons::PROFILES,
                "还没有配置档",
                "不用配置档也完全可以正常使用 Codex —— 上面的「基础设置」就是全局默认值。\n只有当你需要在几套环境之间来回切换时，配置档才有意义。",
            );
        });
        return;
    }

    for name in names {
        let is_active = name == active_profile;
        let selected = app.editing_profile.as_deref() == Some(name.as_str());
        let summary = profile_summary(app, &name);
        let name_for_actions = name.clone();

        widgets::card(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!("{}  {name}", icons::PROFILES))
                        .size(15.0)
                        .strong()
                        .color(theme::TEXT),
                );
                if is_active {
                    widgets::badge(
                        ui,
                        &format!("{} 当前启用", icons::CHECK),
                        theme::OK,
                        theme::OK_WEAK,
                    );
                }
                ui.label(RichText::new(summary).size(12.0).color(theme::TEXT_DIM));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if widgets::danger_button(ui, "删除").clicked() {
                        app.dialog = Some(Dialog::DeleteProfile(name_for_actions.clone()));
                    }
                    if !is_active && widgets::primary_button(ui, "启用这个配置档").clicked()
                    {
                        if let Some(doc) = &mut app.doc {
                            doc.config.set_value_at(
                                &["profile"],
                                TomlValue::from(name_for_actions.clone()),
                            );
                        }
                        app.toast_info(format!("已切换到配置档 {name_for_actions}"));
                    }
                    if is_active && widgets::ghost_button(ui, "停止使用配置档").clicked() {
                        if let Some(doc) = &mut app.doc {
                            doc.config.remove_at(&["profile"]);
                        }
                        app.toast_info("已恢复使用全局默认设置");
                    }
                });
            });
            let toggle_label = if selected {
                format!("{} 收起", icons::CARET_DOWN)
            } else {
                format!("{} 编辑内容", icons::CARET_RIGHT)
            };
            if widgets::ghost_button(ui, &toggle_label).clicked() {
                if selected {
                    app.editing_profile = None;
                    app.profile_editor = None;
                } else {
                    app.select_profile(&name);
                }
            }
        });

        if selected {
            ui.add_space(4.0);
            profile_editor(app, ui, &provider_options, &model_options);
        }
        ui.add_space(8.0);
    }
}

fn profile_summary(app: &App, name: &str) -> String {
    let Some(doc) = &app.doc else {
        return String::new();
    };
    let at = |key: &str| {
        doc.config
            .str_at(&["profiles", name, key])
            .unwrap_or_default()
    };
    let mut parts = Vec::new();
    if !at("model").is_empty() {
        parts.push(format!("模型 {}", at("model")));
    }
    if !at("model_provider").is_empty() {
        parts.push(format!("服务商 {}", at("model_provider")));
    }
    if !at("model_reasoning_effort").is_empty() {
        parts.push(format!("推理 {}", at("model_reasoning_effort")));
    }
    if !at("sandbox_mode").is_empty() {
        parts.push(at("sandbox_mode"));
    }
    if parts.is_empty() {
        "（还是空的，全部沿用全局默认值）".to_string()
    } else {
        parts.join(" · ")
    }
}

fn profile_editor(
    app: &mut App,
    ui: &mut Ui,
    provider_options: &[(String, String)],
    model_options: &[(String, String)],
) {
    let Some(mut editor) = app.profile_editor.clone() else {
        return;
    };
    let mut changed = false;
    widgets::card(ui, |ui| {
        ui.set_width(ui.available_width());
        widgets::hint(ui, "留空的字段表示「沿用全局默认值」。");
        ui.add_space(6.0);
        let mut value = editor.model.clone();
        if widgets::field(
            ui,
            FieldSpec::new("模型", "model", "这个配置档用哪个模型。"),
            |ui| {
                widgets::string_dropdown(
                    ui,
                    "profile-model",
                    &mut value,
                    model_options,
                    "（沿用全局）",
                )
            },
        ) {
            editor.model = value;
            changed = true;
        }
        let mut value = editor.model_provider.clone();
        if widgets::field(
            ui,
            FieldSpec::new("服务商", "model_provider", "这个配置档走哪个服务商。"),
            |ui| {
                widgets::string_dropdown(
                    ui,
                    "profile-provider",
                    &mut value,
                    provider_options,
                    "（沿用全局）",
                )
            },
        ) {
            editor.model_provider = value;
            changed = true;
        }
        let mut value = editor.model_reasoning_effort.clone();
        if widgets::field(
            ui,
            FieldSpec::new("推理强度", "model_reasoning_effort", "模型想多久再回答。"),
            |ui| {
                widgets::choice_dropdown(ui, "profile-effort", &mut value, REASONING_EFFORTS, true)
            },
        ) {
            editor.model_reasoning_effort = value;
            changed = true;
        }
        let mut value = editor.sandbox_mode.clone();
        if widgets::field(
            ui,
            FieldSpec::new("沙箱模式", "sandbox_mode", "这个配置档的文件权限范围。"),
            |ui| widgets::choice_dropdown(ui, "profile-sandbox", &mut value, SANDBOX_MODES, true),
        ) {
            editor.sandbox_mode = value;
            changed = true;
        }
        let mut value = editor.approval_policy.clone();
        if widgets::field(
            ui,
            FieldSpec::new("审批策略", "approval_policy", "执行命令前是否询问你。"),
            |ui| {
                widgets::choice_dropdown(
                    ui,
                    "profile-approval",
                    &mut value,
                    APPROVAL_POLICIES,
                    true,
                )
            },
        ) {
            editor.approval_policy = value;
            changed = true;
        }
        let mut value = editor.personality.clone();
        if widgets::field(
            ui,
            FieldSpec::new("语气人格", "personality", "说话风格。"),
            |ui| {
                widgets::choice_dropdown(ui, "profile-personality", &mut value, PERSONALITIES, true)
            },
        ) {
            editor.personality = value;
            changed = true;
        }
    });
    if changed {
        app.profile_editor = Some(editor);
        app.commit_profile_editor();
    }
}
