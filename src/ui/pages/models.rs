//! 「模型管理」— list + editor for the model catalog JSON.

use egui::{Context, RichText, ScrollArea, Ui};
use serde_json::Value;

use crate::app::{App, Dialog};
use crate::doc::catalog;
use crate::doc::catalog::{ALL_REASONING_LEVELS, reasoning_description};
use crate::doc::schema::{
    APPLY_PATCH_TOOL_TYPES, INPUT_MODALITIES, MODEL_VISIBILITY, REASONING_SUMMARIES, SHELL_TYPES,
    TOOL_MODES, TRUNCATION_MODES, VERBOSITIES, WEB_SEARCH_TOOL_TYPES,
};
use crate::doc::toml_ext::TomlPathExt;
use crate::net::Probe;
use crate::ui::icons;
use crate::ui::theme;
use crate::ui::widgets::{self, FieldSpec};

#[derive(Clone)]
struct Row {
    index: usize,
    slug: String,
    display: String,
    provider: String,
    context_window: Option<i64>,
    levels: usize,
    active: bool,
    hidden: bool,
}

#[derive(PartialEq)]
enum ModelAction {
    None,
    SetActive(String),
    Duplicate(usize),
    RenameSlug(usize, String),
    Delete(usize, String),
}

pub fn show(app: &mut App, ui: &mut Ui, ctx: &Context) {
    let (has_catalog, catalog_path_text, rows) = {
        let Some(doc) = &app.doc else { return };
        let active_model = doc.config.str_at(&["model"]).unwrap_or_default();
        let rows: Vec<Row> = doc
            .catalog
            .as_ref()
            .and_then(|value| catalog::models(value))
            .map(|list| {
                list.iter()
                    .enumerate()
                    .map(|(index, model)| Row {
                        index,
                        slug: catalog::slug_of(model),
                        display: catalog::display_name(model),
                        provider: model
                            .get("provider")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_string(),
                        context_window: catalog::i64_at(model, &["context_window"]),
                        levels: catalog::reasoning_levels(model).len(),
                        active: catalog::slug_of(model) == active_model,
                        hidden: catalog::str_at(model, &["visibility"]).as_deref() == Some("none"),
                    })
                    .collect()
            })
            .unwrap_or_default();
        let path = doc
            .catalog_path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_default();
        (doc.catalog.is_some(), path, rows)
    };

    if !has_catalog {
        let existing = (!catalog_path_text.is_empty()).then(|| catalog_path_text.clone());
        no_catalog(app, ui, existing);
        return;
    }
    let provider_options = app.provider_options();
    let height = (ui.available_height() - 20.0).max(200.0);
    let mut action = ModelAction::None;

    // toolbar
    ui.horizontal(|ui| {
        if widgets::primary_button(ui, &format!("{} 添加模型", icons::ADD)).clicked() {
            app.dialog = Some(Dialog::NewModel {
                slug: String::new(),
                display_name: String::new(),
                provider: app
                    .doc
                    .as_ref()
                    .and_then(|d| d.config.str_at(&["model_provider"]))
                    .unwrap_or_default(),
                clone_from: rows.first().map(|r| r.slug.clone()).unwrap_or_default(),
            });
        }
        if let Some(provider) = app
            .doc
            .as_ref()
            .and_then(|d| d.config.str_at(&["model_provider"]))
            .filter(|p| !p.is_empty())
        {
            let busy = app.probe.is_some();
            ui.add_enabled_ui(!busy, |ui| {
                let label = if busy { "正在拉取…".to_string() } else { format!("{} 从服务商拉取模型列表", icons::DOWNLOAD) };
                if widgets::ghost_button(ui, &label).clicked() {
                    app.start_probe(&provider, Probe::ListModels, None, ctx);
                }
            });
            ui.label(RichText::new(format!("当前服务商：{provider}")).size(11.5).color(theme::TEXT_MUTED));
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                RichText::new(format!("{} 个模型", rows.len()))
                    .size(12.0)
                    .color(theme::TEXT_MUTED),
            );
        });
    });
    ui.label(
        RichText::new(format!("模型目录文件：{catalog_path_text}"))
            .size(11.5)
            .color(theme::TEXT_MUTED),
    );
    ui.add_space(8.0);

    let query = app.model_query.clone();
    ui.horizontal_top(|ui| {
        // ---- list column ------------------------------------------------
        ui.vertical(|ui| {
            ui.set_width(304.0);
            let mut local_query = query.clone();
            if widgets::search_field(ui, &mut local_query) {
                app.model_query = local_query.clone();
            }
            ui.add_space(6.0);
            let filtered: Vec<Row> = rows
                .iter()
                .filter(|row| {
                    local_query.trim().is_empty()
                        || row.slug.to_lowercase().contains(&local_query.to_lowercase())
                        || row.display.to_lowercase().contains(&local_query.to_lowercase())
                        || row.provider.to_lowercase().contains(&local_query.to_lowercase())
                })
                .cloned()
                .collect();
            ScrollArea::vertical()
                .id_salt("model-list")
                .max_height(height - 60.0)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    if filtered.is_empty() {
                        widgets::hint(ui, "没有匹配的模型。");
                    }
                    for row in &filtered {
                        let selected = app.editing_model == Some(row.index);
                        let slug = row.slug.clone();
                        let (response, set_active) = widgets::clickable_frame(ui, selected, |ui| {
                            let mut set_active = false;
                            ui.horizontal(|ui| {
                                ui.vertical(|ui| {
                                    ui.spacing_mut().item_spacing.y = 2.0;
                                    ui.label(
                                        RichText::new(row.display.clone())
                                            .size(13.0)
                                            .strong()
                                            .color(theme::TEXT),
                                    );
                                    ui.label(
                                        RichText::new(row.slug.clone())
                                            .monospace()
                                            .size(11.0)
                                            .color(theme::TEXT_MUTED),
                                    );
                                });
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::TOP),
                                    |ui| {
                                        if row.active {
                                            widgets::badge(ui, "使用中", theme::OK, theme::OK_WEAK);
                                        } else if ui
                                            .small_button("设为当前")
                                            .on_hover_text("把这个模型设成 config.toml 里的 model")
                                            .clicked()
                                        {
                                            set_active = true;
                                        }
                                    },
                                );
                            });
                            ui.add_space(2.0);
                            ui.horizontal_wrapped(|ui| {
                                ui.spacing_mut().item_spacing.x = 4.0;
                                if !row.provider.is_empty() {
                                    widgets::badge(ui, &row.provider, theme::ACCENT_TEXT, theme::ACCENT_WEAK);
                                } else {
                                    widgets::badge(ui, "未绑定服务商", theme::WARN, theme::WARN_WEAK);
                                }
                                if let Some(window) = row.context_window {
                                    widgets::badge(
                                        ui,
                                        &format!("{}k 上下文", window / 1000),
                                        theme::TEXT_DIM,
                                        theme::CARD_ALT,
                                    );
                                }
                                widgets::badge(
                                    ui,
                                    &format!("{} 档推理", row.levels),
                                    theme::TEXT_DIM,
                                    theme::CARD_ALT,
                                );
                                if row.hidden {
                                    widgets::badge(ui, "已禁用", theme::TEXT_MUTED, theme::CARD_ALT);
                                }
                            });
                            set_active
                        });
                        if response.clicked() {
                            app.select_model(row.index);
                        }
                        if set_active {
                            action = ModelAction::SetActive(slug);
                        }
                        ui.add_space(5.0);
                    }
                });
        });

        ui.add_space(14.0);

        // ---- editor column ----------------------------------------------
        ui.vertical(|ui| {
            let width = ui.available_width();
            ui.set_width(width);
            ScrollArea::vertical()
                .id_salt("model-editor")
                .max_height(height)
                .auto_shrink([false, true])
                .show(ui, |ui| match app.editing_model {
                    Some(index) if index < rows.len() => {
                        let row = rows[index].clone();
                        editor(app, ui, &row, &provider_options, &mut action);
                    }
                    _ => widgets::empty_state(
                        ui,
                        icons::CARET_LEFT,
                        "选一个模型开始编辑",
                        "左边列表点一下就能编辑；也可以点上面的「+ 添加模型」新建一个。\n每个字段下面都写了它是干什么用的，照着填就行。",
                    ),
                });
            ui.add_space(30.0);
        });
    });

    match action {
        ModelAction::None => {}
        ModelAction::SetActive(slug) => {
            if let Some(doc) = &mut app.doc {
                doc.config
                    .set_value_at(&["model"], toml_edit::Value::from(slug.clone()));
                let provider = doc
                    .catalog
                    .as_ref()
                    .and_then(|value| catalog::models(value))
                    .and_then(|list| list.iter().find(|m| catalog::slug_of(m) == slug))
                    .and_then(|m| m.get("provider"))
                    .and_then(Value::as_str)
                    .map(str::to_string);
                if let Some(provider) = provider.filter(|p| !p.is_empty()) {
                    doc.config
                        .set_value_at(&["model_provider"], toml_edit::Value::from(provider.clone()));
                    app.toast_info(format!("已把 {slug} 设为当前模型，服务商也切到了 {provider}"));
                } else {
                    app.toast_info(format!("已把 {slug} 设为当前模型"));
                }
            }
        }
        ModelAction::Duplicate(index) => duplicate_model(app, index),
        ModelAction::RenameSlug(index, slug) => {
            app.dialog = Some(Dialog::RenameModel { index, buffer: slug });
        }
        ModelAction::Delete(index, slug) => {
            app.dialog = Some(Dialog::DeleteModel { index, slug });
        }
    }
}

