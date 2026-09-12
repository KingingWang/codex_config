//! Explicit, bounded SSH operations. No remote paths touch local file APIs.

use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::doc::toml_ext::TomlPathExt;
use crate::doc::{Document, SaveReport};

pub const HELPER: &str = include_str!("remote_helper.py");
#[cfg(test)]
#[path = "remote_tests.rs"]
mod tests;
const MAX_OUTPUT: u64 = 32 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SshTarget {
    pub alias: String,
    pub config_file: PathBuf,
    /// Empty means the remote environment's CODEX_HOME or ~/.codex.
    pub home: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Snapshot {
    pub home: String,
    pub user_home: String,
    pub config: Option<String>,
    pub catalog_path: Option<String>,
    pub catalog: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RemoteFile {
    pub path: String,
    pub original: Option<String>,
    pub text: String,
    pub write: bool,
}

/// A literal Host alias, never an SSH option or shell fragment.
pub fn valid_alias(alias: &str) -> bool {
    !alias.is_empty()
        && !alias.starts_with('-')
        && alias
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c))
}

pub fn resolve_path(home: &str, user_home: &str, raw: &str) -> Result<String> {
    if raw.is_empty() || raw.contains('\0') {
        bail!("远程路径不能为空或包含 NUL");
    }
    let full = if raw == "~" {
        user_home.to_owned()
    } else if let Some(rest) = raw.strip_prefix("~/") {
        format!("{user_home}/{rest}")
    } else if raw.starts_with('~') {
        bail!("远程路径仅支持 ~ 或 ~/，不支持 ~user");
    } else if raw.starts_with('/') {
        raw.to_owned()
    } else {
        format!("{home}/{raw}")
    };
    let mut parts = Vec::new();
    for part in full.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            part => parts.push(part),
        }
    }
    Ok(format!("/{}", parts.join("/")))
}

fn ssh_command(target: &SshTarget) -> Result<Command> {
    if !valid_alias(&target.alias) {
        bail!("SSH 别名只能包含字母、数字、点、下划线、连字符，且不能以 - 开头");
    }
    let mut command = Command::new("ssh");
    command.args([
        "-T",
        "-o",
        "BatchMode=yes",
        "-o",
        "StrictHostKeyChecking=yes",
        "-o",
        "ConnectTimeout=10",
        "-o",
        "ConnectionAttempts=1",
        "-o",
        "ServerAliveInterval=10",
        "-o",
        "ServerAliveCountMax=2",
        "-o",
        "ForwardAgent=no",
        "-o",
        "ClearAllForwardings=yes",
        "-o",
        "PermitLocalCommand=no",
        "-o",
        "RemoteCommand=none",
        "-o",
        "ControlMaster=no",
        "-o",
        "ControlPath=none",
        "-o",
        "UpdateHostKeys=no",
        "-o",
        "ForkAfterAuthentication=no",
        "-o",
        "StdinNull=no",
        "-o",
        "SessionType=default",
    ]);
    command
        .arg("-F")
        .arg(&target.config_file)
        .arg(&target.alias);
    // Only a compile-time constant enters the remote shell. User data uses stdin.
    command.arg(format!("python3 -c '{}'", HELPER.replace('\'', "'\"'\"'")));
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    Ok(command)
}

fn request(target: &SshTarget, payload: Value, cancel: &AtomicBool) -> Result<Value> {
    request_command(
        ssh_command(target)?,
        payload,
        cancel,
        Duration::from_secs(90),
    )
}

