//! The in-memory representation of a user's Codex configuration.

pub mod catalog;
pub mod providers;
pub mod schema;
pub mod toml_ext;
pub mod validate;

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use chrono::Local;
use serde_json::Value;
use toml_edit::DocumentMut;

use crate::doc::toml_ext::TomlPathExt;

/// Providers that Codex knows about without any `config.toml` entry.
pub const BUILTIN_PROVIDER_IDS: &[&str] = &["ollama", "lmstudio", "openai", "azure_openai"];

#[derive(Debug, Clone)]
pub struct SaveReport {
    pub written: Vec<PathBuf>,
    pub backups: Vec<PathBuf>,
}

/// One loaded CODEX_HOME: `config.toml` plus the model catalog JSON it points at.
pub struct Document {
    pub codex_home: PathBuf,
    pub config_path: PathBuf,
    pub config: DocumentMut,
    config_on_disk: String,
    pub catalog_path: Option<PathBuf>,
    pub catalog: Option<Value>,
    catalog_on_disk: String,
    /// Canonical baseline for dirty checks; keep the original text for diff/backup.
    catalog_clean_text: String,
    /// Non fatal problems found while loading (missing catalog, bad JSON, ...).
    pub load_notes: Vec<String>,
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
        let codex_home = expand_home(&codex_home);
        let config_path = codex_home.join("config.toml");
        let mut load_notes = Vec::new();

        let config_text = match fs::read_to_string(&config_path) {
            Ok(text) => text,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                load_notes.push(format!(
                    "没有找到 {}，已为你新建一份空白配置（保存后才会写入磁盘）。",
                    config_path.display()
                ));
                String::new()
            }
            Err(err) => bail!("读取 {} 失败: {err}", config_path.display()),
        };

        let config = config_text
            .parse::<DocumentMut>()
            .with_context(|| format!("{} 不是合法的 TOML 文件", config_path.display()))?;

        let mut doc = Self {
            config_path: config_path.clone(),
            config,
            config_on_disk: config_text,
            catalog_path: None,
            catalog: None,
            catalog_on_disk: String::new(),
            catalog_clean_text: String::new(),
            load_notes,
            codex_home: codex_home.clone(),
        };
        doc.reload_catalog();
        Ok(doc)
    }

    /// Re-resolve `model_catalog_json` and (re)load the file it points at.
    pub fn reload_catalog(&mut self) {
        self.catalog_clean_text.clear();
        let raw = self.config.str_at(&["model_catalog_json"]);
        let Some(raw) = raw.filter(|s| !s.trim().is_empty()) else {
            self.catalog_path = None;
            self.catalog = None;
            self.catalog_on_disk = String::new();
            return;
        };
        let path = self.resolve_against_home(&raw);
        match fs::read_to_string(&path) {
            Ok(text) => match serde_json::from_str::<Value>(&text) {
                Ok(value) => {
                    if value.get("models").is_none() {
                        self.load_notes.push(format!(
                            "{} 里没有 \"models\" 数组，模型页会显示为空。",
                            path.display()
                        ));
                    }
                    self.catalog_path = Some(path);
                    self.catalog_clean_text = pretty_json(&value);
                    self.catalog = Some(value);
                    self.catalog_on_disk = text;
                }
                Err(err) => {
                    self.catalog_path = Some(path.clone());
                    self.catalog = None;
                    self.catalog_on_disk = String::new();
                    self.load_notes.push(format!(
                        "模型目录 {} 不是合法的 JSON: {err}",
                        path.display()
                    ));
                }
            },
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                self.catalog_path = Some(path.clone());
                self.catalog = None;
                self.catalog_on_disk = String::new();
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
    }

    pub fn resolve_against_home(&self, raw: &str) -> PathBuf {
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
        self.config.to_string() != self.config_on_disk
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
        &self.config_on_disk
    }

    pub fn catalog_text(&self) -> String {
        match &self.catalog {
            Some(value) => pretty_json(value),
            None => self.catalog_on_disk.clone(),
        }
    }

    pub fn catalog_on_disk(&self) -> &str {
        &self.catalog_on_disk
    }

    /// Backup + atomically write both files (only the dirty ones).
    pub fn save(&mut self) -> Result<SaveReport> {
        let mut report = SaveReport {
            written: Vec::new(),
            backups: Vec::new(),
        };

        if !self.config_dirty() && !self.catalog_dirty() {
            return Ok(report);
        }

        // Validate first: never write something Codex cannot parse.
        let config_text = self.config.to_string();
        config_text
            .parse::<DocumentMut>()
            .context("生成的 config.toml 无法解析")?;

        if let Some(value) = &self.catalog {
            let _ = serde_json::to_vec(value).context("生成的模型目录 JSON 无法序列化")?;
        }

        if self.config_dirty() {
            if let Some(backup) = backup_file(&self.config_path)? {
                report.backups.push(backup);
            }
            atomic_write(&self.config_path, &config_text, 0o600)?;
            self.config_on_disk = config_text;
            report.written.push(self.config_path.clone());
        }

        if self.catalog_dirty()
            && let Some(path) = self.catalog_path.clone()
            && let Some(value) = &self.catalog
        {
            let text = pretty_json(value);
            if let Some(backup) = backup_file(&path)? {
                report.backups.push(backup);
            }
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).ok();
            }
            atomic_write(&path, &text, 0o644)?;
            self.catalog_clean_text.clone_from(&text);
            self.catalog_on_disk = text;
            report.written.push(path);
        }

        prune_backups(&self.codex_home);
        Ok(report)
    }

    /// Replace the whole config.toml text (used by the raw editor).
    pub fn apply_config_text(&mut self, text: &str) -> Result<()> {
        let parsed = text
            .parse::<DocumentMut>()
            .map_err(|err| anyhow::anyhow!("{err}"))?;
        self.config = parsed;
        Ok(())
    }

    pub fn apply_catalog_text(&mut self, text: &str) -> Result<()> {
        let parsed: Value = serde_json::from_str(text).map_err(|err| anyhow::anyhow!("{err}"))?;
        if !parsed.get("models").is_some_and(Value::is_array) {
            bail!("模型目录需要包含 models 数组");
        }
        self.catalog = Some(parsed);
        Ok(())
    }

    /// Stage a new, empty catalog. No file is created until the explicit save.
    pub fn create_catalog(&mut self, filename: &str) -> Result<()> {
        let path = self.resolve_against_home(filename);
        if path.exists() {
            bail!("{} 已经存在，请换一个名字", path.display());
        }
        self.config
            .set_value_at(&["model_catalog_json"], toml_edit::Value::from(filename));
        self.catalog_path = Some(path);
        self.catalog = Some(serde_json::json!({"models": []}));
        self.catalog_on_disk.clear();
        self.catalog_clean_text.clear();
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
}

