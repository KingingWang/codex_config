//! 「服务商」— list + editor for `[model_providers.*]`.

use egui::{Context, RichText, ScrollArea, Sense, Stroke, Ui};

use crate::app::{App, Dialog};
use crate::doc::catalog;
use crate::doc::providers as provider_ops;
use crate::doc::schema::WireApi;
use crate::doc::toml_ext::TomlPathExt;
use crate::editors::AuthMode;
use crate::net::Probe;
use crate::ui::icons;
use crate::ui::theme;
use crate::ui::widgets::{self, FieldSpec};

#[derive(Clone)]
struct Row {
    id: String,
    name: String,
    base_url: String,
    wire: WireApi,
    auth: String,
    active: bool,
    models: usize,
}

enum ProviderAction {
    None,
    SetActive(String),
    Rename(String),
    Delete(String),
    Probe(String, Probe),
}

pub fn show(app: &mut App, ui: &mut Ui, ctx: &Context) {
    let (rows, active_provider) = {
        let Some(doc) = &app.doc else { return };
        let active = doc.config.str_at(&["model_provider"]).unwrap_or_default();
        let rows: Vec<Row> = provider_ops::all(&doc.config)
            .iter()
            .map(|view| Row {
                id: view.id.clone(),
                name: if view.name.is_empty() { view.id.clone() } else { view.name.clone() },
                base_url: view.base_url.clone(),
                wire: view.wire(),
                auth: view.auth_summary(),
                active: view.id == active,
                models: doc
                    .catalog
                    .as_ref()
                    .and_then(|value| catalog::models(value))
                    .map(|list| {
                        list.iter()
                            .filter(|m| m.get("provider").and_then(serde_json::Value::as_str) == Some(view.id.as_str()))
                            .count()
                    })
                    .unwrap_or(0),
            })
            .collect();
        (rows, active)
    };

    if rows.is_empty() {
        widgets::card(ui, |ui| {
            ui.set_width(ui.available_width());
            widgets::empty_state(
                ui,
                icons::PROVIDERS,
                "还没有配置任何服务商",
                "服务商就是「模型从哪里来」。点下面的按钮，选一个模板（OpenAI 官方 / 中转站 / 本地 Ollama…），\n地址、协议、认证方式会自动填好，你只要把密钥换成自己的。",
            );
            ui.horizontal(|ui| {
                if widgets::primary_button(ui, &format!("{} 添加服务商（选模板）", icons::ADD)).clicked() {
                    app.dialog = Some(Dialog::NewProvider);
                }
            });
        });
        return;
    }

    let height = (ui.available_height() - 20.0).max(200.0);
    let mut action = ProviderAction::None;

    ui.horizontal(|ui| {
        if widgets::primary_button(ui, &format!("{} 添加服务商", icons::ADD)).clicked() {
            app.dialog = Some(Dialog::NewProvider);
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                RichText::new(format!("{} 个服务商", rows.len()))
                    .size(12.0)
                    .color(theme::TEXT_MUTED),
            );
        });
    });
    ui.add_space(8.0);

    let query = app.provider_query.clone();
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width(304.0);
            let mut local_query = query.clone();
            if widgets::search_field(ui, &mut local_query) {
                app.provider_query = local_query.clone();
            }
            ui.add_space(6.0);
            let filtered: Vec<Row> = rows
                .iter()
                .filter(|row| {
                    local_query.trim().is_empty()
                        || row.id.to_lowercase().contains(&local_query.to_lowercase())
                        || row.name.to_lowercase().contains(&local_query.to_lowercase())
                        || row.base_url.to_lowercase().contains(&local_query.to_lowercase())
                })
                .cloned()
                .collect();
            ScrollArea::vertical()
                .id_salt("provider-list")
                .max_height(height - 60.0)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    for row in &filtered {
                        let selected = app.editing_provider.as_deref() == Some(row.id.as_str());
                        let (response, _) = widgets::clickable_frame(ui, selected, |ui| {
                            ui.horizontal(|ui| {
                                ui.vertical(|ui| {
                                    ui.spacing_mut().item_spacing.y = 2.0;
                                    ui.label(
                                        RichText::new(row.name.clone())
                                            .size(13.0)
                                            .strong()
                                            .color(theme::TEXT),
                                    );
                                    ui.label(
                                        RichText::new(row.id.clone())
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
                                        }
                                    },
                                );
                            });
                            ui.add_space(2.0);
                            ui.horizontal_wrapped(|ui| {
                                ui.spacing_mut().item_spacing.x = 4.0;
                                widgets::badge(ui, row.wire.label(), theme::ACCENT_TEXT, theme::ACCENT_WEAK);
                                if row.models > 0 {
                                    widgets::badge(
                                        ui,
                                        &format!("{} 个模型", row.models),
                                        theme::TEXT_DIM,
                                        theme::CARD_ALT,
                                    );
                                }
                            });
                            ui.label(
                                RichText::new(if row.base_url.is_empty() {
                                    "（未填地址）".to_string()
                                } else {
                                    row.base_url.clone()
                                })
                                .monospace()
                                .size(10.5)
                                .color(theme::TEXT_MUTED),
                            );
                            ui.label(RichText::new(row.auth.clone()).size(11.0).color(theme::TEXT_DIM));
                        });
                        if response.clicked() {
                            app.select_provider(&row.id);
                        }
                        ui.add_space(5.0);
                    }
                });
        });

        ui.add_space(14.0);

        ui.vertical(|ui| {
            let width = ui.available_width();
            ui.set_width(width);
            ScrollArea::vertical()
                .id_salt("provider-editor")
                .max_height(height)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    let editing = app.editing_provider.clone();
                    match editing {
                        Some(id) if rows.iter().any(|row| row.id == id) => {
                            let row = rows.iter().find(|row| row.id == id).unwrap().clone();
                            editor(app, ui, &row, &active_provider, &mut action, ctx);
                        }
                        _ => widgets::empty_state(
                            ui,
                            icons::CARET_LEFT,
                            "选一个服务商开始编辑",
                            "左边点一下就能改地址、协议和密钥。\n改完记得点右上角「保存」，保存前会自动备份原文件。",
                        ),
                    }
                });
            ui.add_space(30.0);
        });
    });

    match action {
        ProviderAction::None => {}
        ProviderAction::SetActive(id) => {
            if let Some(doc) = &mut app.doc {
                doc.config
                    .set_value_at(&["model_provider"], toml_edit::Value::from(id.clone()));
            }
            app.toast_info(format!("已把 {id} 设为当前服务商"));
        }
        ProviderAction::Rename(id) => {
            app.dialog = Some(Dialog::RenameProvider { old: id, buffer: String::new() });
        }
        ProviderAction::Delete(id) => {
            app.dialog = Some(Dialog::DeleteProvider(id));
        }
        ProviderAction::Probe(id, probe) => {
            let model = app.probe_model_for(&id);
            app.start_probe(&id, probe, model, ctx);
        }
    }
}

