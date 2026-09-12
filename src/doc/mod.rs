//! The in-memory representation of a user's Codex configuration.

pub mod catalog;
mod persistence;
pub mod providers;
pub mod schema;
pub mod toml_ext;
pub mod validate;

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde_json::Value;
use toml_edit::DocumentMut;

use crate::doc::toml_ext::TomlPathExt;

/// Providers built into the supported KingingWang/codex fork.
/// OpenAI and Azure require explicit provider definitions in this fork.
pub const BUILTIN_PROVIDER_IDS: &[&str] = &["ollama", "lmstudio"];

#[derive(Debug, Clone)]
pub struct SaveReport {
    pub written: Vec<PathBuf>,
    pub backups: Vec<PathBuf>,
}

/// One loaded CODEX_HOME: `config.toml` plus the model catalog JSON it points at.
#[derive(Clone)]
pub struct Document {
    pub codex_home: PathBuf,
    pub config_path: PathBuf,
    pub config: DocumentMut,
    config_on_disk: Option<String>,
    pub catalog_path: Option<PathBuf>,
    pub catalog: Option<Value>,
    catalog_on_disk: Option<String>,
    /// Canonical baseline for dirty checks; keep the original text for diff/backup.
    catalog_clean_text: String,
    /// Non fatal problems found while loading (missing catalog, bad JSON, ...).
    pub load_notes: Vec<String>,
    catalog_load_notes: Vec<String>,
    /// Some only for SSH snapshots; never resolve `~` against the local user.
    remote_user_home: Option<String>,
}

impl Document {
    /// `$CODEX_HOME` if set, otherwise `~/.codex`.
    pub fn default_home() -> PathBuf {
        if let Ok(custom) = std::env::var("CODEX_HOME")
            && !custom.trim().is_empty()
        {
            return expand_home(Path::new(custom.trim()));
        }
        dirs::home_dir()
            .map(|home| home.join(".codex"))
            .unwrap_or_else(|| PathBuf::from(".codex"))
    }

    pub fn load(codex_home: PathBuf) -> Result<Self> {
        let codex_home =
            std::path::absolute(expand_home(&codex_home)).context("无法解析配置目录的绝对路径")?;
        let config_path = codex_home.join("config.toml");
        let mut load_notes = Vec::new();

        let config_text = match fs::read_to_string(&config_path) {
            Ok(text) => Some(text),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                load_notes.push(format!(
                    "没有找到 {}，已为你新建一份空白配置（保存后才会写入磁盘）。",
                    config_path.display()
                ));
                None
            }
            Err(err) => bail!("读取 {} 失败: {err}", config_path.display()),
        };

        let config = config_text
            .as_deref()
            .unwrap_or_default()
            .parse::<DocumentMut>()
            .with_context(|| format!("{} 不是合法的 TOML 文件", config_path.display()))?;