pub fn pretty_json(value: &Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
}

pub fn expand_home(path: &Path) -> PathBuf {
    let text = path.to_string_lossy();
    if let Some(rest) = text.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    path.to_path_buf()
}

fn atomic_write(path: &Path, contents: &str, mode: u32) -> Result<()> {
    let parent = path.parent().filter(|p| !p.as_os_str().is_empty());
    if let Some(parent) = parent {
        fs::create_dir_all(parent).with_context(|| format!("无法创建目录 {}", parent.display()))?;
    }
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "config.toml".to_string());
    let tmp = path.with_file_name(format!(".{file_name}.gui-tmp"));
    fs::write(&tmp, contents).with_context(|| format!("写入 {} 失败", tmp.display()))?;
    set_mode(&tmp, mode);
    fs::rename(&tmp, path).with_context(|| format!("替换 {} 失败", path.display()))?;
    Ok(())
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    if let Ok(metadata) = fs::metadata(path) {
        let mut permissions = metadata.permissions();
        permissions.set_mode(mode);
        let _ = fs::set_permissions(path, permissions);
    }
}

#[cfg(not(unix))]
fn set_mode(_path: &Path, _mode: u32) {}

/// Copy `path` next to itself with a timestamp suffix. Returns the backup path.
fn backup_file(path: &Path) -> Result<Option<PathBuf>> {
    if !path.exists() {
        return Ok(None);
    }
    let stamp = Local::now().format("%Y%m%d-%H%M%S").to_string();
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "config.toml".to_string());
    let mut backup = path.with_file_name(format!("{name}.bak-{stamp}"));
    // Avoid clobbering a backup made in the same second.
    let mut counter = 1;
    while backup.exists() {
        backup = path.with_file_name(format!("{name}.bak-{stamp}-{counter}"));
        counter += 1;
    }
    fs::copy(path, &backup).with_context(|| format!("备份 {} 失败", path.display()))?;
    Ok(Some(backup))
}

/// Keep at most 20 GUI backups per config file so the folder does not explode.
fn prune_backups(codex_home: &Path) {
    let Ok(entries) = fs::read_dir(codex_home) else {
        return;
    };
    let mut backups: Vec<(String, PathBuf)> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .map(|name| {
                    let name = name.to_string_lossy();
                    name.contains(".bak-") && !name.ends_with(".gui-tmp")
                })
                .unwrap_or(false)
        })
        .map(|path| (path.to_string_lossy().to_string(), path))
        .collect();
    backups.sort();
    while backups.len() > 20 {
        let (_, oldest) = backups.remove(0);
        let _ = fs::remove_file(oldest);
    }
}