fn editor(
    app: &mut App,
    ui: &mut Ui,
    row: &Row,
    active_provider: &str,
    action: &mut ProviderAction,
    ctx: &Context,
) {
    let _ = ctx;
    let Some(mut editor) = app.provider_editor.clone() else {
        widgets::hint(ui, "正在载入…");
        return;
    };
    let mut changed = false;

    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.label(RichText::new(row.name.clone()).size(19.0).strong().color(theme::TEXT));
            ui.label(
                RichText::new(format!("[model_providers.{}]", row.id))
                    .monospace()
                    .size(12.0)
                    .color(theme::TEXT_MUTED),
            );
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if widgets::danger_button(ui, &format!("{} 删除", icons::DELETE)).clicked() {
                *action = ProviderAction::Delete(row.id.clone());
            }
            if widgets::ghost_button(ui, "重命名").clicked() {
                *action = ProviderAction::Rename(row.id.clone());
            }
            if row.active {
                widgets::badge(ui, &format!("{} 当前服务商", icons::CHECK), theme::OK, theme::OK_WEAK);
            } else if widgets::primary_button(ui, "设为当前服务商").clicked() {
                *action = ProviderAction::SetActive(row.id.clone());
            }
        });
    });
    ui.add_space(4.0);
    ui.label(
        RichText::new(format!("当前服务商（config.toml 里的 model_provider）：{}", if active_provider.is_empty() { "（未设置）" } else { active_provider }))
            .size(11.5)
            .color(theme::TEXT_MUTED),
    );
    ui.add_space(10.0);

    let problems = editor.problems();
    if !problems.is_empty() {
        widgets::note(ui, &problems.join("\n"), theme::WARN);
        ui.add_space(8.0);
    }

    widgets::section(ui, icons::PROTOCOL, "通信协议", "决定 Codex 用什么格式跟这个服务商说话，选错了会直接报错", |ui| {
        if widgets::protocol_picker(ui, &mut editor.wire_api) {
            changed = true;
        }
        ui.add_space(6.0);
        widgets::hint(
            ui,
            &format!(
                "选了 {} 之后，Codex 会向 {} 发请求。",
                editor.wire_api.label(),
                editor.wire_api.endpoint().replace("{base_url}", "{base_url}")
            ),
        );
        if editor.wire_api == WireApi::Anthropic && editor.base_url.ends_with("/v1") {
            widgets::note(
                ui,
                "Anthropic 协议会自己补上 /v1/messages，所以 base_url 一般填到域名就行（https://api.anthropic.com），结尾不要带 /v1。",
                theme::WARN,
            );
        }
        if editor.wire_api == WireApi::Chat && !editor.chat_stream {
            widgets::note(
                ui,
                "Chat 协议 + 不开流式：回答会等全部生成完才一次性显示，看起来像卡住了。建议把下面的「流式输出」打开。",
                theme::WARN,
            );
        }
    });
    ui.add_space(6.0);

    widgets::section(ui, icons::ADDRESS, "服务地址", "服务商的接口地址，通常以 /v1 结尾（Anthropic 除外）", |ui| {
        if widgets::field(
            ui,
            FieldSpec::new("接口地址", "base_url", "模型服务的根地址。中转站一般长这样：https://xxx.com/v1。本地服务是 http://localhost:11434/v1。"),
            |ui| widgets::mono_field(ui, "p-baseurl", &mut editor.base_url, "https://api.openai.com/v1"),
        ) {
            changed = true;
        }
        if widgets::field(
            ui,
            FieldSpec::new("显示名称", "name", "只影响界面上显示的名字，随便写。"),
            |ui| widgets::text_field(ui, "p-name", &mut editor.name, "例如：公司代理"),
        ) {
            changed = true;
        }
        let query_open = editor.query_params.len();
        if query_open > 0 {
            ui.label(RichText::new("URL 查询参数 query_params").size(12.5).strong().color(theme::TEXT));
            widgets::hint(ui, "会拼在请求地址后面，Azure 需要 api-version。");
            if widgets::kv_editor(ui, "p-query", &mut editor.query_params, "参数名", "参数值") {
                changed = true;
            }
        }
    });
    ui.add_space(6.0);

    widgets::section(ui, icons::AUTH, "认证方式", "Codex 用什么凭证访问这个服务商", |ui| {
        for mode in AuthMode::ALL {
            let selected = editor.auth_mode == mode;
            let frame = egui::Frame::new()
                .fill(if selected { theme::ACCENT_WEAK } else { theme::INPUT_BG })
                .stroke(Stroke::new(1.0, if selected { theme::ACCENT } else { theme::BORDER }))
                .corner_radius(egui::CornerRadius::same(9))
                .inner_margin(egui::Margin::symmetric(12, 8));
            let response = frame
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(if selected { icons::CHECK } else { icons::DOT })
                                .size(13.0)
                                .color(if selected { theme::ACCENT } else { theme::TEXT_MUTED }),
                        );
                        ui.label(
                            RichText::new(mode.label())
                                .size(13.0)
                                .strong()
                                .color(if selected { theme::ACCENT_TEXT } else { theme::TEXT }),
                        );
                    });
                })
                .response
                .interact(Sense::click())
                .on_hover_text(mode.help());
            if response.clicked() {
                editor.auth_mode = mode;
                changed = true;
            }
            ui.add_space(3.0);
        }
        ui.add_space(6.0);
        match editor.auth_mode {
            AuthMode::EnvKey => {
                if widgets::field(
                    ui,
                    FieldSpec::new("环境变量名", "env_key", "Codex 启动时会读这个环境变量当 API Key。先在终端里 export 好，例如 export OPENAI_API_KEY=sk-xxx。"),
                    |ui| widgets::mono_field(ui, "p-envkey", &mut editor.env_key, "OPENAI_API_KEY"),
                ) {
                    changed = true;
                }
                if widgets::field(
                    ui,
                    FieldSpec::new("获取密钥的说明", "env_key_instructions", "可选。写给未来的自己看：这个 key 去哪里申请。"),
                    |ui| widgets::text_field(ui, "p-envinstr", &mut editor.env_key_instructions, "例如：去 platform.openai.com 申请"),
                ) {
                    changed = true;
                }
                let key = editor.env_key.clone();
                if !key.is_empty() {
                    match std::env::var(&key) {
                        Ok(value) if !value.is_empty() => {
                            widgets::note(ui, &format!("{} 环境变量 {key} 已经有值（{} 个字符），可以直接用。", icons::CHECK, value.len()), theme::OK);
                        }
                        _ => {
                            widgets::note(
                                ui,
                                &format!("环境变量 {key} 现在没有值。请在终端里执行：export {key}=你的密钥，然后重新启动 Codex。\n（这里显示的是本工具启动时读到的环境，如果你刚在别的窗口 export 过，重开一下本工具就能看到。）"),
                                theme::WARN,
                            );
                        }
                    }
                }
            }
            AuthMode::Bearer => {
                if widgets::field(
                    ui,
                    FieldSpec::new("API Token", "experimental_bearer_token", "直接写在配置文件里的密钥。方便，但任何能读到这个文件的人都能拿到它。"),
                    |ui| widgets::password_field(ui, "p-bearer", &mut editor.bearer, "sk-..."),
                ) {
                    changed = true;
                }
                widgets::note(ui, "这个值会以明文保存在 config.toml 里。分享配置或提交到 git 前记得删掉。", theme::WARN);
            }
            AuthMode::CodexLogin => {
                widgets::note(
                    ui,
                    "会写入 requires_openai_auth = true：Codex 首次启动时弹出登录流程，凭证保存在 ~/.codex/auth.json。\n公司内部代理常用这种方式（配合 experimental_bearer_token 由代理托管）。",
                    theme::ACCENT_TEXT,
                );
            }
            AuthMode::None => {
                widgets::hint(ui, "不会写入任何认证字段，适合本地服务（Ollama / LM Studio）。");
            }
        }
    });
    ui.add_space(6.0);

    test_card(app, ui, row, action);
    ui.add_space(6.0);

    let advanced_open = ui
        .horizontal(|ui| {
            let arrow = if editor.open_advanced { icons::CARET_DOWN } else { icons::CARET_RIGHT };
            widgets::ghost_button(ui, &format!("{arrow} 高级选项（流式、重试、请求头）")).clicked()
        })
        .inner;
    if advanced_open {
        editor.open_advanced = !editor.open_advanced;
        changed = true;
    }
    if editor.open_advanced {
        widgets::section(ui, icons::GEAR, "高级选项", "网络不稳定或者服务有特殊要求时才需要改", |ui| {
            ui.horizontal_wrapped(|ui| {
                if ui
                    .checkbox(&mut editor.chat_stream, "流式输出 (chat_stream)")
                    .on_hover_text("Chat 协议下逐字返回，强烈建议开启")
                    .changed()
                {
                    changed = true;
                }
                if ui
                    .checkbox(&mut editor.supports_websockets, "支持 WebSocket (supports_websockets)")
                    .on_hover_text("只有 Responses 协议的服务商才可能支持")
                    .changed()
                {
                    changed = true;
                }
            });
            ui.add_space(6.0);
            if widgets::field(
                ui,
                FieldSpec::new("请求重试次数", "request_max_retries", "单次请求失败后最多重试几次。网络差可以调大。"),
                |ui| widgets::opt_int_field(ui, "p-retries", &mut editor.request_max_retries),
            ) {
                changed = true;
            }
            if widgets::field(
                ui,
                FieldSpec::new("流断开重试次数", "stream_max_retries", "流式响应中途断开时最多重连几次。"),
                |ui| widgets::opt_int_field(ui, "p-stream-retries", &mut editor.stream_max_retries),
            ) {
                changed = true;
            }
            if widgets::field(
                ui,
                FieldSpec::new("流空闲超时 (毫秒)", "stream_idle_timeout_ms", "多久没收到数据就认为连接断了。"),
                |ui| widgets::opt_int_field(ui, "p-idle", &mut editor.stream_idle_timeout_ms),
            ) {
                changed = true;
            }
            ui.add_space(4.0);
            ui.label(RichText::new("自定义请求头 http_headers").size(12.5).strong().color(theme::TEXT));
            widgets::hint(ui, "每次请求都会带上。有些中转站要求特定的 User-Agent 或自定义头。");
            if widgets::kv_editor(ui, "p-headers", &mut editor.headers, "Header 名", "Header 值") {
                changed = true;
            }
            ui.add_space(6.0);
            ui.label(RichText::new("从环境变量取请求头 env_http_headers").size(12.5).strong().color(theme::TEXT));
            widgets::hint(ui, "值是环境变量名，运行时才取真值，适合放密钥类的 header。");
            if widgets::kv_editor(ui, "p-env-headers", &mut editor.env_http_headers, "Header 名", "环境变量名") {
                changed = true;
            }
            let extras = provider_ops::view(&app.doc.as_ref().unwrap().config, &row.id)
                .map(|view| view.extra_keys)
                .unwrap_or_default();
            if !extras.is_empty() {
                ui.add_space(6.0);
                widgets::note(
                    ui,
                    &format!(
                        "这个服务商还有本工具不直接编辑的字段：{}。它们会被原样保留。",
                        extras.join(", ")
                    ),
                    theme::TEXT_DIM,
                );
            }
        });
        ui.add_space(6.0);
    }

    if changed {
        app.provider_editor = Some(editor);
        app.commit_provider_editor();
    }
}

