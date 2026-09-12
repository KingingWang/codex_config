//! Application shell: navigation, save/revert flow, dialogs and toasts.

use std::path::PathBuf;
use egui::{Align, Color32, Context, CornerRadius, Layout, Margin, RichText, ScrollArea, Stroke, Ui};

use crate::doc::catalog;
use crate::doc::providers as provider_ops;
use crate::doc::validate::{self, Issue, Severity};
use crate::doc::Document;
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
    RenameProvider { old: String, buffer: String },
    RenameModel { index: usize, buffer: String },
    DeleteProvider(String),
    DeleteModel { index: usize, slug: String },
    DeleteProfile(String),
    CreateCatalog { filename: String },
    ImportRemote { provider: String, models: Vec<String> },
    ChangeHome { buffer: String },
    /// Scan-and-restart the background app-server processes so edits take effect.
    RestartServers,
}

#[derive(Debug)]
pub struct ProbeState {
    pub provider_id: String,
    pub kind: Probe,
    pub slot: OutcomeSlot,
}

pub struct App {
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

    // restart app-server flow
    pub restart_scan: Vec<ServerInstance>,
    pub restart_selected: Vec<bool>,
    pub restart_results: Vec<RestartOutcome>,
    pub restart_scanned: bool,
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
        cc.egui_ctx.set_pixels_per_point(cc.egui_ctx.pixels_per_point().max(1.0));

        let mut app = Self {
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
            restart_scan: Vec::new(),
            restart_selected: Vec::new(),
            restart_results: Vec::new(),
            restart_scanned: false,
        };
        app.load(home);
        app
    }

    // -- document lifecycle -------------------------------------------------

    pub fn load(&mut self, home: PathBuf) {
        match Document::load(home) {
            Ok(doc) => {
                self.reset_editors();
                let notes = doc.load_notes.clone();
                let path = doc.config_path.display().to_string();
                self.doc = Some(doc);
                self.load_error = None;
                self.toasts
                    .success(format!("已载入 {path}"))
                    .duration(Some(std::time::Duration::from_secs(4)));
                for note in notes {
                    self.toasts.warning(note).duration(Some(std::time::Duration::from_secs(8)));
                }
            }
            Err(err) => {
                self.doc = None;
                self.load_error = Some(format!("{err:#}"));
            }
        }
        self.raw_buffer.clear();
        self.raw_source.clear();
    }

    pub fn reset_editors(&mut self) {
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
        let Some(index) = self.editing_model else { return };
        let editor = self.model_editor.clone();
        if let Some(doc) = &mut self.doc
            && let Some(list) = doc.catalog.as_mut().and_then(|value| catalog::models_mut(value))
            && let Some(model) = list.get_mut(index)
        {
            editor.write_to(model);
        }
    }

    pub fn commit_provider_editor(&mut self) {
        let Some(editor) = self.provider_editor.clone() else {
            return;
        };
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
        let errors = self
            .issues()
            .into_iter()
            .filter(|issue| issue.severity == Severity::Error)
            .collect::<Vec<_>>();
        if !errors.is_empty() {
            let first = &errors[0];
            self.toasts
                .error(format!("还有 {} 个错误没解决：{}", errors.len(), first.title))
                .duration(Some(std::time::Duration::from_secs(8)));
            self.page = first.page;
            return;
        }
        let Some(doc) = &mut self.doc else { return };
        match doc.save() {
            Ok(report) => {
                if report.written.is_empty() {
                    self.toasts.info("没有需要保存的修改");
                } else {
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
        if let Some(doc) = &mut self.doc {
            let home = doc.codex_home.clone();
            self.load(home);
            self.toasts.info("已放弃所有未保存的修改");
        }
    }

    // -- misc actions -------------------------------------------------------

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
        self.rescan_servers();
        self.restart_results.clear();
        self.dialog = Some(Dialog::RestartServers);
    }

    /// Re-scan running app-server processes and default the selection to every
    /// instance we are allowed to restart (i.e. not the one hosting us).
    pub fn rescan_servers(&mut self) {
        let found = server::scan();
        self.restart_selected = found.iter().map(|i| !i.is_our_host).collect();
        self.restart_scan = found;
        self.restart_scanned = true;
    }

    /// Restart every checked instance.
    pub fn run_restart_selected(&mut self) {
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
                .success(format!("已重启 {restarted} 个 App Server，宿主会自动拉起读取新配置的服务"))
                .duration(Some(std::time::Duration::from_secs(6)));
        }
        // Refresh the list so the user sees the new PIDs once hosts respawn.
        self.rescan_servers();
    }

    pub fn open_in_editor(&mut self, path: PathBuf) {
        if let Err(err) = open::that(&path) {
            self.toast_error(format!("无法用系统默认程序打开：{err}"));
        }
    }

    pub fn start_probe(&mut self, provider_id: &str, kind: Probe, model: Option<String>, ctx: &Context) {
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
            if let Some(list) = catalog::models(catalog) {
                if let Some(model) = list
                    .iter()
                    .find(|m| m.get("slug").and_then(serde_json::Value::as_str) == Some(active.as_str()))
                    .or_else(|| {
                        list.iter().find(|m| {
                            m.get("provider").and_then(serde_json::Value::as_str) == Some(provider_id)
                        })
                    })
                {
                    return model.get("slug").and_then(serde_json::Value::as_str).map(str::to_string);
                }
            }
        }
        None
    }
}

