//! All modal dialogs: adding / renaming / deleting models, providers, profiles.

use egui::{Context, RichText, Sense, Stroke};
use serde_json::Value;

use crate::app::{App, Dialog};
use crate::doc::catalog;
use crate::doc::providers as provider_ops;
use crate::doc::schema::{PROVIDER_TEMPLATES, ProviderTemplate};
use crate::doc::toml_ext::{TomlPathExt, value_str};
use crate::editors::{ModelEditor, ProviderEditor};
use crate::page::Page;
use crate::ui::icons;
use crate::ui::theme;
use crate::ui::widgets;

enum Act {
    None,
    Close,
    CreateProviderFromTemplate(usize),
    CreateBlankProvider(String),
    CreateModel {
        slug: String,
        display_name: String,
        provider: String,
        clone_from: String,
    },
    RenameProvider {
        from: String,
        to: String,
    },
    DeleteProvider(String),
    DeleteModel(usize),
    DeleteProfile(String),
    RenameModel {
        index: usize,
        slug: String,
    },
    CreateCatalog(String),
    ImportModels {
        provider: String,
        ids: Vec<String>,
    },
    ChangeHome(String),
    RescanServers,
    RestartSelected,
    DiscardChanges,
    ExitUnsaved,
}

impl App {
    pub fn dialogs(&mut self, ctx: &Context) {
        let Some(dialog) = self.dialog.clone() else {
            return;
        };
        let action = match dialog {
            Dialog::NewProvider => self.dialog_new_provider(ctx),
            Dialog::NewModel { .. } => self.dialog_new_model(ctx),
            Dialog::RenameProvider { old, buffer } => self.dialog_rename_provider(ctx, old, buffer),
            Dialog::RenameModel { index, buffer } => self.dialog_rename_model(ctx, index, buffer),
            Dialog::DeleteProvider(id) => self.dialog_delete_provider(ctx, id),
            Dialog::DeleteModel { index, slug } => self.dialog_delete_model(ctx, index, slug),
            Dialog::DeleteProfile(name) => self.dialog_delete_profile(ctx, name),
            Dialog::CreateCatalog { filename } => self.dialog_create_catalog(ctx, filename),
            Dialog::ImportRemote { provider, models } => {
                self.dialog_import_remote(ctx, provider, models)
            }
            Dialog::ChangeHome { buffer } => self.dialog_change_home(ctx, buffer),
            Dialog::RestartServers => self.dialog_restart_servers(ctx),
            Dialog::DiscardChanges => self.dialog_unsaved(ctx, false),
            Dialog::ExitUnsaved => self.dialog_unsaved(ctx, true),
        };
        self.run(action);
    }

    fn run(&mut self, action: Act) {
        match action {
            Act::None => {}
            Act::Close => self.dialog = None,
            Act::CreateProviderFromTemplate(index) => {
                let template = PROVIDER_TEMPLATES[index].clone();
                self.create_provider_from_template(&template);
                self.dialog = None;
            }
            Act::CreateBlankProvider(id) => {
                self.create_blank_provider(&id);
                self.dialog = None;
            }
            Act::CreateModel {
                slug,
                display_name,
                provider,
                clone_from,
            } => {
                self.create_model(&slug, &display_name, &provider, &clone_from);
                self.dialog = None;
            }
            Act::RenameProvider { from, to } => {
                self.rename_provider(&from, &to);
                self.dialog = None;
            }
            Act::DeleteProvider(id) => {
                self.delete_provider(&id);
                self.dialog = None;
            }
            Act::DeleteModel(index) => {
                self.delete_model(index);
                self.dialog = None;
            }
            Act::DeleteProfile(name) => {
                self.delete_profile(&name);
                self.dialog = None;
            }
            Act::RenameModel { index, slug } => {
                self.rename_model(index, &slug);
                self.dialog = None;
            }
            Act::CreateCatalog(filename) => {
                if self.is_remote() {
                    self.start_remote_create_catalog(filename);
                } else {
                    self.create_catalog(&filename);
                    self.dialog = None;
                }
            }
            Act::ImportModels { provider, ids } => {
                self.import_remote_models(&provider, &ids);
                self.dialog = None;
            }
            Act::ChangeHome(path) => {
                self.dialog = None;
                self.load(crate::doc::expand_home(std::path::Path::new(&path)));
            }
            Act::RescanServers => self.rescan_servers(),
            Act::RestartSelected => self.run_restart_selected(),
            Act::DiscardChanges => {
                self.dialog = None;
                self.discard();
            }
            Act::ExitUnsaved => {
                self.dialog = None;
                self.close_confirmed = true;
            }
        }
    }

    // ---------------------------------------------------------------- dialogs

    fn dialog_unsaved(&mut self, ctx: &Context, exiting: bool) -> Act {
        let action = widgets::modal(
            ctx,
            "unsaved-changes",
            if exiting {
                "还有修改没有保存"
            } else {
                "要撤销这次修改吗？"
            },
            440.0,
            |ui| {
                widgets::note(
                    ui,
                    "未保存的设置和未应用的源文件草稿都会丢失。磁盘上已保存的配置不会受到影响。",
                    theme::WARN,
                );
                ui.add_space(12.0);
                let mut action = Act::None;
                ui.horizontal(|ui| {
                    if widgets::primary_button(ui, "继续编辑").clicked() {
                        action = Act::Close;
                    }
                    if widgets::danger_button(
                        ui,
                        if exiting {
                            "不保存并退出"
                        } else {
                            "确认撤销修改"
                        },
                    )
                    .clicked()
                    {
                        action = if exiting {
                            Act::ExitUnsaved
                        } else {
                            Act::DiscardChanges
                        };
                    }
                });
                action
            },
        )
        .unwrap_or(Act::None);
        if matches!(action, Act::ExitUnsaved) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        action
    }