fn no_catalog(app: &mut App, ui: &mut Ui, path: Option<String>) {
    widgets::card(ui, |ui| {
        ui.set_width(ui.available_width());
        widgets::empty_state(
            ui,
            icons::MODELS,
            "还没有模型目录文件",
            "Codex 通过 model_catalog_json 指向的 JSON 文件来决定「有哪些模型可选」。\n你可以新建一个空白目录，也可以直接指向一个已有的 JSON 文件。",
        );
        if let Some(path) = path {
            widgets::note(
                ui,
                &format!("config.toml 里写的是 {path}，但这个文件读不出来（不存在或不是合法 JSON）。"),
                theme::WARN,
            );
            ui.add_space(8.0);
        }
        ui.horizontal(|ui| {
            if widgets::primary_button(ui, "新建模型目录文件").clicked() {
                app.dialog = Some(Dialog::CreateCatalog {
                    filename: "model-catalog.json".to_string(),
                });
            }
            if widgets::ghost_button(ui, "选择已有文件…").clicked() {
                let home = app
                    .doc
                    .as_ref()
                    .map(|d| d.codex_home.clone())
                    .unwrap_or_default();
                if let Some(picked) = rfd::FileDialog::new()
                    .add_filter("JSON", &["json"])
                    .set_directory(&home)
                    .pick_file()
                {
                    app.set_catalog_path(picked);
                }
            }
        });
        ui.add_space(10.0);
        widgets::hint(
            ui,
            "提示：模型目录不是必须的。不设置 model_catalog_json 时，Codex 会用内置的模型列表；\n但只要你想用自己的模型名（比如中转站的模型），就需要一个模型目录文件。",
        );
    });
}

