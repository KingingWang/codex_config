//! Target selection and asynchronous SSH state for the native application.

use std::path::PathBuf;
use std::sync::atomic::Ordering;

use egui::{Context, RichText};

use crate::app::App;
use crate::doc::Document;
use crate::remote::{self, Job, JobResult, SshTarget};
use crate::ui::{theme, widgets};

pub struct RemoteUi {
    pub target: Option<SshTarget>,
    pub open: bool,
    pub use_ssh: bool,
    pub config_file: String,
    pub aliases: Vec<String>,
    pub warnings: Vec<String>,
    pub alias: String,
    pub home: String,
    pub local_home: String,
    pub discard_confirmed: bool,
    pub error: Option<String>,
    pub job: Option<Job>,
    pub catalog_open: bool,
    pub catalog_path: String,
    pub ctx: Context,
}

impl RemoteUi {
    pub fn new(ctx: Context, home: &std::path::Path) -> Self {
        Self {
            target: None,
            open: false,
            use_ssh: false,
            config_file: crate::ssh_config::default_path().display().to_string(),
            aliases: Vec::new(),
            warnings: Vec::new(),
            alias: String::new(),
            home: String::new(),
            local_home: home.display().to_string(),
            discard_confirmed: false,
            error: None,
            job: None,
            catalog_open: false,
            catalog_path: String::new(),
            ctx,
        }
    }

    pub fn refresh_aliases(&mut self) {
        let discovery = crate::ssh_config::discover(&crate::doc::expand_home(
            std::path::Path::new(self.config_file.trim()),
        ));
        self.aliases = discovery.aliases;
        self.warnings = discovery.warnings;
        if !self.aliases.contains(&self.alias) {
            self.alias = self.aliases.first().cloned().unwrap_or_default();
            self.home.clear();
        }
    }
}

impl App {
    pub fn is_remote(&self) -> bool {
        self.doc.as_ref().is_some_and(Document::is_remote)
    }

    pub fn ssh_busy(&self) -> bool {
        self.remote.job.is_some()
    }

    pub fn target_label(&self) -> String {
        self.remote.target.as_ref().map_or_else(
            || {
                if self.is_remote() {
                    "SSH · 目标未绑定".into()
                } else {
                    "本地配置".into()
                }
            },
            |target| format!("SSH · {}", target.alias),
        )
    }

    pub fn open_environment(&mut self) {
        if self.ssh_busy() {
            return;
        }
        self.remote.open = true;
        self.remote.error = None;
        self.remote.discard_confirmed = false;
        self.remote.use_ssh = self.is_remote();
        if let Some(target) = &self.remote.target {
            self.remote.alias.clone_from(&target.alias);
            self.remote.home.clone_from(&target.home);
            self.remote.config_file = target.config_file.display().to_string();
        }
        self.remote.refresh_aliases();
    }

    /// Failed connection leaves the current document and all drafts intact.
    pub fn connect_remote(&mut self, target: SshTarget) {
        if self.ssh_busy() {
            return;
        }
        if self.has_unsaved_changes() && !self.remote.discard_confirmed {
            self.remote.error = Some("请先保存当前修改，或勾选放弃未保存修改。".into());
            return;
        }
        if !remote::valid_alias(&target.alias) {
            self.remote.error = Some("请选择有效的 SSH config 别名。".into());
            return;
        }
        self.remote.error = None;
        self.remote.job = Some(Job::spawn(self.remote.ctx.clone(), false, move |cancel| {
            let doc = remote::load(&target, cancel)?;
            Ok(JobResult::Loaded(target, doc))
        }));
    }

    pub fn start_remote_save(&mut self) {
        if self.ssh_busy() {
            return;
        }
        let (Some(target), Some(doc)) = (self.remote.target.clone(), self.doc.clone()) else {
            return;
        };
        if !doc.dirty() {
            return;
        }
        self.remote.error = None;
        self.remote.job = Some(Job::spawn(self.remote.ctx.clone(), true, move |cancel| {
            let (doc, report) = remote::save(&target, doc, cancel)?;
            Ok(JobResult::Saved(doc, report))
        }));
    }

    pub fn select_catalog_file(&mut self) {
        if self.ssh_busy() {
            return;
        }
        if self.is_remote() {
            self.remote.catalog_path = self
                .doc
                .as_ref()
                .and_then(|doc| doc.catalog_path.as_ref())
                .map(|path| path.display().to_string())
                .unwrap_or_default();
            self.remote.catalog_open = true;
            self.dialog = None;
        } else {
            let home = self
                .doc
                .as_ref()
                .map(|doc| doc.codex_home.clone())
                .unwrap_or_default();
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("JSON 模型目录", &["json"])
                .set_directory(home)
                .pick_file()
            {
                self.set_catalog_path(path);
            }
        }
    }