fn request_command(
    mut command: Command,
    payload: Value,
    cancel: &AtomicBool,
    timeout: Duration,
) -> Result<Value> {
    if cancel.load(Ordering::Relaxed) {
        bail!("SSH 操作已取消");
    }
    let bytes = serde_json::to_vec(&payload)?;
    if bytes.len() > MAX_OUTPUT as usize {
        bail!("SSH 请求超过 32 MiB");
    }
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("无法启动 SSH / 远程助手进程，请确认系统 OpenSSH 已安装")?;
    let mut stdin = child.stdin.take().context("无法打开 SSH 输入")?;
    let stdout = child.stdout.take().context("无法打开 SSH 输出")?;
    let stderr = child.stderr.take().context("无法打开 SSH 错误输出")?;
    let writer = std::thread::spawn(move || stdin.write_all(&bytes));
    // Drain pipes concurrently to avoid deadlock; retain bounded output.
    let reader = |mut pipe: Box<dyn Read + Send>| {
        std::thread::spawn(move || -> std::io::Result<Vec<u8>> {
            let mut data = Vec::new();
            (&mut pipe).take(MAX_OUTPUT + 1).read_to_end(&mut data)?;
            if data.len() as u64 > MAX_OUTPUT {
                return Err(std::io::Error::other("SSH 输出超过 32 MiB"));
            }
            Ok(data)
        })
    };
    let output = reader(Box::new(stdout));
    let errors = reader(Box::new(stderr));
    let started = Instant::now();
    let mut exited = None;
    let status = loop {
        if cancel.load(Ordering::Relaxed) || started.elapsed() > timeout {
            let _ = child.kill();
            let _ = child.wait();
            // Reader threads may still be draining a proxy's inherited pipes;
            // never join them on a timeout.
            bail!("SSH 操作已取消或超过 90 秒。若正在保存，远端结果可能未知，请重新载入核对");
        }
        if let Some(status) = exited {
            if output.is_finished() && errors.is_finished() && writer.is_finished() {
                break status;
            }
            std::thread::sleep(Duration::from_millis(40));
            continue;
        }
        match child.try_wait() {
            Ok(Some(status)) => exited = Some(status),
            Ok(None) => std::thread::sleep(Duration::from_millis(40)),
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error).context("无法等待 SSH 进程");
            }
        }
    };
    let out = output
        .join()
        .map_err(|_| anyhow::anyhow!("SSH 输出线程异常"))??;
    let err = errors
        .join()
        .map_err(|_| anyhow::anyhow!("SSH 错误线程异常"))??;
    let parsed = serde_json::from_slice::<Value>(&out);
    if let Ok(value) = &parsed
        && let Some(error) = value.get("error").and_then(Value::as_str)
    {
        bail!("{error}");
    }
    if !status.success() {
        let detail = String::from_utf8_lossy(&err);
        bail!(
            "SSH 失败（{}）：{}\n请确认终端可用 ssh 别名登录、已确认主机指纹，远端已安装 Python 3。",
            status,
            detail.chars().take(4000).collect::<String>()
        );
    }
    writer
        .join()
        .map_err(|_| anyhow::anyhow!("SSH 输入线程异常"))?
        .context("发送 SSH 请求失败")?;
    let envelope = parsed
        .context("远端没有返回合法 JSON；检查 Python 3 或 shell 启动脚本是否向标准输出打印内容")?;
    envelope
        .get("result")
        .cloned()
        .context("远端响应缺少 result")
}

pub fn load(target: &SshTarget, cancel: &AtomicBool) -> Result<Document> {
    let first: Snapshot = serde_json::from_value(request(
        target,
        json!({"operation":"load", "home":target.home}),
        cancel,
    )?)?;
    let config = first
        .config
        .as_deref()
        .unwrap_or_default()
        .parse::<toml_edit::DocumentMut>()
        .context("远程 config.toml 不是合法的 TOML")?;
    let raw = config
        .str_at(&["model_catalog_json"])
        .filter(|s| !s.trim().is_empty());
    let snapshot = if let Some(raw) = raw {
        serde_json::from_value(request(
            target,
            json!({
                "operation":"load", "home":first.home, "catalog":raw,
                "check_config":true, "expected_config":first.config,
            }),
            cancel,
        )?)?
    } else {
        first
    };
    Document::from_remote(snapshot)
}

pub fn switch_catalog(
    target: &SshTarget,
    mut doc: Document,
    raw: &str,
    cancel: &AtomicBool,
) -> Result<Document> {
    if doc.catalog_dirty() {
        bail!("模型目录还有未保存的修改，请先保存或放弃");
    }
    let snapshot: Snapshot = serde_json::from_value(request(
        target,
        json!({
            "operation":"load", "home":doc.codex_home.to_string_lossy(), "catalog":raw,
            "check_config":true, "expected_config":doc.config_snapshot(),
        }),
        cancel,
    )?)?;
    doc.replace_remote_catalog(raw, snapshot)?;
    Ok(doc)
}

pub fn save(
    target: &SshTarget,
    mut doc: Document,
    cancel: &AtomicBool,
) -> Result<(Document, SaveReport)> {
    let mut files = doc.remote_files()?;
    // Publish catalog first, then the config referencing it.
    files.reverse();
    let value = request(target, json!({"operation":"save", "files":files}), cancel)?;
    #[derive(Deserialize)]
    struct Report {
        written: Vec<PathBuf>,
        backups: Vec<PathBuf>,
    }
    let report: Report = serde_json::from_value(value)?;
    doc.mark_remote_saved();
    Ok((
        doc,
        SaveReport {
            written: report.written,
            backups: report.backups,
        },
    ))
}

pub enum JobResult {
    Loaded(SshTarget, Document),
    Saved(Document, SaveReport),
    Catalog(Document),
}

pub struct Job {
    pub receiver: mpsc::Receiver<Result<JobResult>>,
    pub cancel: Arc<AtomicBool>,
    pub saving: bool,
}

impl Job {
    pub fn spawn(
        ctx: egui::Context,
        saving: bool,
        run: impl FnOnce(&AtomicBool) -> Result<JobResult> + Send + 'static,
    ) -> Self {
        let cancel = Arc::new(AtomicBool::new(false));
        let flag = cancel.clone();
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            let result = run(&flag);
            let _ = sender.send(result);
            ctx.request_repaint();
        });
        Self {
            receiver,
            cancel,
            saving,
        }
    }
}

impl Drop for Job {
    fn drop(&mut self) {
        // Saving is not cancellable: a lost SSH connection cannot undo a commit.
        if !self.saving {
            self.cancel.store(true, Ordering::Relaxed);
        }
    }
}
