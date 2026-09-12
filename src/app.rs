//! Application shell: navigation, save/revert flow, dialogs and toasts.

use egui::{
    Align, Color32, Context, CornerRadius, Layout, Margin, RichText, ScrollArea, Stroke, Ui,
};
use std::path::PathBuf;

use crate::doc::Document;
use crate::doc::catalog;
use crate::doc::providers as provider_ops;
use crate::doc::validate::{self, Issue, Severity};
use crate::editors::{ModelEditor, ProfileEditor, ProviderEditor};
use crate::net::{self, HttpOutcome, OutcomeSlot, Probe};
use crate::page::Page;
use crate::server::{self, RestartOutcome, ServerInstance};
use crate::ui::icons;
use crate::ui::pages;
use crate::ui::theme;
use crate::ui::widgets;

/// Modal dialogs that need extra input or a confirmation.
#[derive(Debug, Clone)]
pub enum Dialog {
    NewProvider,
    NewModel {
        slug: String,
        display_name: String,
        provider: String,
        clone_from: String,
    },
    RenameProvider {
        old: String,
        buffer: String,
    },
    RenameModel {
        index: usize,
        buffer: String,
    },
    DeleteProvider(String),
    DeleteModel {
        index: usize,
        slug: String,
    },
    DeleteProfile(String),
    CreateCatalog {
        filename: String,
    },
    ImportRemote {
        provider: String,
        models: Vec<String>,
    },
    ChangeHome {
        buffer: String,
    },
    /// Scan-and-restart the background app-server processes so edits take effect.
    RestartServers,
    DiscardChanges,
    ExitUnsaved,
}

#[derive(Debug)]
pub struct ProbeState {
    pub provider_id: String,
    pub kind: Probe,
    pub slot: OutcomeSlot,
}

pub struct App {
    pub remote: crate::remote_ui::RemoteUi,
    pub doc: Option<Document>,
    pub load_error: Option<String>,
    pub page: Page,
    pub toasts: egui_notify::Toasts,
    pub font_note: String,

    // selection
    pub editing_model: Option<usize>,
    pub model_editor: ModelEditor,
    pub editing_provider: Option<String>,
    pub provider_editor: Option<ProviderEditor>,
    pub editing_profile: Option<String>,
    pub profile_editor: Option<ProfileEditor>,

    // list filters
    pub model_query: String,
    pub provider_query: String,

    pub dialog: Option<Dialog>,
    pub dialog_buffer: String,
    pub dialog_buffer2: String,
    pub dialog_checkbox: Vec<bool>,
    pub probe: Option<ProbeState>,
    pub last_outcome: Option<(String, HttpOutcome)>,
    pub show_diff: bool,
    pub show_about: bool,
    pub raw_tab_is_catalog: bool,
    pub raw_buffer: String,
    pub raw_source: String,
    pub raw_origin: String,
    pub model_input_error: Option<String>,
    pub provider_input_error: Option<String>,