fn editor(
    app: &mut App,
    ui: &mut Ui,
    row: &Row,
    provider_options: &[(String, String)],
    action: &mut ModelAction,
) {
    let mut editor = app.model_editor.clone();
    let mut changed = false;

    // header
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.label(
                RichText::new(if editor.display_name.is_empty() {
                    row.slug.clone()
                } else {
                    editor.display_name.clone()
                })
                .size(19.0)
                .strong()
                .color(theme::TEXT),
            );
            ui.label(
                RichText::new(row.slug.clone())
                    .monospace()
                    .size(12.0)
                    .color(theme::TEXT_MUTED),
            );
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if widgets::danger_button(ui, &format!("{} 删除模型", icons::DELETE)).clicked() {
                *action = ModelAction::Delete(row.index, row.slug.clone());
            }
            if widgets::ghost_button(ui, &format!("{} 复制为新模型", icons::DUPLICATE)).clicked() {
                *action = ModelAction::Duplicate(row.index);
            }
            if widgets::ghost_button(ui, &format!("{} 改 slug", icons::RENAME)).clicked() {
                *action = ModelAction::RenameSlug(row.index, editor.slug.clone());
            }
            if !row.active && widgets::primary_button(ui, "设为当前模型").clicked() {
                *action = ModelAction::SetActive(row.slug.clone());
            }
            if row.active {
                widgets::badge(ui, &format!("{} 当前使用中", icons::CHECK), theme::OK, theme::OK_WEAK);
            }
        });
    });
    ui.add_space(10.0);

    let problems = editor.problems();
    if !problems.is_empty() {
        widgets::note(ui, &problems.join("\n"), theme::WARN);
        ui.add_space(8.0);
    }

    widgets::section(ui, icons::BASICS, "基本信息", "模型叫什么、走哪个服务商、在列表里怎么显示", |ui| {
        if widgets::field(
            ui,
            FieldSpec::new("模型标识", "slug", "Codex 和服务商都用这个名字来识别模型，必须完全一致。改这里等于改模型的身份。"),
            |ui| widgets::mono_field(ui, "m-slug", &mut editor.slug, "gpt-5.2-codex"),
        ) {
            changed = true;
        }
        if widgets::field(
            ui,
            FieldSpec::new("显示名称", "display_name", "在 Codex 的 /model 列表里显示的名字，可以写中文，随便改。"),
            |ui| widgets::text_field(ui, "m-display", &mut editor.display_name, "例如：Claude Opus 5"),
        ) {
            changed = true;
        }
        if widgets::field(
            ui,
            FieldSpec::new("描述", "description", "一句话说明这个模型适合干什么，只显示给你自己看。"),
            |ui| widgets::text_field(ui, "m-desc", &mut editor.description, "例如：适合复杂重构"),
        ) {
            changed = true;
        }
        if widgets::field(
            ui,
            FieldSpec::new("使用哪个服务商", "provider", "选中这个模型时，Codex 会自动切到这里的请求地址。留空则沿用「基础设置」里的当前服务商。"),
            |ui| {
                let mut provider = editor.provider.clone();
                let mut options = vec![(String::new(), "（沿用当前服务商）".to_string())];
                options.extend(provider_options.iter().cloned());
                let row_changed = widgets::string_dropdown(ui, "m-provider", &mut provider, &options, "（沿用当前服务商）");
                editor.provider = provider;
                row_changed
            },
        ) {
            changed = true;
        }
        if !editor.provider.is_empty() && !provider_options.iter().any(|(value, _)| *value == editor.provider) {
            widgets::note(
                ui,
                &format!("服务商「{}」在 config.toml 里还不存在，请去「服务商」页添加它。", editor.provider),
                theme::DANGER,
            );
        }
        if widgets::field(
            ui,
            FieldSpec::new("是否显示", "visibility", "控制这个模型在 /model 列表里能不能被看到、能不能用。"),
            |ui| widgets::choice_dropdown(ui, "m-visibility", &mut editor.visibility, MODEL_VISIBILITY, false),
        ) {
            changed = true;
        }
        if widgets::field(
            ui,
            FieldSpec::new("排序优先级", "priority", "数字越小越靠前，只影响 /model 列表里的顺序。"),
            |ui| widgets::opt_int_field(ui, "m-priority", &mut editor.priority),
        ) {
            changed = true;
        }
    });
    ui.add_space(6.0);

    widgets::section(ui, icons::REASONING, "推理与思考", "模型能想多深，界面上显示多少思考内容", |ui| {
        if widgets::field(
            ui,
            FieldSpec::new("支持的推理档位", "supported_reasoning_levels", "勾上这个模型支持的档位，Codex 的 /model 里才会出现它们。不确定就多勾几个，模型不支持时会退回默认。"),
            |ui| {
                let options: Vec<(String, String, String)> = ALL_REASONING_LEVELS
                    .iter()
                    .map(|level| ((*level).to_string(), (*level).to_string(), reasoning_description(level)))
                    .collect();
                widgets::chip_toggles(ui, &options, &mut editor.supported_reasoning_levels)
            },
        ) {
            // keep the canonical order
            let order: Vec<&str> = ALL_REASONING_LEVELS.to_vec();
            editor.supported_reasoning_levels.sort_by_key(|level| {
                order.iter().position(|l| *l == level).unwrap_or(99)
            });
            changed = true;
        }
        if widgets::field(
            ui,
            FieldSpec::new("默认推理档位", "default_reasoning_level", "没有特别指定时用的档位，必须是上面勾选过的其中一个。"),
            |ui| {
                let options: Vec<(String, String)> = editor
                    .supported_reasoning_levels
                    .iter()
                    .map(|level| (level.clone(), format!("{level} · {}", reasoning_description(level))))
                    .collect();
                widgets::string_dropdown(ui, "m-default-level", &mut editor.default_reasoning_level, &options, "（未设置）")
            },
        ) {
            changed = true;
        }
        if widgets::field(
            ui,
            FieldSpec::new("思考摘要", "default_reasoning_summary", "默认是否显示模型的思考摘要。auto 由 Codex 决定。"),
            |ui| {
                widgets::choice_dropdown(
                    ui,
                    "m-summary",
                    &mut editor.default_reasoning_summary,
                    REASONING_SUMMARIES,
                    false,
                )
            },
        ) {
            changed = true;
        }
        if widgets::field(
            ui,
            FieldSpec::new("支持详细程度调节", "support_verbosity", "打开后才能用 low/medium/high 控制回答长短（GPT-5 系列支持）。"),
            |ui| ui.checkbox(&mut editor.support_verbosity, "支持 verbosity 参数").changed(),
        ) {
            changed = true;
        }
        if editor.support_verbosity
            && widgets::field(
                ui,
                FieldSpec::new("默认详细程度", "default_verbosity", "回答默认写多长。"),
                |ui| {
                    widgets::choice_dropdown(
                        ui,
                        "m-verbosity",
                        &mut editor.default_verbosity,
                        VERBOSITIES,
                        true,
                    )
                },
            )
        {
            changed = true;
        }
    });
    ui.add_space(6.0);

    widgets::section(ui, icons::CONTEXT, "上下文与截断", "一次能记住多少内容，命令输出太长时怎么截断", |ui| {
        if widgets::field(
            ui,
            FieldSpec::new("上下文窗口", "context_window", "模型一次能处理的最大 token 数（输入+输出）。填大了会报错，填小了会频繁压缩历史。常见：128000、200000、1000000。"),
            |ui| widgets::opt_int_field(ui, "m-ctx", &mut editor.context_window),
        ) {
            changed = true;
        }
        if widgets::field(
            ui,
            FieldSpec::new("上下文窗口上限", "max_context_window", "允许用户手动调到的最大值。一般和上面填一样。"),
            |ui| widgets::opt_int_field(ui, "m-maxctx", &mut editor.max_context_window),
        ) {
            changed = true;
        }
        if widgets::field(
            ui,
            FieldSpec::new("自动压缩阈值", "auto_compact_token_limit", "对话超过这个 token 数就自动压缩历史。留空则由 Codex 按上下文窗口的 90% 推算。"),
            |ui| widgets::opt_int_field(ui, "m-compact", &mut editor.auto_compact_token_limit),
        ) {
            changed = true;
        }
        if widgets::field(
            ui,
            FieldSpec::new("截断方式", "truncation_policy.mode", "命令输出太长时按什么单位截断。"),
            |ui| widgets::choice_dropdown(ui, "m-trunc-mode", &mut editor.truncation_mode, TRUNCATION_MODES, false),
        ) {
            changed = true;
        }
        if widgets::field(
            ui,
            FieldSpec::new("截断上限", "truncation_policy.limit", "超过这个数量就截断。常用 25600（token）或 32768。"),
            |ui| widgets::opt_int_field(ui, "m-trunc-limit", &mut editor.truncation_limit),
        ) {
            changed = true;
        }
    });
    ui.add_space(6.0);

    widgets::section(ui, icons::CAPABILITIES, "能力与工具", "这个模型能用哪些工具、支持哪些输入", |ui| {
        if widgets::field(
            ui,
            FieldSpec::new("支持的输入类型", "input_modalities", "勾上模型真正支持的输入。不勾 image 就没法发截图。"),
            |ui| {
                let options: Vec<(String, String, String)> = INPUT_MODALITIES
                    .iter()
                    .map(|c| (c.value.to_string(), c.label.to_string(), c.help.to_string()))
                    .collect();
                widgets::chip_toggles(ui, &options, &mut editor.input_modalities)
            },
        ) {
            changed = true;
        }
        if widgets::field(
            ui,
            FieldSpec::new("命令执行方式", "shell_type", "模型能不能运行终端命令。disabled 表示只对话、只改文件。"),
            |ui| widgets::choice_dropdown(ui, "m-shell", &mut editor.shell_type, SHELL_TYPES, false),
        ) {
            changed = true;
        }
        if widgets::field(
            ui,
            FieldSpec::new("改文件的方式", "apply_patch_tool_type", "freeform = 让模型直接输出补丁文本，兼容性最好。留空则用 Codex 默认方式。"),
            |ui| {
                widgets::choice_dropdown(
                    ui,
                    "m-applypatch",
                    &mut editor.apply_patch_tool_type,
                    APPLY_PATCH_TOOL_TYPES,
                    true,
                )
            },
        ) {
            changed = true;
        }
        if widgets::field(
            ui,
            FieldSpec::new("联网搜索返回", "web_search_tool_type", "搜索结果里能不能带图片。"),
            |ui| {
                widgets::choice_dropdown(
                    ui,
                    "m-websearch",
                    &mut editor.web_search_tool_type,
                    WEB_SEARCH_TOOL_TYPES,
                    false,
                )
            },
        ) {
            changed = true;
        }
        if widgets::field(
            ui,
            FieldSpec::new("工具调用模式", "tool_mode", "direct = 模型直接调用工具（最常见）。code_mode = 通过写代码调用。不确定就留空。"),
            |ui| widgets::choice_dropdown(ui, "m-toolmode", &mut editor.tool_mode, TOOL_MODES, true),
        ) {
            changed = true;
        }
        ui.horizontal_wrapped(|ui| {
            let mut flags = [
                ("supports_parallel_tool_calls", "支持并行调用工具", &mut editor.supports_parallel_tool_calls),
                ("supports_image_detail_original", "支持原图细节", &mut editor.supports_image_detail_original),
                ("use_responses_lite", "使用精简版 Responses", &mut editor.use_responses_lite),
                ("include_skills_usage_instructions", "注入技能使用说明", &mut editor.include_skills_usage_instructions),
                ("prefer_websockets", "优先用 WebSocket", &mut editor.prefer_websockets),
                ("supported_in_api", "支持 API 调用", &mut editor.supported_in_api),
            ];
            for (key, label, value) in flags.iter_mut() {
                if ui
                    .checkbox(*value, RichText::new(*label).size(12.5).color(theme::TEXT_DIM))
                    .on_hover_text(format!("对应 JSON 字段：{}", *key))
                    .changed()
                {
                    changed = true;
                }
            }
        });
    });
    ui.add_space(6.0);

    // advanced (collapsed by default)
    let open = ui
        .horizontal(|ui| {
            let text = if editor.open_advanced { icons::CARET_DOWN } else { icons::CARET_RIGHT };
            widgets::ghost_button(ui, &format!("{text} 高级字段（一般不用动）")).clicked()
        })
        .inner;
    if open {
        editor.open_advanced = !editor.open_advanced;
        changed = true;
    }
    if editor.open_advanced {
        widgets::section(ui, icons::GEAR, "高级字段", "这些字段通常保持默认就好，除非你明确知道要改", |ui| {
            if widgets::field(
                ui,
                FieldSpec::new("压缩兼容标识", "comp_hash", "内部用来判断压缩行为是否兼容的标记，一般不用改。"),
                |ui| widgets::mono_field(ui, "m-comphash", &mut editor.comp_hash, "留空即可"),
            ) {
                changed = true;
            }
            if widgets::field(
                ui,
                FieldSpec::new("代码审查用的模型", "auto_review_model_override", "自动代码审查时改用哪个模型。留空表示用当前模型。"),
                |ui| widgets::mono_field(ui, "m-review", &mut editor.auto_review_model_override, "留空即可"),
            ) {
                changed = true;
            }
            if widgets::field(
                ui,
                FieldSpec::new("首次可见时的提示语", "availability_nux.message", "用户第一次在列表里看到这个模型时显示的一段说明。留空则不显示。"),
                |ui| widgets::multiline_field(ui, "m-nux", &mut editor.nux_message, 3),
            ) {
                changed = true;
            }
            if widgets::field(
                ui,
                FieldSpec::new("自定义系统提示词", "base_instructions", "覆盖这个模型的默认系统提示词。绝大多数情况请留空，乱改会让 Codex 行为异常。"),
                |ui| widgets::multiline_field(ui, "m-instr", &mut editor.base_instructions, 4),
            ) {
                changed = true;
            }
        });
        ui.add_space(6.0);
    }

    let json_open = ui
        .horizontal(|ui| {
            let text = if editor.open_json { icons::CARET_DOWN } else { icons::CARET_RIGHT };
            widgets::ghost_button(ui, &format!("{text} 查看这个模型的原始 JSON")).clicked()
        })
        .inner;
    if json_open {
        editor.open_json = !editor.open_json;
        changed = true;
    }
    if editor.open_json {
        let raw = app
            .doc
            .as_ref()
            .and_then(|doc| doc.catalog.as_ref())
            .and_then(|value| catalog::models(value))
            .and_then(|list| list.get(row.index))
            .map(|model| serde_json::to_string_pretty(model).unwrap_or_default())
            .unwrap_or_default();
        widgets::card(ui, |ui| {
            ui.set_width(ui.available_width());
            egui::ScrollArea::horizontal().show(ui, |ui| {
                ui.label(RichText::new(raw).monospace().size(11.5).color(theme::TEXT_DIM));
            });
        });
    }

    if changed {
        app.model_editor = editor;
        app.commit_model_editor();
    }
}

fn duplicate_model(app: &mut App, index: usize) {
    let Some(doc) = &mut app.doc else { return };
    let Some(source) = doc
        .catalog
        .as_ref()
        .and_then(|value| catalog::models(value))
        .and_then(|list| list.get(index))
        .cloned()
    else {
        return;
    };
    let base = catalog::slug_of(&source);
    let mut new_slug = format!("{base}-copy");
    let existing = catalog::slugs(doc.catalog.as_ref().unwrap());
    let mut n = 2;
    while existing.contains(&new_slug) {
        new_slug = format!("{base}-copy{n}");
        n += 1;
    }
    let display = catalog::display_name(&source);
    let mut copy = source;
    catalog::set(&mut copy, &["slug"], Value::String(new_slug.clone()));
    catalog::set(
        &mut copy,
        &["display_name"],
        Value::String(format!("{display}（副本）")),
    );
    let new_index = {
        let list = catalog::models_mut(doc.catalog.as_mut().unwrap()).unwrap();
        list.push(copy);
        list.len() - 1
    };
    app.editing_model = None;
    app.select_model(new_index);
    app.toast_info(format!("已复制为 {new_slug}，改个名字和服务商就能用"));
}