    pub fn start_remote_catalog(&mut self, path: String) {
        if self.ssh_busy() {
            return;
        }
        if self.has_raw_draft()
            || self.model_input_error.is_some()
            || self.doc.as_ref().is_some_and(Document::catalog_dirty)
        {
            self.toast_error("请先保存或放弃模型目录修改，并应用源文件草稿，再切换目录。");
            return;
        }
        let (Some(target), Some(doc)) = (self.remote.target.clone(), self.doc.clone()) else {
            return;
        };
        self.remote.error = None;
        self.remote.job = Some(Job::spawn(self.remote.ctx.clone(), false, move |cancel| {
            remote::switch_catalog(&target, doc, &path, cancel).map(JobResult::Catalog)
        }));
    }

    pub fn poll_ssh(&mut self) {
        let result = self
            .remote
            .job
            .as_ref()
            .and_then(|job| match job.receiver.try_recv() {
                Ok(result) => Some(result),
                Err(std::sync::mpsc::TryRecvError::Empty) => None,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => Some(Err(anyhow::anyhow!(
                    "SSH 后台任务意外终止；请重新载入核对远程状态"
                ))),
            });
        let Some(mut result) = result else { return };
        if self
            .remote
            .job
            .as_ref()
            .is_some_and(|job| !job.saving && job.cancel.load(Ordering::Relaxed))
        {
            result = Err(anyhow::anyhow!("已取消读取，保留原来的配置和修改。"));
        }
        self.remote.job = None;
        match result {
            Ok(JobResult::Loaded(mut target, doc)) => {
                // Pin the actual resolved directory so a later reconnect cannot
                // silently switch to a different remote CODEX_HOME.
                target.home = doc.codex_home.to_string_lossy().into_owned();
                self.reset_editors();
                self.raw_buffer.clear();
                self.raw_source.clear();
                self.raw_origin.clear();
                self.last_outcome = None;
                self.saved_needs_restart = false;
                self.show_diff = false;
                self.load_error = None;
                self.remote.target = Some(target);
                self.doc = Some(doc);
                self.remote.open = false;
                self.remote.discard_confirmed = false;
                self.toast_info(format!(
                    "已载入 {} 的配置快照；修改后需点击保存。",
                    self.target_label()
                ));
            }
            Ok(JobResult::Saved(doc, report)) => {
                self.doc = Some(doc);
                self.raw_buffer.clear();
                self.raw_source.clear();
                self.saved_needs_restart = true;
                self.toast_info(format!(
                    "已保存到 {}：{} 个文件，{} 份备份。请在远端重启 Codex。",
                    self.target_label(),
                    report.written.len(),
                    report.backups.len()
                ));
            }
            Ok(JobResult::Catalog(doc)) => {
                self.doc = Some(doc);
                self.editing_model = None;
                self.remote.catalog_open = false;
                self.toast_info("已读取远程模型目录；路径修改尚未保存。");
            }
            Err(error) => {
                let message = format!("{error:#}");
                self.remote.error = Some(message.clone());
                self.toast_error(message);
            }
        }
    }