    // restart app-server flow
    pub restart_scan: Vec<ServerInstance>,
    pub restart_selected: Vec<bool>,
    pub restart_results: Vec<RestartOutcome>,
    pub restart_scanned: bool,
    pub saved_needs_restart: bool,
    pub close_confirmed: bool,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        Self::with_home(cc, Document::default_home())
    }

    /// Build the app around an explicit CODEX_HOME (used by tests and by
    /// "切换文件夹").
    pub fn with_home(cc: &eframe::CreationContext<'_>, home: PathBuf) -> Self {
        let font_note = theme::install(&cc.egui_ctx)
            .unwrap_or_else(|| "未找到系统中文字体，界面中文可能显示为方块".to_string());
        cc.egui_ctx
            .set_pixels_per_point(cc.egui_ctx.pixels_per_point().max(1.0));

        let mut app = Self {
            remote: crate::remote_ui::RemoteUi::new(cc.egui_ctx.clone(), &home),
            doc: None,
            load_error: None,
            page: Page::Overview,
            toasts: egui_notify::Toasts::new().with_anchor(egui_notify::Anchor::BottomRight),
            font_note,
            editing_model: None,
            model_editor: ModelEditor::default(),
            editing_provider: None,
            provider_editor: None,
            editing_profile: None,
            profile_editor: None,
            model_query: String::new(),
            provider_query: String::new(),
            dialog: None,
            dialog_buffer: String::new(),
            dialog_buffer2: String::new(),
            dialog_checkbox: Vec::new(),
            probe: None,
            last_outcome: None,
            show_diff: false,
            show_about: false,
            raw_tab_is_catalog: false,
            raw_buffer: String::new(),
            raw_source: String::new(),
            raw_origin: String::new(),
            model_input_error: None,
            provider_input_error: None,
            restart_scan: Vec::new(),
            restart_selected: Vec::new(),
            restart_results: Vec::new(),
            restart_scanned: false,
            saved_needs_restart: false,
            close_confirmed: false,
        };
        app.load(home);
        app
    }

    // -- document lifecycle -------------------------------------------------

    pub fn load(&mut self, home: PathBuf) {
        if self.ssh_busy() {
            return;
        }
        match Document::load(home) {
            Ok(doc) => {
                self.reset_editors();
                let notes = doc.load_notes.clone();
                let path = doc.config_path.display().to_string();
                self.remote.local_home = doc.codex_home.display().to_string();
                self.remote.target = None;
                self.remote.error = None;
                self.saved_needs_restart = false;
                self.last_outcome = None;
                self.show_diff = false;
                self.doc = Some(doc);
                self.load_error = None;
                self.toasts
                    .success(format!("已载入 {path}"))
                    .duration(Some(std::time::Duration::from_secs(4)));
                for note in notes {
                    self.toasts
                        .warning(note)
                        .duration(Some(std::time::Duration::from_secs(8)));
                }
            }
            Err(err) => {
                if self.doc.is_some() {
                    self.remote.error = Some(format!("载入失败，已保留当前修改：{err:#}"));
                    self.toast_error(format!("载入失败，已保留当前修改：{err:#}"));
                } else {
                    self.load_error = Some(format!("{err:#}"));
                }
                return;
            }
        }
        self.raw_buffer.clear();
        self.raw_source.clear();
    }

    pub fn reset_editors(&mut self) {
        self.model_input_error = None;
        self.provider_input_error = None;
        self.editing_model = None;
        self.model_editor = ModelEditor::default();
        self.editing_provider = None;
        self.provider_editor = None;
        self.editing_profile = None;
        self.profile_editor = None;
        self.dialog = None;
        self.probe = None;
    }

    pub fn select_model(&mut self, index: usize) {
        if self.editing_model == Some(index) {
            return;
        }
        if let Some(error) = &self.model_input_error {
            self.toast_error(error.clone());
            return;
        }
        let Some(doc) = &self.doc else { return };
        let Some(model) = doc
            .catalog
            .as_ref()
            .and_then(|value| catalog::models(value))
            .and_then(|list| list.get(index))
        else {
            return;
        };
        self.model_editor = ModelEditor::from_value(model);
        self.editing_model = Some(index);
    }

    pub fn select_provider(&mut self, id: &str) {
        if self.editing_provider.as_deref() == Some(id) && self.provider_editor.is_some() {
            return;
        }
        if let Some(error) = &self.provider_input_error {
            self.toast_error(error.clone());
            return;
        }
        let Some(doc) = &self.doc else { return };
        if let Some(view) = provider_ops::view(&doc.config, id) {
            self.provider_editor = Some(ProviderEditor::from_view(&view));
            self.editing_provider = Some(id.to_string());
        }
    }

    pub fn select_profile(&mut self, name: &str) {
        if self.editing_profile.as_deref() == Some(name) && self.profile_editor.is_some() {
            return;
        }
        let Some(doc) = &self.doc else { return };
        self.profile_editor = Some(ProfileEditor::from_config(&doc.config, name));
        self.editing_profile = Some(name.to_string());
    }

    /// Push the current model editor back into the catalog.
    pub fn commit_model_editor(&mut self) {
        let Some(index) = self.editing_model else {
            return;
        };
        let editor = self.model_editor.clone();
        self.model_input_error = editor.numeric_error();
        if self.model_input_error.is_some() {
            return;
        }
        if let Some(doc) = &mut self.doc
            && let Some(list) = doc
                .catalog
                .as_mut()
                .and_then(|value| catalog::models_mut(value))
            && let Some(model) = list.get_mut(index)
        {
            editor.write_to(model);
        }
    }

    pub fn commit_provider_editor(&mut self) {
        let Some(editor) = self.provider_editor.clone() else {
            return;
        };
        self.provider_input_error = editor.numeric_error();
        if self.provider_input_error.is_some() {
            return;
        }
        if let Some(doc) = &mut self.doc {
            editor.write_to(&mut doc.config);
        }
    }

    pub fn commit_profile_editor(&mut self) {
        let Some(editor) = self.profile_editor.clone() else {
            return;
        };
        if let Some(doc) = &mut self.doc {
            editor.write_to(&mut doc.config);
        }
    }

    // -- save / revert ------------------------------------------------------

    pub fn issues(&self) -> Vec<Issue> {
        match &self.doc {
            Some(doc) => validate::validate(doc),
            None => Vec::new(),
        }
    }

    pub fn save(&mut self) {
        if self.ssh_busy() {
            return;
        }
        if let Some(error) = self.model_input_error.clone() {
            self.toast_error(error);
            self.page = Page::Models;
            return;
        }
        if let Some(error) = self.provider_input_error.clone() {
            self.toast_error(error);
            self.page = Page::Providers;
            return;
        }
        if self.has_raw_draft() {
            self.toast_error("源文件还有未应用的内容，请先「应用到编辑器」，再保存配置。");
            self.page = Page::Raw;
            return;
        }
        let errors = self
            .issues()
            .into_iter()
            .filter(|issue| issue.severity == Severity::Error)
            .collect::<Vec<_>>();
        if !errors.is_empty() {
            let first = &errors[0];
            self.toasts
                .error(format!(
                    "还有 {} 个错误没解决：{}",
                    errors.len(),
                    first.title
                ))
                .duration(Some(std::time::Duration::from_secs(8)));
            self.page = first.page;
            return;
        }
        if self.is_remote() {
            self.start_remote_save();
            return;
        }
        let Some(doc) = &mut self.doc else { return };
        match doc.save() {
            Ok(report) => {
                if report.written.is_empty() {
                    self.toasts.info("没有需要保存的修改");
                } else {
                    self.saved_needs_restart = true;
                    let backups = report.backups.len();
                    self.toasts
                        .success(format!(
                            "已保存 {} 个文件（自动备份了 {backups} 份）",
                            report.written.len()
                        ))
                        .duration(Some(std::time::Duration::from_secs(5)));
                }
                self.raw_buffer.clear();
                self.raw_source.clear();
            }
            Err(err) => {
                self.toasts
                    .error(format!("保存失败：{err:#}"))
                    .duration(Some(std::time::Duration::from_secs(12)));
            }
        }
    }

    pub fn discard(&mut self) {
        if self.ssh_busy() {
            return;
        }
        if self.is_remote() {
            let Some(target) = self.remote.target.clone() else {
                self.toast_error("远程目标未绑定，请通过「切换环境」重新连接。");
                return;
            };
            self.remote.discard_confirmed = true;
            self.connect_remote(target);
            return;
        }
        if let Some(doc) = &mut self.doc {
            let home = doc.codex_home.clone();
            self.load(home);
            self.toasts.info("已放弃所有未保存的修改");
        }
    }

    // -- misc actions -------------------------------------------------------

    pub fn has_raw_draft(&self) -> bool {
        !self.raw_source.is_empty() && self.raw_buffer != self.raw_origin
    }

    pub fn has_unsaved_changes(&self) -> bool {
        self.has_raw_draft()
            || self.model_input_error.is_some()
            || self.provider_input_error.is_some()
            || self.doc.as_ref().is_some_and(Document::dirty)
    }

    pub fn request_discard(&mut self) {
        if self.has_unsaved_changes() {
            self.dialog = Some(Dialog::DiscardChanges);
        } else {
            self.discard();
        }
    }

    pub fn toast_info(&mut self, text: impl Into<String>) {
        self.toasts.info(text.into());
    }

    pub fn toast_error(&mut self, text: impl Into<String>) {
        self.toasts
            .error(text.into())
            .duration(Some(std::time::Duration::from_secs(10)));
    }

    // -- restart app-server -------------------------------------------------

    /// Open the "restart app-server" dialog and kick off a fresh scan.
    pub fn open_restart_dialog(&mut self) {
        if self.is_remote() || self.ssh_busy() {
            self.toast_info("远程配置保存后，请在目标机器重启 Codex；本工具不会重启本地服务。");
            return;
        }
        self.rescan_servers();
        self.restart_results.clear();
        self.dialog = Some(Dialog::RestartServers);
    }

    /// Re-scan running app-server processes and default the selection to every
    /// instance we are allowed to restart (i.e. not the one hosting us).
    pub fn rescan_servers(&mut self) {
        if self.is_remote() || self.ssh_busy() {
            return;
        }
        let found = server::scan();
        self.restart_selected = found.iter().map(|i| !i.is_our_host).collect();
        self.restart_scan = found;
        self.restart_scanned = true;
    }

    /// Restart every checked instance.
    pub fn run_restart_selected(&mut self) {
        if self.is_remote() || self.ssh_busy() {
            return;
        }
        let mut results = Vec::new();
        let mut restarted = 0usize;
        for (inst, checked) in self.restart_scan.iter().zip(self.restart_selected.iter()) {
            if !checked {
                continue;
            }
            let outcome = server::restart_instance(inst);
            if outcome.ok {
                restarted += 1;
            }
            results.push(outcome);
        }
        self.restart_results = results;
        if restarted > 0 {
            self.toasts
                .success(format!(
                    "已重启 {restarted} 个 App Server，宿主会自动拉起读取新配置的服务"
                ))
                .duration(Some(std::time::Duration::from_secs(6)));
        }
        // Refresh the list so the user sees the new PIDs once hosts respawn.
        self.rescan_servers();
    }

    pub fn open_in_editor(&mut self, path: PathBuf) {
        if self.is_remote() {
            self.page = Page::Raw;
            self.toast_info("远程文件请在「源文件编辑」查看；不会打开本地同名路径。");
            return;
        }
        if let Err(err) = open::that(&path) {
            self.toast_error(format!("无法用系统默认程序打开：{err}"));
        }
    }

    pub fn start_probe(
        &mut self,
        provider_id: &str,
        kind: Probe,
        model: Option<String>,
        ctx: &Context,
    ) {
        if self.is_remote() || self.ssh_busy() {
            self.toast_info("远程模式不从本机发起服务商测试或读取本机密钥。请在目标机器验证连接。");
            return;
        }
        let Some(doc) = &self.doc else { return };
        let Some(view) = provider_ops::view(&doc.config, provider_id) else {
            self.toast_error("找不到这个服务商，先保存一次再试");
            return;
        };
        let slot = net::spawn(ctx, view, model, kind);
        self.probe = Some(ProbeState {
            provider_id: provider_id.to_string(),
            kind,
            slot,
        });
    }

    pub fn poll_probe(&mut self) -> Option<(String, Probe, HttpOutcome)> {
        let finished = self.probe.as_ref().and_then(|state| {
            net::take(&state.slot).map(|result| (state.provider_id.clone(), state.kind, result))
        });
        if let Some((id, kind, result)) = finished {
            self.probe = None;
            match result {
                Ok(outcome) => {
                    if kind == Probe::Chat {
                        if outcome.ok {
                            self.toasts.success(format!("{id}：{}", outcome.summary));
                        } else {
                            self.toasts
                                .error(format!("{id}：{}", outcome.summary))
                                .duration(Some(std::time::Duration::from_secs(10)));
                        }
                    }
                    self.last_outcome = Some((id.clone(), outcome.clone()));
                    return Some((id, kind, outcome));
                }
                Err(err) => {
                    self.toast_error(err.clone());
                    self.last_outcome = Some((
                        id.clone(),
                        HttpOutcome {
                            ok: false,
                            status: 0,
                            summary: "请求失败".into(),
                            detail: err,
                            elapsed_ms: 0,
                            remote_models: Vec::new(),
                        },
                    ));
                }
            }
        }
        None
    }

    /// Model slug used when probing a provider.
    pub fn probe_model_for(&self, provider_id: &str) -> Option<String> {
        let doc = self.doc.as_ref()?;
        if let Some(catalog) = &doc.catalog {
            let active = doc.config.str_at(&["model"]).unwrap_or_default();
            if let Some(list) = catalog::models(catalog)
                && let Some(model) = list
                    .iter()
                    .find(|m| {
                        m.get("slug").and_then(serde_json::Value::as_str) == Some(active.as_str())
                    })
                    .or_else(|| {
                        list.iter().find(|m| {
                            m.get("provider").and_then(serde_json::Value::as_str)
                                == Some(provider_id)
                        })
                    })
            {
                return model
                    .get("slug")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_string);
            }
        }
        None
    }
}