    fn dialog_new_provider(&mut self, ctx: &Context) -> Act {
        widgets::modal(ctx, "new-provider", "添加模型服务商", 820.0, |ui| {
            let mut action = Act::None;
            ui.label(
                RichText::new("服务商 = 模型的来源（OpenAI 官方、公司内部代理、中转站、本地 Ollama…）。\n选一个模板，地址 / 协议 / 认证方式会自动填好，之后只需要改成你自己的。")
                    .size(12.5)
                    .color(theme::TEXT_DIM),
            );
            ui.add_space(10.0);
            // Card width must be computed *before* the row layout, otherwise each
            // card would take the full width and overflow the modal.
            let card_width = ((ui.available_width() - 14.0) / 2.0).clamp(220.0, 380.0);
            egui::ScrollArea::vertical()
                .max_height(420.0)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    let entries: Vec<usize> = (0..PROVIDER_TEMPLATES.len()).collect();
                    for chunk in entries.chunks(2) {
                        ui.horizontal_top(|ui| {
                            for index in chunk {
                                let template = &PROVIDER_TEMPLATES[*index];
                                let width = card_width;
                                let frame = egui::Frame::new()
                                    .fill(theme::INPUT_BG)
                                    .stroke(Stroke::new(1.0, theme::BORDER))
                                    .corner_radius(egui::CornerRadius::same(10))
                                    .inner_margin(egui::Margin::same(12));
                                let response = frame
                                    .show(ui, |ui| {
                                        ui.vertical(|ui| {
                                            widgets::wrap(ui);
                                            ui.set_width(width - 24.0);
                                            ui.label(
                                                RichText::new(template.title)
                                                    .size(13.5)
                                                    .strong()
                                                    .color(theme::TEXT),
                                            );
                                            ui.label(
                                                RichText::new(template.summary)
                                                    .size(11.5)
                                                    .color(theme::TEXT_DIM),
                                            );
                                            ui.add_space(4.0);
                                            ui.label(
                                                RichText::new(template.wire_api.label())
                                                    .size(11.0)
                                                    .color(theme::ACCENT_TEXT),
                                            );
                                            ui.label(
                                                RichText::new(template.base_url)
                                                    .monospace()
                                                    .size(10.5)
                                                    .color(theme::TEXT_MUTED),
                                            );
                                        });
                                    })
.response
                                    .interact(Sense::click());
                                if response.clicked() {
                                    action = Act::CreateProviderFromTemplate(*index);
                                }
                            }
                        });
                    }

                    ui.add_space(12.0);
                    ui.separator();
                    ui.add_space(8.0);
                    ui.label(RichText::new("或者：从空白开始").size(13.5).strong().color(theme::TEXT));
                    ui.label(
                        RichText::new("自己填所有字段。适合已经有现成配置片段、只想搬过来的情况。")
                            .size(11.5)
                            .color(theme::TEXT_DIM),
                    );
                    ui.horizontal(|ui| {
                        let mut id = self.dialog_buffer.clone();
                        let changed = widgets::text_field_w(ui, "blank-provider-id", &mut id, "my-provider", 260.0);
                        if changed {
                            self.dialog_buffer = id.clone();
                        }
                        if widgets::primary_button(ui, "创建空白服务商").clicked() {
                            let wanted = if id.trim().is_empty() { "my-provider" } else { id.trim() };
                            action = Act::CreateBlankProvider(wanted.to_string());
                        }
                    });

                    ui.add_space(8.0);
                    if widgets::ghost_button(ui, "取消").clicked() {
                        action = Act::Close;
                    }
                });
            action
        })
        .unwrap_or(Act::None)
    }

    fn dialog_new_model(&mut self, ctx: &Context) -> Act {
        let Dialog::NewModel {
            slug,
            display_name,
            provider,
            clone_from,
        } = self.dialog.clone().unwrap()
        else {
            return Act::None;
        };
        let provider_options = self.provider_options();
        let clone_options = self.clone_options();

        widgets::modal(ctx, "new-model", "添加模型", 620.0, |ui| {
            let mut action = Act::None;
            let mut slug = slug.clone();
            let mut display_name = display_name.clone();
            let mut provider = provider.clone();
            let mut clone_from = clone_from.clone();

            ui.label(
                RichText::new("模型写进「模型目录 JSON」文件里。slug 是 Codex 用来找模型的唯一标识，必须和服务商那边的模型名完全一致。")
                    .size(12.5)
                    .color(theme::TEXT_DIM),
            );
            ui.add_space(10.0);

            widgets::field(
                ui,
                widgets::FieldSpec::new(
                    "从哪个模型复制？",
                    "",
                    "复制一个现成的模型最省事：它的上下文窗口、推理档位、工具设置都会一起带过来，你只要改 slug 和服务商。",
                ),
                |ui| {
                    widgets::string_dropdown(
                        ui,
                        "clone-source",
                        &mut clone_from,
                        &clone_options,
                        "从空白模板新建",
                    )
                },
            );

            widgets::field(
                ui,
                widgets::FieldSpec::new("模型标识 slug", "slug", "服务商接口里认的模型名，例如 gpt-5.2-codex、claude-opus-5、deepseek-chat。"),
                |ui| widgets::mono_field(ui, "new-model-slug", &mut slug, "gpt-5.2-codex"),
            );
            widgets::field(
                ui,
                widgets::FieldSpec::new("显示名称", "display_name", "在 Codex 的 /model 列表里看到的名字，可以写中文。"),
                |ui| widgets::text_field(ui, "new-model-name", &mut display_name, "留空则和 slug 一样"),
            );
            widgets::field(
                ui,
                widgets::FieldSpec::new("使用哪个服务商", "provider", "这个模型走哪个服务商发请求。留空表示沿用当前选中的服务商。"),
                |ui| {
                    widgets::string_dropdown(ui, "new-model-provider", &mut provider, &provider_options, "（沿用当前服务商）")
                },
            );

            let slug_trimmed = slug.trim().to_string();
            if slug_trimmed.is_empty() {
                widgets::note(ui, "请先填写模型标识 slug。", theme::WARN);
            } else if self.model_slug_exists(&slug_trimmed) {
                widgets::note(ui, &format!("已经有一个叫 {slug_trimmed} 的模型了，换一个名字。"), theme::DANGER);
            }

            ui.add_space(6.0);
            ui.horizontal(|ui| {
                let valid = !slug_trimmed.is_empty() && !self.model_slug_exists(&slug_trimmed);
                ui.add_enabled_ui(valid, |ui| {
                    if widgets::primary_button(ui, "添加模型").clicked() {
                        action = Act::CreateModel {
                            slug: slug_trimmed.clone(),
                            display_name: display_name.clone(),
                            provider: provider.clone(),
                            clone_from: clone_from.clone(),
                        };
                    }
                });
                if widgets::ghost_button(ui, "取消").clicked() {
                    action = Act::Close;
                }
            });

            if let Some(dialog) = &mut self.dialog {
                *dialog = Dialog::NewModel { slug, display_name, provider, clone_from };
            }
            action
        })
        .unwrap_or(Act::None)
    }

    fn dialog_rename_provider(&mut self, ctx: &Context, old: String, buffer: String) -> Act {
        widgets::modal(ctx, "rename-provider", "重命名服务商", 520.0, |ui| {
            let mut action = Act::None;
            let mut next = buffer.clone();
            ui.label(
                RichText::new(format!("把 [{old}] 改成什么名字？"))
                    .size(13.0)
                    .color(theme::TEXT_DIM),
            );
            ui.add_space(6.0);
            widgets::mono_field(ui, "rename-provider-id", &mut next, "my-provider");
            ui.label(
                RichText::new(
                    "改名后，所有引用它的地方（当前服务商、模型绑定、配置档）都会自动跟着改。",
                )
                .size(11.5)
                .color(theme::TEXT_MUTED),
            );
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let trimmed = next.trim().to_string();
                let ok = !trimmed.is_empty()
                    && (trimmed == old
                        || !provider_ops::exists(&self.doc.as_ref().unwrap().config, &trimmed));
                ui.add_enabled_ui(ok, |ui| {
                    if widgets::primary_button(ui, "重命名").clicked() {
                        action = Act::RenameProvider {
                            from: old.clone(),
                            to: trimmed,
                        };
                    }
                });
                if widgets::ghost_button(ui, "取消").clicked() {
                    action = Act::Close;
                }
            });
            self.dialog = Some(Dialog::RenameProvider { old, buffer: next });
            action
        })
        .unwrap_or(Act::None)
    }

    fn dialog_rename_model(&mut self, ctx: &Context, index: usize, buffer: String) -> Act {
        widgets::modal(ctx, "rename-model", "修改模型标识 slug", 520.0, |ui| {
            let mut action = Act::None;
            let mut next = buffer.clone();
            ui.label(
                RichText::new("slug 必须和服务商那边的模型名完全一致，否则请求会失败。")
                    .size(12.5)
                    .color(theme::TEXT_DIM),
            );
            ui.add_space(6.0);
            widgets::mono_field(ui, "rename-model-slug", &mut next, "gpt-5.2-codex");
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let trimmed = next.trim().to_string();
                let taken = self.model_slug_taken_elsewhere(&trimmed, index);
                ui.add_enabled_ui(!trimmed.is_empty() && !taken, |ui| {
                    if widgets::primary_button(ui, "保存新 slug").clicked() {
                        action = Act::RenameModel {
                            index,
                            slug: trimmed,
                        };
                    }
                });
                if widgets::ghost_button(ui, "取消").clicked() {
                    action = Act::Close;
                }
            });
            if taken_note(self, &next, index) {
                widgets::note(ui, "已经有别的模型用了这个 slug。", theme::DANGER);
            }
            self.dialog = Some(Dialog::RenameModel {
                index,
                buffer: next,
            });
            action
        })
        .unwrap_or(Act::None)
    }

    fn dialog_delete_provider(&mut self, ctx: &Context, id: String) -> Act {
        let affected = self.models_using_provider(&id);
        let is_active = self
            .doc
            .as_ref()
            .and_then(|doc| doc.config.str_at(&["model_provider"]))
            .is_some_and(|active| active == id);
        widgets::modal(ctx, "delete-provider", "删除服务商", 560.0, |ui| {
            let mut action = Act::None;
            ui.label(
                RichText::new(format!("确定要删除服务商 [{id}] 吗？"))
                    .size(14.0)
                    .color(theme::TEXT),
            );
            ui.label(
                RichText::new(
                    "只会从配置里移除，不会动你的账号或密钥。保存之前都可以点「放弃修改」反悔。",
                )
                .size(12.0)
                .color(theme::TEXT_DIM),
            );
            ui.add_space(8.0);
            if is_active {
                widgets::note(
                    ui,
                    "这是「基础设置」里当前正在用的服务商，删除后 Codex 会连不上，记得换一个。",
                    theme::WARN,
                );
            }
            if !affected.is_empty() {
                widgets::note(
                    ui,
                    &format!(
                        "有 {} 个模型绑定了它：{}",
                        affected.len(),
                        affected.join("、")
                    ),
                    theme::WARN,
                );
            }
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if widgets::danger_button(ui, "删除").clicked() {
                    action = Act::DeleteProvider(id.clone());
                }
                if widgets::ghost_button(ui, "取消").clicked() {
                    action = Act::Close;
                }
            });
            action
        })
        .unwrap_or(Act::None)
    }

    fn dialog_delete_model(&mut self, ctx: &Context, index: usize, slug: String) -> Act {
        let is_active = self
            .doc
            .as_ref()
            .and_then(|doc| doc.config.str_at(&["model"]))
            .is_some_and(|active| active == slug);
        widgets::modal(ctx, "delete-model", "删除模型", 540.0, |ui| {
            let mut action = Act::None;
            ui.label(RichText::new(format!("确定要删除模型「{slug}」吗？")).size(14.0).color(theme::TEXT));
            ui.label(RichText::new("只从这个模型目录 JSON 文件里删掉一条记录。").size(12.0).color(theme::TEXT_DIM));
            ui.add_space(8.0);
            if is_active {
                widgets::note(ui, "这正是「基础设置」里当前使用的模型。删除后请换一个模型，否则 Codex 会找不到它。", theme::WARN);
            }
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if widgets::danger_button(ui, "删除").clicked() {
                    action = Act::DeleteModel(index);
                }
                if widgets::ghost_button(ui, "取消").clicked() {
                    action = Act::Close;
                }
            });
            action
        })
        .unwrap_or(Act::None)
    }

    fn dialog_delete_profile(&mut self, ctx: &Context, name: String) -> Act {
        widgets::modal(ctx, "delete-profile", "删除配置档", 520.0, |ui| {
            let mut action = Act::None;
            ui.label(
                RichText::new(format!("确定要删除配置档「{name}」吗？"))
                    .size(14.0)
                    .color(theme::TEXT),
            );
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if widgets::danger_button(ui, "删除").clicked() {
                    action = Act::DeleteProfile(name.clone());
                }
                if widgets::ghost_button(ui, "取消").clicked() {
                    action = Act::Close;
                }
            });
            action
        })
        .unwrap_or(Act::None)
    }

    fn dialog_create_catalog(&mut self, ctx: &Context, filename: String) -> Act {
        widgets::modal(ctx, "create-catalog", "创建模型目录文件", 620.0, |ui| {
            let mut action = Act::None;
            let mut filename = filename.clone();
            ui.label(
                RichText::new("模型目录是一个 JSON 文件，里面列出 Codex 可以选择的所有模型。\n它会被创建在 CODEX_HOME 文件夹里，并在 config.toml 里用 model_catalog_json 指向它。")
                    .size(12.5)
                    .color(theme::TEXT_DIM),
            );
            ui.add_space(10.0);
            widgets::field(
                ui,
                widgets::FieldSpec::new("文件名", "model_catalog_json", "相对路径会以 CODEX_HOME 为基准，也可以填绝对路径。"),
                |ui| widgets::mono_field(ui, "catalog-filename", &mut filename, "model-catalog.json"),
            );
            let target = match &self.doc {
                Some(doc) => doc.resolve_against_home(filename.trim()),
                None => return action,
            };
            widgets::kv_row(ui, "将创建在", &target.display().to_string());
            let exists = !self.is_remote() && target.exists();
            if exists {
                widgets::note(ui, "这个文件已经存在了。换一个名字，或者用「选择已有文件」直接指向它。", theme::WARN);
            }
            if self.is_remote() {
                widgets::hint(ui, "创建前先检查远端路径；保存时再次核验，不覆盖已有文件。");
                if let Some(error) = &self.remote.error {
                    widgets::note(ui, error, theme::DANGER);
                }
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.add_enabled_ui(!exists && !filename.trim().is_empty(), |ui| {
                    if widgets::primary_button(ui, "创建并启用").clicked() {
                        action = Act::CreateCatalog(filename.trim().to_string());
                    }
                });
                if widgets::ghost_button(ui, "选择已有文件…").clicked() {
                    self.select_catalog_file();
                    action = Act::Close;
                }
                if widgets::ghost_button(ui, "取消").clicked() {
                    action = Act::Close;
                }
            });
            self.dialog = Some(Dialog::CreateCatalog { filename });
            action
        })
        .unwrap_or(Act::None)
    }

    fn dialog_import_remote(
        &mut self,
        ctx: &Context,
        provider: String,
        models: Vec<String>,
    ) -> Act {
        let existing = self.model_slugs();
        widgets::modal(
            ctx,
            "import-remote",
            "导入服务商的模型",
            620.0,
            |ui| {
                let mut action = Act::None;
                ui.label(
                    RichText::new(format!(
                        "服务商 [{provider}] 返回了 {} 个模型。勾选想要的，一键加进模型目录。",
                        models.len()
                    ))
                    .size(12.5)
                    .color(theme::TEXT_DIM),
                );
                ui.add_space(8.0);
                if self.dialog_checkbox.len() != models.len() {
                    self.dialog_checkbox = vec![false; models.len()];
                }
                egui::ScrollArea::vertical()
                    .max_height(340.0)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        for (index, model) in models.iter().enumerate() {
                            let already = existing.contains(model);
                            ui.horizontal(|ui| {
                                ui.add_enabled_ui(!already, |ui| {
                                    if ui.checkbox(&mut self.dialog_checkbox[index], "").changed() {
                                    }
                                });
                                ui.label(
                                    RichText::new(model)
                                        .monospace()
                                        .size(12.5)
                                        .color(theme::TEXT),
                                );
                                if already {
                                    widgets::badge(
                                        ui,
                                        "已在目录里",
                                        theme::TEXT_MUTED,
                                        theme::CARD_ALT,
                                    );
                                }
                            });
                        }
                    });
                let picked: Vec<String> = models
                    .iter()
                    .zip(self.dialog_checkbox.iter())
                    .filter(|(_, checked)| **checked)
                    .map(|(model, _)| model.clone())
                    .collect();
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!("已勾选 {} 个", picked.len()))
                            .size(12.0)
                            .color(theme::TEXT_DIM),
                    );
                    ui.add_enabled_ui(!picked.is_empty(), |ui| {
                        if widgets::primary_button(ui, "导入所选模型").clicked() {
                            action = Act::ImportModels {
                                provider: provider.clone(),
                                ids: picked.clone(),
                            };
                        }
                    });
                    if widgets::ghost_button(ui, "取消").clicked() {
                        action = Act::Close;
                    }
                });
                action
            },
        )
        .unwrap_or(Act::None)
    }

    fn dialog_change_home(&mut self, ctx: &Context, buffer: String) -> Act {
        widgets::modal(ctx, "change-home", "切换 CODEX_HOME", 620.0, |ui| {
            let mut action = Act::None;
            let mut buffer = buffer.clone();
            ui.label(
                RichText::new("CODEX_HOME 是 Codex 存放配置的文件夹，默认是 ~/.codex。切换后本工具会重新读取那个文件夹里的 config.toml。")
                    .size(12.5)
                    .color(theme::TEXT_DIM),
            );
            ui.add_space(8.0);
            let dirty = self.has_unsaved_changes();
            if dirty {
                widgets::note(ui, "当前还有未保存的修改。请先取消并保存，或明确选择放弃修改后载入。", theme::WARN);
            }
            widgets::mono_field(ui, "home-path", &mut buffer, "~/.codex");
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if widgets::ghost_button(ui, "浏览…").clicked()
                    && let Some(picked) = rfd::FileDialog::new().pick_folder() {
                        buffer = picked.display().to_string();
                    }
                let trimmed = buffer.trim().to_string();
                ui.add_enabled_ui(!trimmed.is_empty(), |ui| {
                    let response = if dirty { widgets::danger_button(ui, "放弃修改并载入") }
                        else { widgets::primary_button(ui, "载入") };
                    if response.clicked() {
                        action = Act::ChangeHome(trimmed.clone());
                    }
                });
                if widgets::ghost_button(ui, "取消").clicked() {
                    action = Act::Close;
                }
            });
            self.dialog = Some(Dialog::ChangeHome { buffer });
            action
        })
        .unwrap_or(Act::None)
    }

    fn dialog_restart_servers(&mut self, ctx: &Context) -> Act {
        widgets::modal(ctx, "restart-servers", "重启 App Server（让配置生效）", 720.0, |ui| {
            let mut action = Act::None;
            if self.has_unsaved_changes() {
                widgets::note(ui, "你还有未保存的修改。重启只会读取磁盘上的旧配置，请先保存。", theme::WARN);
            }
            ui.label(
                RichText::new("Codex 的后台服务（App Server）在启动时只读取一次配置。改完配置后，需要重启对应的服务，新的模型 / 服务商 / 参数才会生效。\n下面列出了当前正在运行的 App Server，勾选要重启的即可——重启其实就是结束旧进程，宿主应用（ChatGPT、Zed 等）会自动拉起一个读取新配置的新进程。")
                    .size(12.5)
                    .color(theme::TEXT_DIM),
            );
            ui.add_space(10.0);

            if self.restart_scan.is_empty() {
                widgets::empty_state(
                    ui,
                    icons::SERVER,
                    "没有扫描到正在运行的 App Server",
                    "可能它们还没启动，或者已经退出。启动 ChatGPT 桌面应用 / Zed 后再点「重新扫描」。",
                );
            } else {
                if self.restart_selected.len() != self.restart_scan.len() {
                    self.restart_selected = self.restart_scan.iter().map(|i| !i.is_our_host).collect();
                }
                egui::ScrollArea::vertical()
                    .id_salt("restart-scan")
                    .max_height(320.0)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        for (index, inst) in self.restart_scan.iter().enumerate() {
                            let protected = inst.is_our_host;
                            let frame = egui::Frame::new()
                                .fill(if protected { theme::CARD_ALT } else { theme::INPUT_BG })
                                .stroke(Stroke::new(1.0, theme::BORDER))
                                .corner_radius(egui::CornerRadius::same(10))
                                .inner_margin(egui::Margin::same(12));
                            frame.show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.horizontal(|ui| {
                                    ui.add_enabled_ui(!protected, |ui| {
                                        ui.checkbox(&mut self.restart_selected[index], "");
                                    });
                                    ui.label(RichText::new(icons::SERVER).size(18.0).color(theme::ACCENT));
                                    ui.add_space(2.0);
                                    ui.vertical(|ui| {
                                        widgets::wrap(ui);
                                        ui.horizontal(|ui| {
                                            ui.label(RichText::new(inst.host.label()).size(13.5).strong().color(theme::TEXT));
                                            widgets::badge(ui, &format!("PID {}", inst.pid), theme::TEXT_MUTED, theme::CARD_ALT);
                                            if let Some(sp) = inst.shell_pid {
                                                widgets::badge(ui, &format!("壳 {sp}", ), theme::TEXT_MUTED, theme::CARD_ALT);
                                            }
                                            if protected {
                                                widgets::badge(ui, &format!("{} 当前会话，已保护", icons::SHIELD), theme::OK, theme::OK_WEAK);
                                            } else if !inst.host.auto_respawns() {
                                                widgets::badge(ui, "需手动重启", theme::WARN, theme::WARN_WEAK);
                                            }
                                        });
                                        ui.label(RichText::new(&inst.cmd_summary).monospace().size(10.5).color(theme::TEXT_MUTED));
                                        if protected {
                                            ui.label(
                                                RichText::new("这是托管当前这段对话 / 任务的服务。为避免中断你正在做的事，它不会被重启——等任务结束后重开 App 即可读取新配置。")
                                                    .size(11.0)
                                                    .color(theme::TEXT_DIM),
                                            );
                                        }
                                    });
                                });
                            });
                            ui.add_space(6.0);
                        }
                    });
            }

            if !self.restart_results.is_empty() {
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(6.0);
                ui.label(RichText::new("上次操作结果").size(12.5).strong().color(theme::TEXT));
                for r in &self.restart_results {
                    let color = if r.ok { theme::OK } else { theme::WARN };
                    widgets::note(ui, &format!("[{}] {}", r.host, r.message), color);
                }
            }

            ui.add_space(10.0);
            let any_selected = self
                .restart_selected
                .iter()
                .zip(self.restart_scan.iter())
                .any(|(c, i)| *c && !i.is_our_host);
            ui.horizontal(|ui| {
                ui.add_enabled_ui(any_selected, |ui| {
                    if widgets::primary_button(ui, &format!("{} 重启所选服务", icons::RESTART)).clicked() {
                        action = Act::RestartSelected;
                    }
                });
                if widgets::ghost_button(ui, &format!("{} 重新扫描", icons::SEARCH)).clicked() {
                    action = Act::RescanServers;
                }
                if widgets::ghost_button(ui, "关闭").clicked() {
                    action = Act::Close;
                }
            });
            action
        })
        .unwrap_or(Act::None)
    }

    // ---------------------------------------------------------------- actions

    fn create_provider_from_template(&mut self, template: &ProviderTemplate) {
        let Some(doc) = &mut self.doc else { return };
        let id = provider_ops::suggest_id(&doc.config, template.key);
        let editor = ProviderEditor::from_template(&id, template);
        editor.write_to(&mut doc.config);
        self.provider_editor = Some(editor);
        self.editing_provider = Some(id.clone());
        self.page = Page::Providers;
        self.toast_info(format!(
            "已添加服务商 {id}（{}）。把地址和密钥改成你自己的，再点右上角保存。",
            template.title
        ));
    }

    fn create_blank_provider(&mut self, wanted: &str) {
        let Some(doc) = &mut self.doc else { return };
        let id = provider_ops::suggest_id(&doc.config, wanted);
        provider_ops::create(&mut doc.config, &id);
        let view = provider_ops::view(&doc.config, &id).unwrap_or_else(|| {
            crate::doc::providers::ProviderView {
                id: id.clone(),
                ..Default::default()
            }
        });
        let mut editor = ProviderEditor::from_view(&view);
        editor.id = id.clone();
        editor.name = id.clone();
        editor.write_to(&mut doc.config);
        self.provider_editor = Some(editor);
        self.editing_provider = Some(id.clone());
        self.page = Page::Providers;
        self.toast_info(format!("已创建空白服务商 {id}"));
    }

    fn create_model(&mut self, slug: &str, display_name: &str, provider: &str, clone_from: &str) {
        let Some(doc) = &mut self.doc else { return };
        if doc.catalog.is_none() {
            self.toast_error("还没有模型目录文件，先创建一个");
            return;
        }
        let name = if display_name.trim().is_empty() {
            slug
        } else {
            display_name.trim()
        };
        let new_model = if clone_from.is_empty() {
            catalog::new_model_template(slug, name, Some(provider).filter(|p| !p.is_empty()))
        } else {
            let mut cloned = doc
                .catalog
                .as_ref()
                .and_then(|value| catalog::models(value))
                .and_then(|list| list.iter().find(|m| catalog::slug_of(m) == clone_from))
                .cloned()
                .unwrap_or_else(|| catalog::new_model_template(slug, name, None));
            catalog::set(&mut cloned, &["slug"], Value::String(slug.to_string()));
            catalog::set(
                &mut cloned,
                &["display_name"],
                Value::String(name.to_string()),
            );
            if provider.is_empty() {
                catalog::remove(&mut cloned, &["provider"]);
            } else {
                catalog::set(
                    &mut cloned,
                    &["provider"],
                    Value::String(provider.to_string()),
                );
            }
            cloned
        };

        let new_index = {
            let list = catalog::models_mut(doc.catalog.as_mut().unwrap()).unwrap();
            list.push(new_model);
            list.len() - 1
        };
        self.editing_model = None;
        self.select_model(new_index);
        self.page = Page::Models;
        self.toast_info(format!(
            "已添加模型 {slug}，检查一下上下文窗口和推理档位再保存"
        ));
    }

    fn rename_provider(&mut self, from: &str, to: &str) {
        if from == to {
            self.dialog = None;
            return;
        }
        let Some(doc) = &mut self.doc else { return };
        if !provider_ops::rename(&mut doc.config, from, to) {
            self.toast_error(format!("重命名失败：{to} 已经存在"));
            return;
        }
        let mut touched = 0usize;
        if doc.config.str_at(&["model_provider"]).as_deref() == Some(from) {
            doc.config.set_value_at(&["model_provider"], value_str(to));
            touched += 1;
        }
        if doc.config.str_at(&["oss_provider"]).as_deref() == Some(from) {
            doc.config.set_value_at(&["oss_provider"], value_str(to));
            touched += 1;
        }
        for profile in doc.profile_ids() {
            let path = ["profiles", profile.as_str(), "model_provider"];
            if doc.config.str_at(&path).as_deref() == Some(from) {
                doc.config.set_value_at(&path, value_str(to));
                touched += 1;
            }
        }
        if let Some(catalog) = doc.catalog.as_mut()
            && let Some(list) = catalog::models_mut(catalog)
        {
            for model in list.iter_mut() {
                if model.get("provider").and_then(Value::as_str) == Some(from) {
                    catalog::set(model, &["provider"], Value::String(to.to_string()));
                    touched += 1;
                }
            }
        }
        if self.editing_provider.as_deref() == Some(from) {
            self.editing_provider = Some(to.to_string());
            if let Some(editor) = &mut self.provider_editor {
                editor.id = to.to_string();
            }
        }
        self.toast_info(format!("已重命名为 {to}，同时更新了 {touched} 处引用"));
    }

    fn delete_provider(&mut self, id: &str) {
        let Some(doc) = &mut self.doc else { return };
        provider_ops::remove(&mut doc.config, id);
        if doc.config.str_at(&["model_provider"]).as_deref() == Some(id) {
            doc.config.remove_at(&["model_provider"]);
            self.toast_info("当前服务商已清空，记得去「基础设置」重新选一个");
        }
        if self.editing_provider.as_deref() == Some(id) {
            self.editing_provider = None;
            self.provider_editor = None;
        }
        self.toast_info(format!("已删除服务商 {id}（还没保存到磁盘）"));
    }

    fn delete_model(&mut self, index: usize) {
        let Some(doc) = &mut self.doc else { return };
        let removed = doc
            .catalog
            .as_mut()
            .and_then(|value| catalog::models_mut(value))
            .map(|list| list.remove(index));
        if let Some(removed) = removed {
            let slug = catalog::slug_of(&removed);
            if doc.config.str_at(&["model"]).as_deref() == Some(slug.as_str()) {
                doc.config.remove_at(&["model"]);
                self.toast_info(format!("已删除模型 {slug}，并清空了「当前模型」设置"));
            } else {
                self.toast_info(format!("已删除模型 {slug}"));
            }
        }
        self.editing_model = None;
        self.model_editor = ModelEditor::default();
    }

    fn delete_profile(&mut self, name: &str) {
        let Some(doc) = &mut self.doc else { return };
        doc.config.remove_at(&["profiles", name]);
        if doc.config.str_at(&["profile"]).as_deref() == Some(name) {
            doc.config.remove_at(&["profile"]);
        }
        if self.editing_profile.as_deref() == Some(name) {
            self.editing_profile = None;
            self.profile_editor = None;
        }
        self.toast_info(format!("已删除配置档 {name}"));
    }

    fn rename_model(&mut self, index: usize, slug: &str) {
        let Some(doc) = &mut self.doc else { return };
        let old = doc
            .catalog
            .as_mut()
            .and_then(|value| catalog::models_mut(value))
            .and_then(|list| list.get_mut(index))
            .map(|model| {
                let old = catalog::slug_of(model);
                catalog::set(model, &["slug"], Value::String(slug.to_string()));
                old
            });
        if let Some(old) = old {
            if doc.config.str_at(&["model"]).as_deref() == Some(old.as_str()) {
                doc.config.set_value_at(&["model"], value_str(slug));
            }
            self.model_editor.slug = slug.to_string();
            self.toast_info(format!("slug 已从 {old} 改为 {slug}"));
        }
    }

    fn create_catalog(&mut self, filename: &str) {
        if self.has_raw_draft() || self.model_input_error.is_some() {
            self.toast_error("源文件还有未应用的草稿，请先应用或放弃，再创建模型目录。");
            return;
        }
        let Some(doc) = &mut self.doc else { return };
        if let Err(err) = doc.create_catalog(filename) {
            self.toast_error(format!("{err:#}"));
            return;
        }
        self.editing_model = None;
        self.page = Page::Models;
        self.toast_info("空白模型目录已准备好。添加模型后点「保存配置」才会写入文件。");
    }

    pub fn set_catalog_path(&mut self, path: std::path::PathBuf) {
        if self.ssh_busy() {
            return;
        }
        if self.is_remote() {
            self.start_remote_catalog(path.to_string_lossy().into_owned());
            return;
        }
        if self.has_raw_draft() || self.model_input_error.is_some() {
            self.toast_error("源文件还有未应用的草稿，请先应用或放弃，再切换模型目录。");
            return;
        }
        let Some(doc) = &mut self.doc else { return };
        let stored = match path.strip_prefix(&doc.codex_home) {
            Ok(relative) => relative.to_string_lossy().to_string(),
            Err(_) => path.display().to_string(),
        };
        let mut next_config = doc.config.clone();
        next_config.set_value_at(&["model_catalog_json"], value_str(&stored));
        if let Err(err) = doc.apply_config_text(&next_config.to_string()) {
            self.toast_error(format!("{err:#}"));
            return;
        }
        self.editing_model = None;
        if let Some(catalog) = &doc.catalog {
            let count = catalog::model_count(catalog);
            self.toast_info(format!("已指向 {}，里面有 {count} 个模型", path.display()));
        } else {
            self.toast_error("这个文件读不出来，检查一下是不是合法的 JSON");
        }
        if self.editing_model.is_none() {
            self.select_model(0);
        }
    }

    fn import_remote_models(&mut self, provider: &str, ids: &[String]) {
        let Some(doc) = &mut self.doc else { return };
        if doc.catalog.is_none() {
            self.toast_error("还没有模型目录文件，先创建一个");
            return;
        }
        let mut added = 0usize;
        {
            let existing = catalog::slugs(doc.catalog.as_ref().unwrap());
            let list = catalog::models_mut(doc.catalog.as_mut().unwrap()).unwrap();
            for id in ids {
                if existing.contains(id) || list.iter().any(|m| catalog::slug_of(m) == *id) {
                    continue;
                }
                list.push(catalog::new_model_template(
                    id,
                    id,
                    Some(provider).filter(|p| !p.is_empty()),
                ));
                added += 1;
            }
        }
        self.toast_info(format!(
            "已导入 {added} 个模型，记得检查上下文窗口等参数再保存"
        ));
    }

    // ---------------------------------------------------------------- helpers

    pub fn model_slugs(&self) -> Vec<String> {
        self.doc
            .as_ref()
            .and_then(|doc| doc.catalog.as_ref())
            .map(catalog::slugs)
            .unwrap_or_default()
    }

    fn model_slug_exists(&self, slug: &str) -> bool {
        self.model_slugs().iter().any(|s| s == slug)
    }

    fn model_slug_taken_elsewhere(&self, slug: &str, index: usize) -> bool {
        self.doc
            .as_ref()
            .and_then(|doc| doc.catalog.as_ref())
            .and_then(|value| catalog::models(value))
            .map(|list| {
                list.iter()
                    .enumerate()
                    .any(|(i, m)| i != index && catalog::slug_of(m) == slug)
            })
            .unwrap_or(false)
    }

    fn models_using_provider(&self, id: &str) -> Vec<String> {
        self.doc
            .as_ref()
            .and_then(|doc| doc.catalog.as_ref())
            .and_then(|value| catalog::models(value))
            .map(|list| {
                list.iter()
                    .filter(|m| m.get("provider").and_then(Value::as_str) == Some(id))
                    .map(catalog::slug_of)
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn provider_options(&self) -> Vec<(String, String)> {
        let Some(doc) = &self.doc else {
            return Vec::new();
        };
        doc.selectable_provider_ids()
            .iter()
            .map(|id| (id.clone(), doc.provider_display(id)))
            .collect()
    }

    fn clone_options(&self) -> Vec<(String, String)> {
        self.doc
            .as_ref()
            .and_then(|doc| doc.catalog.as_ref())
            .and_then(|value| catalog::models(value))
            .map(|list| {
                list.iter()
                    .map(|model| {
                        let slug = catalog::slug_of(model);
                        let label = format!("复制「{}」", catalog::display_name(model));
                        (slug, label)
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
}

fn taken_note(app: &App, slug: &str, index: usize) -> bool {
    let Some(doc) = &app.doc else { return false };
    doc.catalog
        .as_ref()
        .and_then(|value| catalog::models(value))
        .map(|list| {
            list.iter()
                .enumerate()
                .any(|(i, m)| i != index && catalog::slug_of(m) == slug.trim())
        })
        .unwrap_or(false)
}