    pub fn environment_windows(&mut self, ctx: &Context) {
        if self.ssh_busy() {
            egui::Window::new("SSH 操作")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(if self.remote.job.as_ref().is_some_and(|job| job.saving) {
                            "正在保存到远端…请等待结果，不要关闭应用。"
                        } else {
                            "正在读取远程配置…当前编辑内容会保留到载入成功。"
                        });
                    });
                    widgets::hint(
                        ui,
                        "连接超时 10 秒，每次 SSH 请求最多 90 秒；读取配置及目录最多两次请求。",
                    );
                    if let Some(job) = &self.remote.job
                        && !job.saving
                        && widgets::ghost_button(ui, "取消读取").clicked()
                    {
                        job.cancel.store(true, Ordering::Relaxed);
                    }
                });
            return;
        }
        if self.remote.open {
            let mut open = true;
            egui::Window::new("配置环境").open(&mut open).collapsible(false)
                .default_size([640.0, 560.0]).min_size([520.0, 480.0])
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .resizable(true).vscroll(true)
                .show(ctx, |ui| {
                    widgets::hint(ui, &format!("当前编辑：{}", self.target_label()));
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut self.remote.use_ssh, false, "本地机器");
                        ui.selectable_value(&mut self.remote.use_ssh, true, "远程 SSH");
                    });
                    ui.add_space(14.0);
                    if self.remote.use_ssh {
                        ui.label(RichText::new("从 SSH config 选择机器").strong());
                        let previous_alias = self.remote.alias.clone();
                        ui.horizontal(|ui| {
                            egui::ComboBox::from_id_salt("ssh-alias")
                                .selected_text(if self.remote.alias.is_empty() { "没有可用别名" } else { &self.remote.alias })
                                .width(340.0).show_ui(ui, |ui| {
                                    for alias in &self.remote.aliases {
                                        ui.selectable_value(&mut self.remote.alias, alias.clone(), alias);
                                    }
                                });
                            if widgets::ghost_button(ui, "刷新别名").clicked() {
                                self.remote.refresh_aliases();
                            }
                        });
                        if self.remote.alias != previous_alias {
                            self.remote.home.clear();
                        }
                        widgets::hint(ui, "读取 Host 的具体别名和 Include 文件；不展开通配 Host，不会自动连接。");
                        ui.add_space(10.0);
                        ui.label("远程 CODEX_HOME（可选）");
                        widgets::mono_field(ui, "ssh-home", &mut self.remote.home, "留空：远端 CODEX_HOME 或 ~/.codex");
                        widgets::hint(ui, "可填写 ~/目录 或绝对路径。留空取 SSH 非交互会话的环境，不一定与终端登录相同。");
                        egui::CollapsingHeader::new("SSH 配置文件").show(ui, |ui| {
                            widgets::mono_field(ui, "ssh-config-file", &mut self.remote.config_file, "~/.ssh/config");
                            widgets::hint(ui, "修改文件位置后点击「刷新别名」。连接会使用该配置中的 User、Port、IdentityFile、ProxyJump 等。");
                            for warning in &self.remote.warnings {
                                widgets::note(ui, warning, theme::WARN);
                            }
                        });
                        if self.remote.aliases.is_empty() {
                            widgets::note(ui, "没有找到可用别名。请检查 SSH 配置文件，在其中添加 Host 别名后刷新。", theme::WARN);
                        }
                        ui.add_space(8.0);
                        widgets::note(ui, "需要系统 OpenSSH、已信任的主机指纹、密钥 / agent 登录；远端需 Linux / macOS 和 Python 3。不存储密码，不自动接受新主机指纹。", theme::TEXT_DIM);
                    } else {
                        ui.label("本地 CODEX_HOME");
                        widgets::mono_field(ui, "environment-local-home", &mut self.remote.local_home, "~/.codex");
                        if widgets::ghost_button(ui, "浏览本地文件夹…").clicked()
                            && let Some(path) = rfd::FileDialog::new().pick_folder()
                        {
                            self.remote.local_home = path.display().to_string();
                        }
                    }
                    let dirty = self.has_unsaved_changes();
                    if dirty {
                        ui.add_space(8.0);
                        ui.checkbox(&mut self.remote.discard_confirmed, "载入成功后，放弃当前未保存的修改");
                    }
                    if let Some(error) = &self.remote.error {
                        widgets::note(ui, error, theme::DANGER);
                    }
                    ui.add_space(12.0);
                    let valid = if self.remote.use_ssh {
                        self.remote.aliases.contains(&self.remote.alias) && remote::valid_alias(&self.remote.alias)
                    } else { !self.remote.local_home.trim().is_empty() };
                    ui.add_enabled_ui(valid && (!dirty || self.remote.discard_confirmed), |ui| {
                        let label = if self.remote.use_ssh { "连接并载入" } else { "载入本地配置" };
                        if widgets::primary_button(ui, label).clicked() {
                            if self.remote.use_ssh {
                                self.connect_remote(SshTarget {
                                    alias: self.remote.alias.clone(),
                                    config_file: crate::doc::expand_home(std::path::Path::new(self.remote.config_file.trim())),
                                    home: self.remote.home.trim().to_owned(),
                                });
                            } else {
                                let home = PathBuf::from(self.remote.local_home.trim());
                                self.load(home);
                                if !self.is_remote() && self.load_error.is_none() && self.remote.error.is_none() {
                                    self.remote.open = false;
                                }
                            }
                        }
                    });
                });
            self.remote.open &= open;
        }
        if self.remote.catalog_open {
            let mut open = true;
            egui::Window::new("选择远程模型目录")
                .open(&mut open)
                .collapsible(false)
                .default_width(560.0)
                .show(ctx, |ui| {
                    widgets::hint(ui, &format!("目标：{}", self.target_label()));
                    widgets::mono_field(
                        ui,
                        "remote-catalog-path",
                        &mut self.remote.catalog_path,
                        "model-catalog.json",
                    );
                    widgets::hint(
                        ui,
                        "相对远程 CODEX_HOME，也支持 ~/ 和绝对路径。不会打开本地文件选择器。",
                    );
                    if let Some(error) = &self.remote.error {
                        widgets::note(ui, error, theme::DANGER);
                    }
                    if widgets::primary_button(ui, "读取远程文件").clicked() {
                        self.start_remote_catalog(self.remote.catalog_path.trim().to_owned());
                    }
                });
            self.remote.catalog_open &= open;
        }
    }
}