use crate::doc::toml_ext::TomlPathExt;

impl eframe::App for App {
    fn ui(&mut self, root: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = root.ctx().clone();
        self.poll_ssh();
        if self.ssh_busy() && ctx.input(|input| input.viewport().close_requested()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        }
        if ctx.input(|input| input.viewport().close_requested())
            && !self.ssh_busy()
            && self.has_unsaved_changes()
            && !self.close_confirmed
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.dialog = Some(Dialog::ExitUnsaved);
        }
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::S)) {
            self.save();
        }
        // Press ? to open help
        if !ctx.egui_wants_keyboard_input() && ctx.input(|i| i.key_pressed(egui::Key::Questionmark))
        {
            self.show_about = true;
        }
        if let Some((provider, kind, outcome)) = self.poll_probe()
            && kind == Probe::ListModels
            && outcome.ok
        {
            let models = outcome.remote_models.clone();
            if models.is_empty() {
                self.toast_info("服务商返回成功，但没解析出模型列表");
            } else {
                self.dialog_checkbox = vec![false; models.len()];
                self.dialog = Some(Dialog::ImportRemote { provider, models });
            }
        }

        self.sidebar(root);
        self.top_bar(root);
        self.status_bar(root);

        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(theme::BG)
                    .inner_margin(Margin::symmetric(28, 22)),
            )
            .show(root, |ui| {
                if self.ssh_busy() {
                    ui.disable();
                }
                if self.doc.is_none() {
                    self.error_page(ui);
                    return;
                }
                widgets::page_header(
                    ui,
                    self.page.icon(),
                    self.page.title(),
                    self.page.subtitle(),
                );
                if self.is_remote() {
                    widgets::hint(
                        ui,
                        &format!(
                            "{}  /  {}  ·  远程快照，点击保存才会写入",
                            self.target_label(),
                            self.doc
                                .as_ref()
                                .map(|doc| doc.codex_home.display().to_string())
                                .unwrap_or_default()
                        ),
                    );
                    ui.add_space(10.0);
                }
                if let Some(error) = &self.remote.error {
                    ScrollArea::vertical()
                        .id_salt("environment-error")
                        .max_height(100.0)
                        .show(ui, |ui| widgets::note(ui, error, theme::DANGER));
                    ui.add_space(8.0);
                }
                match self.page {
                    // These two manage their own split view and scrolling.
                    Page::Models => pages::models::show(self, ui, &ctx),
                    Page::Providers => pages::providers::show(self, ui, &ctx),
                    page => {
                        ScrollArea::vertical()
                            .id_salt(("page-scroll", page))
                            .auto_shrink([false, false])
                            .show(ui, |ui| match page {
                                Page::Overview => pages::overview::show(self, ui, &ctx),
                                Page::Profiles => pages::profiles::show(self, ui, &ctx),
                                Page::Advanced => pages::advanced::show(self, ui, &ctx),
                                Page::Raw => pages::raw::show(self, ui, &ctx),
                                Page::Models | Page::Providers => {}
                            });
                    }
                }
            });

        if !self.ssh_busy() {
            self.dialogs(&ctx);
            self.diff_window(&ctx);
        }
        self.environment_windows(&ctx);
        self.about_window(&ctx);
        self.toasts.show(&ctx);
    }
}

