//! Staged local writes with conflict checks and bounded, per-file backups.
//! Each rename is atomic; a multi-file save is not a crash-atomic transaction.

use std::fs::{self, File, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{Context, Result, bail};
use chrono::{Local, NaiveDateTime};

use super::SaveReport;

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

pub(super) struct Change<'a> {
    pub path: &'a Path,
    pub text: &'a str,
    pub original: Option<&'a str>,
}

/// Missing and empty files are different snapshots. Refuse to replace symlinks.
pub(super) fn check_unchanged(path: &Path, original: Option<&str>) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_symlink() => {
            bail!(
                "{} 是符号链接，为避免替换链接，请直接打开目标文件所在的配置目录",
                path.display()
            );
        }
        Ok(meta) if !meta.is_file() => bail!("{} 不是普通文件", path.display()),
        Ok(_) => {}
        Err(err) if err.kind() == ErrorKind::NotFound => {}
        Err(err) => return Err(err).with_context(|| format!("检查 {} 失败", path.display())),
    }
    let current = match fs::read_to_string(path) {
        Ok(text) => Some(text),
        Err(err) if err.kind() == ErrorKind::NotFound => None,
        Err(err) => return Err(err).with_context(|| format!("读取 {} 失败", path.display())),
    };
    if current.as_deref() != original {
        bail!(
            "{} 已被其他程序修改、创建或删除，未覆盖磁盘内容。请先复制保留当前修改，再重新载入配置",
            path.display()
        );
    }
    Ok(())
}

struct StagedFile {
    temporary: PathBuf,
}

impl StagedFile {
    fn new(path: &Path, text: &str) -> Result<Self> {
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            fs::create_dir_all(parent)
                .with_context(|| format!("无法创建目录 {}", parent.display()))?;
        }
        let name = path
            .file_name()
            .context("文件路径缺少文件名")?
            .to_string_lossy();
        for _ in 0..100 {
            let sequence = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
            let temporary =
                path.with_file_name(format!(".{name}.gui-tmp-{}-{sequence}", std::process::id()));
            match create_private(&temporary) {
                Ok(mut file) => {
                    let staged = Self { temporary };
                    file.write_all(text.as_bytes())
                        .with_context(|| format!("写入 {} 失败", staged.temporary.display()))?;
                    file.sync_all().context("同步临时文件失败")?;
                    return Ok(staged);
                }
                Err(err) if err.kind() == ErrorKind::AlreadyExists => continue,
                Err(err) => {
                    return Err(err).with_context(|| format!("创建 {} 失败", temporary.display()));
                }
            }
        }
        bail!("无法为 {} 创建唯一临时文件", path.display())
    }

    fn commit(&self, path: &Path) -> Result<()> {
        fs::rename(&self.temporary, path).with_context(|| format!("替换 {} 失败", path.display()))
    }
}

impl Drop for StagedFile {
    fn drop(&mut self) {
        // This path was exclusively created by this instance.
        let _ = fs::remove_file(&self.temporary);
    }
}

fn create_private(path: &Path) -> std::io::Result<File> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)
}

pub(super) fn save_changes(changes: &[Change<'_>]) -> Result<SaveReport> {
    save_changes_with(changes, StagedFile::commit)
}

fn save_changes_with(
    changes: &[Change<'_>],
    commit: impl Fn(&StagedFile, &Path) -> Result<()>,
) -> Result<SaveReport> {
    for change in changes {
        check_unchanged(change.path, change.original)?;
    }
    // Finish all fallible staging and backups before changing any live file.
    let staged = changes
        .iter()
        .map(|change| StagedFile::new(change.path, change.text))
        .collect::<Result<Vec<_>>>()?;
    let mut report = SaveReport {
        written: Vec::new(),
        backups: Vec::new(),
    };
    for change in changes {
        if let Some(original) = change.original {
            report.backups.push(backup_file(change.path, original)?);
        }
    }
    for change in changes {
        check_unchanged(change.path, change.original)?;
    }
    for (index, (change, staged)) in changes.iter().zip(&staged).enumerate() {
        if let Err(err) =
            check_unchanged(change.path, change.original).and_then(|()| commit(staged, change.path))
        {
            let mut rollback_errors = Vec::new();
            for previous in changes[..index].iter().rev() {
                let restored =
                    check_unchanged(previous.path, Some(previous.text)).and_then(
                        |()| match previous.original {
                            Some(text) => {
                                StagedFile::new(previous.path, text)?.commit(previous.path)
                            }
                            None => {
                                fs::remove_file(previous.path).context("移除未完成保存的新文件失败")
                            }
                        },
                    );
                if let Err(rollback) = restored {
                    rollback_errors.push(format!("{rollback:#}"));
                }
            }
            if rollback_errors.is_empty() {
                return Err(err).context("保存未完成；已写入的文件已回滚，编辑内容仍保留");
            }
            bail!(
                "{err:#}；部分回滚失败：{}。请保留当前编辑并检查备份：{:?}",
                rollback_errors.join("；"),
                report.backups
            );
        }
        report.written.push(change.path.to_path_buf());
    }
    for change in changes {
        prune_backups(change.path);
    }
    Ok(report)
}

fn backup_file(path: &Path, text: &str) -> Result<PathBuf> {
    let now = Local::now();
    let stamp = now.format("%Y%m%d-%H%M%S");
    let name = path
        .file_name()
        .context("备份路径缺少文件名")?
        .to_string_lossy();
    for offset in 0..10_000 {
        // Never reuse the smallest deleted counter within the same second:
        // retention would mistake that brand-new backup for the oldest one.
        let counter = now.timestamp_subsec_nanos() + offset;
        let backup = path.with_file_name(format!("{name}.bak-{stamp}-{counter:09}"));
        match create_private(&backup) {
            Ok(mut file) => {
                if let Err(err) = file
                    .write_all(text.as_bytes())
                    .and_then(|()| file.sync_all())
                {
                    drop(file);
                    let _ = fs::remove_file(&backup);
                    return Err(err).with_context(|| format!("备份 {} 失败", path.display()));
                }
                return Ok(backup);
            }
            Err(err) if err.kind() == ErrorKind::AlreadyExists => continue,
            Err(err) => return Err(err).with_context(|| format!("备份 {} 失败", path.display())),
        }
    }
    bail!("{} 的备份文件名冲突过多", path.display())
}

/// Only timestamped backups belonging to this exact file are eligible.
fn prune_backups(path: &Path) {
    let Some(name) = path.file_name() else { return };
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let Ok(entries) = fs::read_dir(parent) else {
        return;
    };
    let prefix = format!("{}.bak-", name.to_string_lossy());
    let mut backups = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            if !entry.file_type().ok()?.is_file() {
                return None;
            }
            let name = entry.file_name();
            let suffix = name.to_str()?.strip_prefix(&prefix)?;
            let stamp = suffix.get(..15)?;
            NaiveDateTime::parse_from_str(stamp, "%Y%m%d-%H%M%S").ok()?;
            let counter = match suffix.get(15..)? {
                "" => 0,
                tail => tail.strip_prefix('-')?.parse::<u32>().ok()?,
            };
            Some((stamp.to_owned(), counter, entry.path()))
        })
        .collect::<Vec<_>>();
    backups.sort_by(|a, b| (&a.0, a.1).cmp(&(&b.0, b.1)));
    for (_, _, backup) in backups.iter().take(backups.len().saturating_sub(20)) {
        let _ = fs::remove_file(backup);
    }
}

#[cfg(test)]
#[path = "persistence_tests.rs"]
mod tests;