        let mut doc = Self {
            config_path: config_path.clone(),
            config,
            config_on_disk: config_text,
            catalog_path: None,
            catalog: None,
            catalog_on_disk: None,
            catalog_clean_text: String::new(),
            load_notes,
            catalog_load_notes: Vec::new(),
            codex_home: codex_home.clone(),
            remote_user_home: None,
        };
        doc.reload_catalog();
        Ok(doc)
    }

    /// Re-resolve `model_catalog_json` and (re)load the file it points at.
    pub fn reload_catalog(&mut self) {
        if self.is_remote() {
            return; // Remote reads are explicit background SSH operations.
        }
        // Keep configuration notes, but remove stale catalog read errors.
        self.load_notes
            .retain(|note| !self.catalog_load_notes.contains(note));
        self.catalog_load_notes.clear();
        let previous_notes = self.load_notes.len();
        self.catalog_clean_text.clear();
        self.catalog_on_disk = None;
        self.catalog = None;
        let raw = self.config.str_at(&["model_catalog_json"]);
        let Some(raw) = raw.filter(|s| !s.trim().is_empty()) else {
            self.catalog_path = None;
            self.catalog = None;
            return;
        };
        let path = self.resolve_against_home(&raw);
        match fs::read_to_string(&path) {
            Ok(text) => {
                self.catalog_on_disk = Some(text.clone());
                match serde_json::from_str::<Value>(&text) {
                    Ok(value) => {
                        if !value.get("models").is_some_and(Value::is_array) {
                            self.load_notes.push(format!(
                                "{} 里没有 \"models\" 数组，模型页会显示为空。",
                                path.display()
                            ));
                        }
                        self.catalog_path = Some(path);
                        self.catalog_clean_text = pretty_json(&value);
                        self.catalog = Some(value);
                    }
                    Err(err) => {
                        self.catalog_path = Some(path.clone());
                        self.catalog = None;
                        self.load_notes.push(format!(
                            "模型目录 {} 不是合法的 JSON: {err}",
                            path.display()
                        ));
                    }
                }
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                self.catalog_path = Some(path.clone());
                self.catalog = None;
                self.load_notes.push(format!(
                    "模型目录文件 {} 不存在。你可以在「模型」页新建一个。",
                    path.display()
                ));
            }
            Err(err) => {
                self.catalog_path = Some(path.clone());
                self.catalog = None;
                self.load_notes
                    .push(format!("读取模型目录 {} 失败: {err}", path.display()));
            }
        }
        self.catalog_load_notes = self.load_notes[previous_notes..].to_vec();
    }

    pub fn resolve_against_home(&self, raw: &str) -> PathBuf {
        if let Some(user_home) = &self.remote_user_home {
            return PathBuf::from(
                crate::remote::resolve_path(&self.codex_home.to_string_lossy(), user_home, raw)
                    .unwrap_or_else(|_| raw.to_owned()),
            );
        }
        let expanded = expand_home(Path::new(raw));
        if expanded.is_absolute() {
            expanded
        } else {
            self.codex_home.join(expanded)
        }
    }

    pub fn dirty(&self) -> bool {
        self.config_dirty() || self.catalog_dirty()
    }

    pub fn config_dirty(&self) -> bool {
        self.config.to_string() != self.config_on_disk()
    }

    pub fn catalog_dirty(&self) -> bool {
        match (&self.catalog, self.catalog_path.is_some()) {
            (Some(_), true) => self.catalog_text() != self.catalog_clean_text,
            _ => false,
        }
    }

    pub fn config_text(&self) -> String {
        self.config.to_string()
    }

    pub fn config_on_disk(&self) -> &str {
        self.config_on_disk.as_deref().unwrap_or_default()
    }

    pub fn config_snapshot(&self) -> Option<&str> {
        self.config_on_disk.as_deref()
    }

    pub fn catalog_text(&self) -> String {
        match &self.catalog {
            Some(value) => pretty_json(value),
            None => self.catalog_on_disk().to_owned(),
        }
    }

    pub fn catalog_on_disk(&self) -> &str {
        self.catalog_on_disk.as_deref().unwrap_or_default()
    }

    /// Backup + atomically write both files (only the dirty ones).
    pub fn save(&mut self) -> Result<SaveReport> {
        if self.is_remote() {
            bail!("远程文档只能通过 SSH 保存，禁止写入本地文件");
        }
        if !self.config_dirty() && !self.catalog_dirty() {
            return Ok(SaveReport {
                written: Vec::new(),
                backups: Vec::new(),
            });
        }

        let desired_path = self
            .config
            .str_at(&["model_catalog_json"])
            .filter(|s| !s.trim().is_empty())
            .map(|s| self.resolve_against_home(&s));
        if desired_path != self.catalog_path {
            bail!("模型目录路径与编辑内容不同步，请先应用源文件或重新选择模型目录");
        }
        if let Some(path) = &self.catalog_path
            && (path == &self.config_path
                || fs::canonicalize(path)
                    .ok()
                    .zip(fs::canonicalize(&self.config_path).ok())
                    .is_some_and(|(a, b)| a == b))
        {
            bail!("模型目录不能与 config.toml 使用同一个文件");
        }
        if let Some(issue) = validate::validate(self)
            .into_iter()
            .find(|issue| issue.severity == validate::Severity::Error)
        {
            bail!("{}：{}", issue.title, issue.detail);
        }

        let config_text = self.config.to_string();
        config_text
            .parse::<DocumentMut>()
            .context("生成的 config.toml 无法解析")?;

        let catalog_text = self.catalog_text();
        // Check even unchanged companion files: both belong to the loaded snapshot.
        persistence::check_unchanged(&self.config_path, self.config_on_disk.as_deref())?;
        if let Some(path) = &self.catalog_path {
            persistence::check_unchanged(path, self.catalog_on_disk.as_deref())?;
        }
        let mut changes = Vec::new();
        // Publish the catalog before the config that may reference it.
        if self.catalog_dirty()
            && let Some(path) = &self.catalog_path
        {
            changes.push(persistence::Change {
                path,
                text: &catalog_text,
                original: self.catalog_on_disk.as_deref(),
            });
        }
        if self.config_dirty() {
            changes.push(persistence::Change {
                path: &self.config_path,
                text: &config_text,
                original: self.config_on_disk.as_deref(),
            });
        }
        let report = persistence::save_changes(&changes)?;
        if report.written.contains(&self.config_path) {
            self.config_on_disk = Some(config_text);
        }
        if self
            .catalog_path
            .as_ref()
            .is_some_and(|path| report.written.contains(path))
        {
            self.catalog_clean_text.clone_from(&catalog_text);
            self.catalog_on_disk = Some(catalog_text);
        }
        self.load_notes.clear();
        self.catalog_load_notes.clear();
        Ok(report)
    }

    /// Replace the whole config.toml text (used by the raw editor).
    pub fn apply_config_text(&mut self, text: &str) -> Result<()> {
        let parsed = text
            .parse::<DocumentMut>()
            .map_err(|err| anyhow::anyhow!("{err}"))?;
        let next_path = parsed
            .str_at(&["model_catalog_json"])
            .filter(|s| !s.trim().is_empty())
            .map(|s| self.resolve_against_home(&s));
        let path_changed = next_path != self.catalog_path;
        if path_changed && self.is_remote() {
            bail!("请使用「文件与配置目录 → 选择已有文件」切换远程模型目录，再编辑源文件");
        }
        if path_changed && self.catalog_dirty() {
            bail!("模型目录还有未保存的修改，请先保存或放弃修改，再切换目录");
        }
        self.config = parsed;
        if path_changed {
            self.reload_catalog();
        }
        Ok(())
    }

    pub fn apply_catalog_text(&mut self, text: &str) -> Result<()> {
        if self.catalog_path.is_none() {
            bail!("请先创建或选择模型目录文件，再应用 JSON");
        }
        let parsed: Value = serde_json::from_str(text).map_err(|err| anyhow::anyhow!("{err}"))?;
        if !parsed.get("models").is_some_and(Value::is_array) {
            bail!("模型目录需要包含 models 数组");
        }
        self.catalog = Some(parsed);
        self.load_notes
            .retain(|note| !self.catalog_load_notes.contains(note));
        self.catalog_load_notes.clear();
        Ok(())
    }

    /// Stage a new, empty catalog. No file is created until the explicit save.
    pub fn create_catalog(&mut self, filename: &str) -> Result<()> {
        if self.catalog_dirty() {
            bail!("模型目录还有未保存的修改，请先保存或放弃修改，再创建目录");
        }
        if filename.trim().is_empty() {
            bail!("模型目录文件名不能为空");
        }
        let path = self.resolve_against_home(filename);
        if self.is_remote() {
            crate::remote::resolve_path(
                &self.codex_home.to_string_lossy(),
                self.remote_user_home.as_deref().unwrap_or_default(),
                filename,
            )?;
        }
        if path == self.config_path
            || (self.is_remote()
                && self.catalog_path.as_ref() == Some(&path)
                && self.catalog_on_disk.is_some())
            || (!self.is_remote() && fs::symlink_metadata(&path).is_ok())
        {
            bail!("{} 已经存在，请换一个名字", path.display());
        }
        self.config
            .set_value_at(&["model_catalog_json"], toml_edit::Value::from(filename));
        self.catalog_path = Some(path);
        self.catalog = Some(serde_json::json!({"models": []}));
        self.catalog_on_disk = None;
        self.catalog_clean_text.clear();
        self.load_notes
            .retain(|note| !self.catalog_load_notes.contains(note));
        self.catalog_load_notes.clear();
        Ok(())
    }

    // Convenience accessors used by several pages ---------------------------

    pub fn provider_ids(&self) -> Vec<String> {
        self.config.keys_at(&["model_providers"])
    }

    /// Provider ids including the ones Codex provides out of the box.
    pub fn selectable_provider_ids(&self) -> Vec<String> {
        let mut ids = self.provider_ids();
        for builtin in BUILTIN_PROVIDER_IDS {
            if !ids.iter().any(|id| id == builtin) {
                ids.push((*builtin).to_string());
            }
        }
        ids
    }

    pub fn provider_display(&self, id: &str) -> String {
        let name = self
            .config
            .str_at(&["model_providers", id, "name"])
            .unwrap_or_default();
        if name.is_empty() || name == id {
            id.to_string()
        } else {
            format!("{name} · {id}")
        }
    }

    pub fn profile_ids(&self) -> Vec<String> {
        self.config.keys_at(&["profiles"])
    }

    pub fn is_remote(&self) -> bool {
        self.remote_user_home.is_some()
    }

    /// Build an in-memory remote document without any local filesystem access.
    pub fn from_remote(snapshot: crate::remote::Snapshot) -> Result<Self> {
        if !snapshot.home.starts_with('/') || !snapshot.user_home.starts_with('/') {
            bail!("远程主机必须使用 POSIX 绝对路径（Linux / macOS）");
        }
        let config = snapshot
            .config
            .as_deref()
            .unwrap_or_default()
            .parse::<DocumentMut>()
            .context("远程 config.toml 不是合法 TOML")?;
        let mut doc = Self {
            config_path: PathBuf::from(format!(
                "{}/config.toml",
                snapshot.home.trim_end_matches('/')
            )),
            codex_home: PathBuf::from(&snapshot.home),
            config,
            config_on_disk: snapshot.config.clone(),
            catalog_path: None,
            catalog: None,
            catalog_on_disk: None,
            catalog_clean_text: String::new(),
            load_notes: Vec::new(),
            catalog_load_notes: Vec::new(),
            remote_user_home: Some(snapshot.user_home.clone()),
        };
        if snapshot.config.is_none() {
            doc.load_notes
                .push("远端没有 config.toml，保存后才会创建。".into());
        }
        doc.hydrate_remote_catalog(&snapshot)?;
        Ok(doc)
    }

    fn hydrate_remote_catalog(&mut self, snapshot: &crate::remote::Snapshot) -> Result<()> {
        let expected = self
            .config
            .str_at(&["model_catalog_json"])
            .filter(|s| !s.trim().is_empty())
            .map(|raw| crate::remote::resolve_path(&snapshot.home, &snapshot.user_home, &raw))
            .transpose()?;
        if expected != snapshot.catalog_path {
            bail!("远程模型目录快照与 config.toml 不匹配");
        }
        self.load_notes
            .retain(|note| !self.catalog_load_notes.contains(note));
        self.catalog_load_notes.clear();
        self.catalog_path = snapshot.catalog_path.as_ref().map(PathBuf::from);
        self.catalog_on_disk.clone_from(&snapshot.catalog);
        self.catalog = None;
        self.catalog_clean_text.clear();
        if let Some(text) = &snapshot.catalog {
            match serde_json::from_str::<Value>(text) {
                Ok(value) => {
                    if !value.get("models").is_some_and(Value::is_array) {
                        self.catalog_load_notes
                            .push("远程模型目录缺少 models 数组，请在源文件页修复。".into());
                    }
                    self.catalog_clean_text = pretty_json(&value);
                    self.catalog = Some(value);
                }
                Err(error) => self
                    .catalog_load_notes
                    .push(format!("远程模型目录不是合法 JSON：{error}")),
            }
        } else if self.catalog_path.is_some() {
            self.catalog_load_notes
                .push("远程模型目录不存在。可新建目录，保存时会检查是否被其他程序创建。".into());
        }
        self.load_notes.extend(self.catalog_load_notes.clone());
        Ok(())
    }

    pub fn replace_remote_catalog(
        &mut self,
        raw: &str,
        snapshot: crate::remote::Snapshot,
    ) -> Result<()> {
        if !self.is_remote() || self.catalog_dirty() {
            bail!("只能为没有未保存模型修改的远程文档切换目录");
        }
        let mut next = self.clone();
        if raw.trim().is_empty() {
            next.config.remove_at(&["model_catalog_json"]);
        } else {
            next.config
                .set_value_at(&["model_catalog_json"], toml_edit::Value::from(raw));
        }
        next.hydrate_remote_catalog(&snapshot)?;
        *self = next;
        Ok(())
    }

    /// Config first in the snapshot; the transport publishes catalog first.
    pub fn remote_files(&self) -> Result<Vec<crate::remote::RemoteFile>> {
        if !self.is_remote() {
            bail!("本地文档不能作为 SSH 保存请求");
        }
        let expected = self
            .config
            .str_at(&["model_catalog_json"])
            .filter(|s| !s.trim().is_empty())
            .map(|s| {
                crate::remote::resolve_path(
                    &self.codex_home.to_string_lossy(),
                    self.remote_user_home.as_deref().unwrap_or_default(),
                    &s,
                )
                .map(PathBuf::from)
            })
            .transpose()?;
        if expected != self.catalog_path || self.catalog_path.as_ref() == Some(&self.config_path) {
            bail!("远程模型目录路径不同步或与配置文件冲突");
        }
        if let Some(issue) = validate::validate(self)
            .into_iter()
            .find(|issue| issue.severity == validate::Severity::Error)
        {
            bail!("{}：{}", issue.title, issue.detail);
        }
        let mut files = vec![crate::remote::RemoteFile {
            path: self.config_path.to_string_lossy().into_owned(),
            original: self.config_on_disk.clone(),
            text: self.config_text(),
            write: self.config_dirty(),
        }];
        if let Some(path) = &self.catalog_path {
            files.push(crate::remote::RemoteFile {
                path: path.to_string_lossy().into_owned(),
                original: self.catalog_on_disk.clone(),
                text: self.catalog_text(),
                write: self.catalog_dirty(),
            });
        }
        Ok(files)
    }

    pub fn mark_remote_saved(&mut self) {
        if self.config_dirty() {
            self.config_on_disk = Some(self.config_text());
        }
        if self.catalog_dirty() {
            let text = self.catalog_text();
            self.catalog_clean_text.clone_from(&text);
            self.catalog_on_disk = Some(text);
        }
        self.load_notes.clear();
        self.catalog_load_notes.clear();
    }
}

pub fn pretty_json(value: &Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
}

pub fn expand_home(path: &Path) -> PathBuf {
    let text = path.to_string_lossy();
    if text == "~"
        && let Some(home) = dirs::home_dir()
    {
        return home;
    }
    if let Some(rest) = text.strip_prefix("~/")
        && let Some(home) = dirs::home_dir()
    {
        return home.join(rest);
    }
    path.to_path_buf()
}