impl App {
    fn error_page(&mut self, ui: &mut Ui) {
        widgets::page_header(
            ui,
            icons::WARNING,
            "无法载入配置",
            "先解决下面的问题，才能开始编辑",
        );
        widgets::card(ui, |ui| {
            ui.set_width(ui.available_width());
            if let Some(err) = &self.load_error {
                ui.label(
                    RichText::new(err)
                        .monospace()
                        .size(12.5)
                        .color(theme::DANGER),
                );
            }
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if widgets::ghost_button(ui, "选择 CODEX_HOME 文件夹").clicked()
                    && let Some(folder) = rfd::FileDialog::new().pick_folder()
                {
                    self.load(folder);
                }
                if widgets::ghost_button(ui, "重试默认位置 (~/.codex)").clicked() {
                    self.load(Document::default_home());
                }
            });
        });
    }

    fn top_bar(&mut self, root: &mut Ui) {
        egui::Panel::top("top_bar")
            .frame(
                egui::Frame::new()
                    .fill(theme::PANEL)
                    .inner_margin(Margin::symmetric(24, 14))
                    .stroke(Stroke::new(1.0, theme::BORDER)),
            )
            .show(root, |ui| {
                if self.ssh_busy() {
                    ui.disable();
                }
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("我的工作空间")
                            .size(12.5)
                            .color(theme::TEXT_MUTED),
                    );
                    ui.label(RichText::new("/").color(theme::BORDER_STRONG));
                    ui.label(
                        RichText::new(self.target_label())
                            .size(12.5)
                            .color(theme::TEXT),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let dirty = self.has_unsaved_changes();
                        ui.add_enabled_ui(dirty, |ui| {
                            let shortcut = if cfg!(target_os = "macos") {
                                "⌘ S"
                            } else {
                                "Ctrl+S"
                            };
                            if widgets::primary_button(ui, "保存配置")
                                .on_hover_text(format!("写入前自动备份 · {shortcut}"))
                                .clicked()
                            {
                                self.save();
                            }
                            if widgets::ghost_button(ui, "预览修改").clicked() {
                                self.show_diff = true;
                            }
                            if widgets::link_button(ui, "撤销修改", theme::TEXT_DIM).clicked() {
                                self.request_discard();
                            }
                        });
                    });
                });
            });
    }

    fn status_bar(&mut self, root: &mut Ui) {
        egui::Panel::bottom("status_bar")
            .frame(
                egui::Frame::new()
                    .fill(theme::PANEL)
                    .inner_margin(Margin::symmetric(24, 10))
                    .stroke(Stroke::new(1.0, theme::BORDER)),
            )
            .show(root, |ui| {
                if self.ssh_busy() {
                    ui.disable();
                }
                ui.horizontal(|ui| {
                    let (text, color) = if self.ssh_busy() {
                        ("SSH 操作进行中…", theme::INFO)
                    } else if self.doc.is_none() {
                        ("配置尚未载入", theme::DANGER)
                    } else if self.has_raw_draft() {
                        ("源文件草稿尚未应用", theme::WARN)
                    } else if self.has_unsaved_changes() {
                        ("有未保存的修改", theme::WARN)
                    } else if self.saved_needs_restart {
                        ("已保存 · 重启 Codex 后使用新配置", theme::OK)
                    } else if self.is_remote() {
                        ("已载入远程快照 · 非实时同步", theme::TEXT_DIM)
                    } else {
                        ("已与本地文件同步", theme::TEXT_DIM)
                    };
                    ui.label(RichText::new(icons::SHIELD).size(16.0).color(color));
                    ui.label(RichText::new(text).size(12.0).color(color));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if self.is_remote() {
                            ui.label(
                                RichText::new("远端重启需手动完成")
                                    .size(11.5)
                                    .color(theme::TEXT_MUTED),
                            );
                        } else if widgets::link_button(ui, "重启服务…", theme::ACCENT).clicked()
                        {
                            self.open_restart_dialog();
                        }
                        if ui.available_width() > 160.0 {
                            ui.label(
                                RichText::new("保存前自动备份")
                                    .size(11.5)
                                    .color(theme::TEXT_MUTED),
                            );
                        }
                    });
                });
            });
    }

    fn sidebar(&mut self, root: &mut Ui) {
        egui::Panel::left("sidebar")
            .exact_size(208.0)
            .resizable(false)
            .frame(
                egui::Frame::new()
                    .fill(theme::PANEL)
                    .inner_margin(Margin::symmetric(16, 24))
                    .stroke(Stroke::new(1.0, theme::BORDER)),
            )
            .show(root, |ui| {
                if self.ssh_busy() {
                    ui.disable();
                }
                ScrollArea::vertical()
                    .id_salt("sidebar-navigation")
                    .max_height((ui.available_height() - 132.0).max(160.0))
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 4.0;
                        ui.horizontal(|ui| {
                            widgets::icon_tile(ui, icons::BRAND, 36.0, 24.0, theme::ACCENT);
                            ui.vertical(|ui| {
                                ui.spacing_mut().item_spacing.y = 0.0;
                                ui.label(
                                    RichText::new("Codex")
                                        .size(23.0)
                                        .strong()
                                        .color(theme::TEXT),
                                );
                                ui.label(
                                    RichText::new("配置助手").size(12.0).color(theme::TEXT_DIM),
                                );
                            });
                        });
                        ui.add_space(20.0);
                        theme::subtle_frame().show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.label(
                                RichText::new("当前环境")
                                    .size(10.5)
                                    .color(theme::TEXT_MUTED),
                            );
                            ui.label(
                                RichText::new(self.target_label())
                                    .size(13.0)
                                    .strong()
                                    .color(theme::TEXT),
                            );
                            if widgets::link_button(ui, "切换环境…", theme::ACCENT).clicked()
                            {
                                self.open_environment();
                            }
                        });
                        ui.add_space(18.0);
                        ui.label(
                            RichText::new("日常使用")
                                .size(11.0)
                                .color(theme::TEXT_MUTED),
                        );
                        ui.add_space(8.0);
                        let counts = self.sidebar_counts();
                        for page in Page::ALL {
                            if page == Page::Profiles {
                                ui.add_space(16.0);
                                ui.label(
                                    RichText::new("进阶工具")
                                        .size(11.0)
                                        .color(theme::TEXT_MUTED),
                                );
                                ui.add_space(8.0);
                            }
                            let selected = self.page == page;
                            if self.nav_item(
                                ui,
                                page,
                                selected,
                                counts.get(&page).copied().flatten(),
                            ) {
                                self.page = page;
                            }
                            ui.add_space(2.0);
                        }
                    });

                ui.with_layout(Layout::bottom_up(Align::LEFT), |ui| {
                    ui.label(
                        RichText::new(concat!("桌面版  /  v", env!("CARGO_PKG_VERSION")))
                            .size(10.5)
                            .color(theme::TEXT_MUTED),
                    );
                    ui.add_space(12.0);
                    if widgets::link_button(ui, "使用帮助", theme::TEXT_DIM).clicked() {
                        self.show_about = true;
                    }
                    ui.add_space(4.0);
                    if let Some(doc) = &self.doc {
                        let home = doc.codex_home.clone();
                        if widgets::link_button(
                            ui,
                            if self.is_remote() {
                                "查看远程源文件"
                            } else {
                                "打开配置文件夹"
                            },
                            theme::TEXT_DIM,
                        )
                        .on_hover_text(home.display().to_string())
                        .clicked()
                        {
                            self.open_in_editor(home);
                        }
                    }
                    ui.add_space(10.0);
                    ui.separator();
                });
            });
    }

    /// One navigation row: an accent bar on the left when selected, an icon,
    /// the page name and an optional count badge. Returns true when clicked.
    fn nav_item(&self, ui: &mut Ui, page: Page, selected: bool, count: Option<usize>) -> bool {
        let color = if selected {
            theme::ACCENT
        } else {
            theme::TEXT_DIM
        };
        let response = ui.add_sized(
            [ui.available_width(), 36.0],
            egui::Button::new(RichText::new(page.title()).size(13.5).color(color))
                .fill(if selected {
                    theme::ACCENT_WEAK
                } else {
                    Color32::TRANSPARENT
                })
                .stroke(Stroke::NONE)
                .corner_radius(CornerRadius::same(10))
                .selected(selected),
        );
        ui.painter().text(
            egui::pos2(response.rect.left() + 18.0, response.rect.center().y),
            egui::Align2::CENTER_CENTER,
            page.icon(),
            egui::FontId::proportional(18.0),
            color,
        );
        if let Some(count) = count {
            ui.painter().text(
                egui::pos2(response.rect.right() - 14.0, response.rect.center().y),
                egui::Align2::CENTER_CENTER,
                count.to_string(),
                egui::FontId::proportional(11.0),
                theme::TEXT_MUTED,
            );
        }
        response.clicked()
    }

    fn sidebar_counts(&self) -> std::collections::HashMap<Page, Option<usize>> {
        let mut counts = std::collections::HashMap::new();
        if let Some(doc) = &self.doc {
            counts.insert(Page::Models, doc.catalog.as_ref().map(catalog::model_count));
            counts.insert(Page::Providers, Some(doc.provider_ids().len()));
            counts.insert(Page::Profiles, Some(doc.profile_ids().len()));
        }
        counts
    }

    fn diff_window(&mut self, ctx: &Context) {
        if !self.show_diff {
            return;
        }
        let Some(doc) = &self.doc else { return };
        let config_old = doc.config_on_disk().to_string();
        let config_new = doc.config_text();
        let catalog_old = doc.catalog_on_disk().to_string();
        let catalog_new = doc.catalog_text();
        let config_dirty = doc.config_dirty();
        let catalog_dirty = doc.catalog_dirty();
        let config_path = doc.config_path.display().to_string();
        let catalog_path = doc
            .catalog_path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "(未设置模型目录)".to_string());

        let mut open = self.show_diff;
        let mut close_requested = false;
        egui::Window::new("保存前对比差异")
            .open(&mut open)
            .collapsible(false)
            .default_size([900.0, 620.0])
            .vscroll(true)
            .show(ctx, |ui| {
                if !config_dirty && !catalog_dirty {
                    widgets::note(ui, "目前没有未保存的修改。", theme::TEXT_DIM);
                    return;
                }
                ui.label(
                    RichText::new("红色减号是原有内容，绿色加号是修改后的内容。")
                        .size(12.0)
                        .color(theme::TEXT_DIM),
                );
                ui.add_space(6.0);
                if config_dirty {
                    ui.label(
                        RichText::new(format!("config.toml · {config_path}"))
                            .size(13.0)
                            .strong()
                            .color(theme::TEXT),
                    );
                    diff_view(ui, "diff-config", &config_old, &config_new);
                }
                if catalog_dirty {
                    ui.add_space(10.0);
                    ui.label(
                        RichText::new(format!("模型目录 · {catalog_path}"))
                            .size(13.0)
                            .strong()
                            .color(theme::TEXT),
                    );
                    diff_view(ui, "diff-catalog", &catalog_old, &catalog_new);
                }
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if widgets::primary_button(ui, "确认并保存").clicked() {
                        close_requested = true;
                        self.save();
                    }
                    if widgets::ghost_button(ui, "关闭").clicked() {
                        close_requested = true;
                    }
                });
            });
        self.show_diff = open && !close_requested;
    }

    fn about_window(&mut self, ctx: &Context) {
        if !self.show_about {
            return;
        }
        let mut open = self.show_about;
        let mut close_requested = false;
        let font_note = self.font_note.clone();
        egui::Window::new("关于 Codex 配置助手")
            .open(&mut open)
            .collapsible(false)
            .default_size([560.0, 460.0])
            .show(ctx, |ui| {
                ui.label(RichText::new(concat!("Codex 配置助手 v", env!("CARGO_PKG_VERSION"))).size(17.0).strong());
                ui.label(
                    RichText::new("一个给 Codex 命令行工具用的图形化配置编辑器：不用手写 TOML 也能改配置、加模型、加服务商。")
                        .size(13.0)
                        .color(theme::TEXT_DIM),
                );
                ui.add_space(8.0);
                widgets::section(ui, icons::STEPS, "使用小贴士", "", |ui| {
                    for tip in [
                        "1. 先去「服务商」页添加一个模型服务商（有现成模板，点一下就填好）。",
                        "2. 再去「模型管理」页添加模型，并给每个模型选好服务商。",
                        "3. 回到「开始使用」页，选择当前模型和服务商；不确定的选项保持默认。",
                        "4. 点右上角「保存配置」。保存前会自动备份；重启 Codex 后使用新配置。",
                    ] {
                        ui.label(RichText::new(tip).size(12.5).color(theme::TEXT_DIM));
                        ui.add_space(2.0);
                    }
                });
                ui.add_space(6.0);

                // Keyboard shortcuts section
                widgets::section(ui, icons::GEAR, "键盘快捷键", "", |ui| {
                    let is_mac = cfg!(target_os = "macos");
                    let cmd = if is_mac { "⌘" } else { "Ctrl+" };
                    let shortcuts = [
                        (format!("{}S", cmd), "保存配置"),
                        ("?".to_string(), "打开这个帮助窗口"),
                    ];
                    for (key, action) in shortcuts {
                        ui.horizontal(|ui| {
                            widgets::code_chip(ui, &key);
                            ui.label(
                                RichText::new(action)
                                    .size(12.5)
                                    .color(theme::TEXT_DIM),
                            );
                        });
                        ui.add_space(2.0);
                    }
                });
                ui.add_space(6.0);
                widgets::kv_row(ui, "中文字体", &font_note);
                widgets::kv_row(ui, "配置文件", "config.toml（TOML，保留你的注释和排版）");
                widgets::kv_row(ui, "模型目录", "model_catalog_json 指向的 JSON 文件");
                ui.add_space(6.0);
                if widgets::ghost_button(ui, "关闭").clicked() {
                    close_requested = true;
                }
            });
        self.show_about = open && !close_requested;
    }
}

fn diff_view(ui: &mut Ui, id: &str, old: &str, new: &str) {
    use similar::{ChangeTag, TextDiff};
    let diff = TextDiff::from_lines(old, new);
    ScrollArea::vertical()
        .id_salt(id)
        .max_height(300.0)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            theme::subtle_frame().show(ui, |ui| {
                ui.set_width(ui.available_width());
                for change in diff.iter_all_changes() {
                    let (color, prefix) = match change.tag() {
                        ChangeTag::Equal => (theme::TEXT_MUTED, "  "),
                        ChangeTag::Delete => (theme::DANGER, "- "),
                        ChangeTag::Insert => (theme::OK, "+ "),
                    };
                    let text = change.value().trim_end_matches('\n');
                    ui.label(
                        RichText::new(format!("{prefix}{text}"))
                            .monospace()
                            .size(11.5)
                            .color(color),
                    );
                }
            });
        });
}