use crate::doc::toml_ext::TomlPathExt;

impl eframe::App for App {
    fn ui(&mut self, root: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = root.ctx().clone();
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::S)) {
            self.save();
        }
        if let Some((provider, kind, outcome)) = self.poll_probe() {
            if kind == Probe::ListModels && outcome.ok {
                let models = outcome.remote_models.clone();
                if models.is_empty() {
                    self.toast_info("服务商返回成功，但没解析出模型列表");
                } else {
                    self.dialog_checkbox = vec![false; models.len()];
                    self.dialog = Some(Dialog::ImportRemote { provider, models });
                }
            }
        }

        self.top_bar(root);
        self.sidebar(root);

        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(theme::BG)
                    .inner_margin(Margin::symmetric(22, 18)),
            )
            .show(root, |ui| {
                if self.doc.is_none() {
                    self.error_page(ui);
                    return;
                }
                widgets::page_header(ui, self.page.icon(), self.page.title(), self.page.subtitle());
                match self.page {
                    // These two manage their own split view and scrolling.
                    Page::Models => pages::models::show(self, ui, &ctx),
                    Page::Providers => pages::providers::show(self, ui, &ctx),
                    page => {
                        ScrollArea::vertical()
                            .auto_shrink([false, false])
                            .show(ui, |ui| match page {
                                Page::Overview => pages::overview::show(self, ui, &ctx),
                                Page::Profiles => pages::profiles::show(self, ui, &ctx),
                                Page::Advanced => pages::advanced::show(self, ui, &ctx),
                                Page::Raw => pages::raw::show(self, ui, &ctx),
                                Page::Models | Page::Providers => {}
                            });
                        ui.add_space(40.0);
                    }
                }
            });

        self.dialogs(&ctx);
        self.diff_window(&ctx);
        self.about_window(&ctx);
        self.toasts.show(&ctx);
    }
}