fn test_card(app: &mut App, ui: &mut Ui, row: &Row, action: &mut ProviderAction) {
    let busy = app.probe.as_ref().is_some_and(|probe| probe.provider_id == row.id);
    let model = app.probe_model_for(&row.id);
    widgets::section(ui, icons::TEST, "测试连接", "保存前先确认地址、协议、密钥三者是对的", |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.add_enabled_ui(!busy, |ui| {
                let label = match &model {
                    Some(model) => format!("{} 发一条测试请求（用 {model}）", icons::TEST),
                    None => format!("{} 发一条测试请求", icons::TEST),
                };
                if widgets::primary_button(ui, &label).clicked() {
                    *action = ProviderAction::Probe(row.id.clone(), Probe::Chat);
                }
                if widgets::ghost_button(ui, "\u{2261} 拉取模型列表").clicked() {
                    *action = ProviderAction::Probe(row.id.clone(), Probe::ListModels);
                }
            });
            if busy {
                ui.spinner();
                ui.label(RichText::new("正在请求…").size(12.0).color(theme::TEXT_DIM));
            }
        });
        if model.is_none() {
            widgets::hint(
                ui,
                "提示：模型目录里还没有绑定这个服务商的模型，测试时会用占位模型名，可能返回「模型不存在」。\n先加一个模型再测更准。",
            );
        }
        if let Some((id, outcome)) = app.last_outcome.clone() {
            if id == row.id {
                ui.add_space(6.0);
                let color = if outcome.ok { theme::OK } else { theme::DANGER };
                widgets::note(
                    ui,
                    &format!(
                        "{}（HTTP {}，{} ms）\n{}",
                        outcome.summary, outcome.status, outcome.elapsed_ms, outcome.detail
                    ),
                    color,
                );
            }
        }
    });
}
