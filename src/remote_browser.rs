//! Read-only remote directory selection, pinned to the chosen SSH target.

use egui::Context;

use crate::app::App;
use crate::remote::{self, DirectoryListing, Job, JobResult, PathKind, SshTarget};
use crate::remote_ui::RemoteActivity;
use crate::ui::{theme, widgets};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserPurpose {
    Home,
    Catalog,
}

pub struct RemoteBrowser {
    pub target: SshTarget,
    pub purpose: BrowserPurpose,
    pub path_input: String,
    pub listing: Option<DirectoryListing>,
}

impl App {
    pub fn open_remote_browser(&mut self, purpose: BrowserPurpose) {
        if self.ssh_busy() {
            return;
        }
        let target = match purpose {
            BrowserPurpose::Home => SshTarget {
                alias: self.remote.alias.clone(),
                config_file: crate::doc::expand_home(std::path::Path::new(
                    self.remote.config_file.trim(),
                )),
                home: self.remote.home.trim().to_owned(),
            },
            BrowserPurpose::Catalog => {
                let Some(target) = self.remote.target.clone().filter(|_| self.is_remote()) else {
                    self.toast_error("请先连接 SSH 目标。");
                    return;
                };
                target
            }
        };
        if !remote::valid_alias(&target.alias) {
            self.toast_error("请选择有效的 SSH 别名。");
            return;
        }
        let path = if purpose == BrowserPurpose::Home && target.home.is_empty() {
            "~"
        } else {
            "."
        };
        self.remote.browser = Some(RemoteBrowser {
            target,
            purpose,
            path_input: path.into(),
            listing: None,
        });
        self.start_remote_browse(path.into());
    }

    pub fn start_remote_browse(&mut self, path: String) {
        if self.ssh_busy() {
            return;
        }
        let Some(browser) = &self.remote.browser else {
            return;
        };
        let target = browser.target.clone();
        self.remote.error = None;
        self.remote.activity = RemoteActivity::Browse;
        self.remote.job = Some(Job::spawn(self.remote.ctx.clone(), false, move |cancel| {
            let listing = remote::browse(&target, &path, cancel)?;
            Ok(JobResult::Browsed(target, listing))
        }));
    }

    pub fn select_remote_browser_path(&mut self, path: String) {
        if self.ssh_busy() {
            return;
        }
        let Some(browser) = &self.remote.browser else {
            return;
        };
        let valid = browser
            .listing
            .as_ref()
            .is_some_and(|listing| match browser.purpose {
                BrowserPurpose::Home => listing.path == path,
                BrowserPurpose::Catalog => listing.entries.iter().any(|entry| {
                    entry.path == path
                        && entry.kind == PathKind::File
                        && entry.name.to_lowercase().ends_with(".json")
                }),
            });
        if !valid {
            self.toast_error("请从当前目录中选择可用的目标，不支持符号链接。");
            return;
        }
        match browser.purpose {
            BrowserPurpose::Home => {
                if self.remote.alias != browser.target.alias
                    || crate::doc::expand_home(std::path::Path::new(self.remote.config_file.trim()))
                        != browser.target.config_file
                {
                    self.toast_error("SSH 目标已变化，请重新浏览。");
                    return;
                }
                self.remote.home = path;
            }
            BrowserPurpose::Catalog => {
                if self.remote.target.as_ref() != Some(&browser.target) || !self.is_remote() {
                    self.toast_error("SSH 目标已变化，请重新浏览。");
                    return;
                }
                self.remote.catalog_path.clone_from(&path);
                self.start_remote_catalog(path);
            }
        }
        self.remote.browser = None;
    }

    pub(crate) fn remote_browser_window(&mut self, ctx: &Context) {
        let Some(browser) = &mut self.remote.browser else {
            return;
        };
        let mut open = true;
        let mut closed = false;
        let mut navigate = None;
        let mut selected = None;
        egui::Window::new("浏览远程目录")
            .open(&mut open).collapsible(false).default_width(640.0)
            .show(ctx, |ui| {
                widgets::hint(ui, &format!("SSH · {}（仅浏览，不创建或删除文件）", browser.target.alias));
                widgets::mono_field(ui, "remote-browser-path", &mut browser.path_input, "~/");
                if widgets::ghost_button(ui, "打开路径").clicked() {
                    navigate = Some(browser.path_input.trim().to_owned());
                }
                if let Some(error) = &self.remote.error {
                    widgets::note(ui, error, theme::DANGER);
                }
                if let Some(listing) = &browser.listing {
                    widgets::hint(ui, &format!("当前目录：{}", listing.path));
                    if let Some(parent) = &listing.parent
                        && widgets::ghost_button(ui, "上一级").clicked()
                    {
                        navigate = Some(parent.clone());
                    }
                    egui::ScrollArea::vertical().max_height(350.0).show(ui, |ui| {
                        for entry in &listing.entries {
                            if entry.kind == PathKind::Directory {
                                if widgets::ghost_button(ui, &format!("进入 {}", entry.name)).clicked() {
                                    navigate = Some(entry.path.clone());
                                }
                            } else {
                                let selectable = browser.purpose == BrowserPurpose::Catalog
                                    && entry.kind == PathKind::File
                                    && entry.name.to_lowercase().ends_with(".json");
                                let label = match entry.kind {
                                    PathKind::Symlink => format!("{}（符号链接，不可选）", entry.name),
                                    _ => format!("选择 {}", entry.name),
                                };
                                if ui.add_enabled(selectable, egui::Button::new(label)).clicked() {
                                    selected = Some(entry.path.clone());
                                }
                            }
                        }
                    });
                    if listing.truncated {
                        widgets::note(ui, "目录内容较多，仅显示前 512 项；可输入准确路径继续浏览或在上层窗口手填文件路径。", theme::WARN);
                    }
                    if browser.purpose == BrowserPurpose::Home
                        && widgets::primary_button(ui, "选择此文件夹").clicked()
                    {
                        selected = Some(listing.path.clone());
                    }
                }
                if widgets::ghost_button(ui, "取消浏览").clicked() { closed = true; }
            });
        if !open || closed {
            self.remote.browser = None;
        } else if let Some(path) = navigate {
            self.start_remote_browse(path);
        } else if let Some(path) = selected {
            self.select_remote_browser_path(path);
        }
    }
}