impl App {
    fn error_page(&mut self, ui: &mut Ui) {
        widgets::page_header(ui, icons::WARNING, "无法载入配置", "先解决下面的问题，才能开始编辑");
        widgets::card(ui, |ui| {
            ui.set_width(ui.available_width());
            if let Some(err) = &self.load_error {
                ui.label(RichText::new(err).monospace().size(12.5).color(theme::DANGER));
            }
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if widgets::ghost_button(ui, "选择 CODEX_HOME 文件夹").clicked() {
                    if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                        self.load(folder);
                    }
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
                    .inner_margin(Margin::symmetric(18, 12))
                    .stroke(Stroke::new(1.0, theme::BORDER)),
            )
            .show(root, |ui| {
                ui.horizontal(|ui| {
                    widgets::icon_tile(ui, icons::BRAND, 30.0, 18.0, theme::ACCENT);
                    ui.add_space(6.0);
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 1.0;
                        ui.label(RichText::new("Codex 配置助手").size(15.5).strong().color(theme::TEXT));
                        if let Some(doc) = &self.doc {
                            ui.label(
                                RichText::new(doc.config_path.display().to_string())
                                    .size(11.0)
                                    .color(theme::TEXT_MUTED),
                            );
                        }
                    });
                    ui.add_space(14.0);

                    if let Some(doc) = &self.doc {
                        let dirty = doc.dirty();
                        if dirty {
                            widgets::badge(ui, &format!("{} 有未保存的修改", icons::DOT), theme::WARN, theme::WARN_WEAK);
                        } else {
                            widgets::badge(ui, &format!("{} 已保存", icons::CHECK), theme::OK, theme::OK_WEAK);
                        }
                        let issues = self.issues();
                        let errors = issues.iter().filter(|i| i.severity == Severity::Error).count();
                        let warnings = issues.iter().filter(|i| i.severity == Severity::Warning).count();
                        if errors > 0 {
                            widgets::badge(ui, &format!("{} {errors} 个错误", icons::ERROR), theme::DANGER, theme::DANGER_WEAK);
                        }
                        if warnings > 0 {
                            widgets::badge(ui, &format!("{} {warnings} 个警告", icons::WARNING), theme::WARN, theme::WARN_WEAK);
                        }
                        if errors == 0 && warnings == 0 {
                            widgets::badge(ui, &format!("{} 配置检查通过", icons::CHECK), theme::OK, theme::OK_WEAK);
                        }
                    }

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let dirty = self.doc.as_ref().is_some_and(|doc| doc.dirty());
                        ui.add_enabled_ui(dirty, |ui| {
                            if widgets::primary_button(ui, &format!("{} 保存 · Cmd+S", icons::SAVE)).clicked() {
                                self.save();
                            }
                        });
                        ui.add_enabled_ui(dirty, |ui| {
                            if widgets::ghost_button(ui, &format!("{} 放弃修改", icons::REVERT)).clicked() {
                                self.discard();
                            }
                        });
                        if widgets::ghost_button(ui, &format!("{} 对比差异", icons::DIFF)).clicked() {
                            self.show_diff = true;
                        }
                        if widgets::ghost_button(ui, &format!("{} 重启生效", icons::RESTART)).clicked() {
                            self.open_restart_dialog();
                        }
                    });
                });
            });
    }

    fn sidebar(&mut self, root: &mut Ui) {
        egui::Panel::left("sidebar")
            .exact_size(224.0)
            .resizable(false)
            .frame(
                egui::Frame::new()
                    .fill(theme::PANEL)
                    .inner_margin(Margin::symmetric(12, 14))
                    .stroke(Stroke::new(1.0, theme::BORDER)),
            )
            .show(root, |ui| {
                ui.label(
                    RichText::new("导航")
                        .size(10.5)
                        .strong()
                        .color(theme::TEXT_MUTED),
                );
                ui.add_space(6.0);
                let counts = self.sidebar_counts();
                for page in Page::ALL {
                    let selected = self.page == page;
                    if self.nav_item(ui, page, selected, counts.get(&page).copied().flatten()) {
                        self.page = page;
                    }
                    ui.add_space(3.0);
                }

                ui.with_layout(Layout::bottom_up(Align::LEFT), |ui| {
                    ui.add_space(8.0);
                    if widgets::ghost_button(ui, &format!("{} 关于 / 帮助", icons::INFO)).clicked() {
                        self.show_about = true;
                    }
                    ui.add_space(4.0);
                    if let Some(doc) = &self.doc {
                        let home = doc.codex_home.clone();
                        if widgets::ghost_button(ui, &format!("{} 打开配置文件夹", icons::OPEN_FOLDER)).clicked() {
                            let _ = open::that(&home);
                        }
                    }
                    ui.add_space(10.0);
                    ui.separator();
                    if let Some(doc) = &self.doc {
                        ui.label(
                            RichText::new("CODEX_HOME")
                                .size(10.5)
                                .color(theme::TEXT_MUTED),
                        );
                        ui.label(
                            RichText::new(doc.codex_home.display().to_string())
                                .size(10.5)
                                .monospace()
                                .color(theme::TEXT_MUTED),
                        );
                    }
                });
            });
    }

    /// One navigation row: an accent bar on the left when selected, an icon,
    /// the page name and an optional count badge. Returns true when clicked.
    fn nav_item(&self, ui: &mut Ui, page: Page, selected: bool, count: Option<usize>) -> bool {
        let fill = if selected { theme::ACCENT_WEAK } else { Color32::TRANSPARENT };
        let stroke = if selected {
            Stroke::new(1.0, theme::ACCENT.gamma_multiply(0.55))
        } else {
            Stroke::NONE
        };
        let frame = egui::Frame::new()
            .fill(fill)
            .stroke(stroke)
            .corner_radius(CornerRadius::same(10))
            .inner_margin(Margin::symmetric(10, 8));
        let response = frame
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    let icon_color = if selected { theme::ACCENT_HI } else { theme::TEXT_MUTED };
                    ui.label(RichText::new(page.icon()).size(17.0).color(icon_color));
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(page.title())
                            .size(13.5)
                            .strong()
                            .color(if selected { theme::ACCENT_TEXT } else { theme::TEXT_DIM }),
                    );
                    if let Some(count) = count {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let (fg, bg) = if selected {
                                (theme::ACCENT_TEXT, theme::ACCENT.gamma_multiply(0.22))
                            } else {
                                (theme::TEXT_MUTED, theme::CARD_ALT)
                            };
                            widgets::count_pill(ui, count, fg, bg);
                        });
                    }
                });
            })
            .response
            .interact(egui::Sense::click());
        // Accent indicator bar on the very left edge when selected.
        if selected {
            let rect = response.rect;
            let bar = egui::Rect::from_min_max(
                rect.left_top() + egui::vec2(-2.0, 6.0),
                rect.left_bottom() + egui::vec2(1.0, -6.0),
            );
            ui.painter().rect_filled(bar, CornerRadius::same(2), theme::ACCENT);
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
                ui.label(RichText::new("左边红色是磁盘上的旧内容，右边绿色是即将写入的新内容。")
                    .size(12.0)
                    .color(theme::TEXT_DIM));
                ui.add_space(6.0);
                if config_dirty {
                    ui.label(RichText::new(format!("config.toml · {config_path}"))
                        .size(13.0).strong().color(theme::TEXT));
                    diff_view(ui, "diff-config", &config_old, &config_new);
                }
                if catalog_dirty {
                    ui.add_space(10.0);
                    ui.label(RichText::new(format!("模型目录 · {catalog_path}"))
                        .size(13.0).strong().color(theme::TEXT));
                    diff_view(ui, "diff-catalog", &catalog_old, &catalog_new);
                }
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if widgets::primary_button(ui, "确认并保存").clicked() {
                        self.show_diff = false;
                        self.save();
                    }
                    if widgets::ghost_button(ui, "关闭").clicked() {
                        self.show_diff = false;
                    }
                });
            });
        self.show_diff = open;
    }

    fn about_window(&mut self, ctx: &Context) {
        if !self.show_about {
            return;
        }
        let mut open = self.show_about;
        let font_note = self.font_note.clone();
        egui::Window::new("关于 Codex 配置助手")
            .open(&mut open)
            .collapsible(false)
            .default_size([560.0, 460.0])
            .show(ctx, |ui| {
                ui.label(RichText::new("Codex 配置助手 v0.1.0").size(17.0).strong());
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
                        "3. 回到「基础设置」页，把「当前模型」和「当前服务商」设成刚才那两个。",
                        "4. 点右上角「保存」。保存前会自动备份原文件，出问题可以随时还原。",
                    ] {
                        ui.label(RichText::new(tip).size(12.5).color(theme::TEXT_DIM));
                        ui.add_space(2.0);
                    }
                });
                ui.add_space(6.0);
                widgets::kv_row(ui, "中文字体", &font_note);
                widgets::kv_row(ui, "配置文件", "config.toml（TOML，保留你的注释和排版）");
                widgets::kv_row(ui, "模型目录", "model_catalog_json 指向的 JSON 文件");
                ui.add_space(6.0);
                if widgets::ghost_button(ui, "关闭").clicked() {
                    self.show_about = false;
                }
            });
        self.show_about = open;
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
                    if change.tag() != ChangeTag::Equal {
                        ui.label(RichText::new(format!("{prefix}{text}")).monospace().size(11.5).color(color));
                    } else {
                        ui.label(RichText::new(format!("{prefix}{text}")).monospace().size(11.5).color(color));
                    }
                }
            });
        });
}
