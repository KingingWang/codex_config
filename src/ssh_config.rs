//! Static SSH alias discovery. Never runs `ssh -G` or evaluates `Match exec`.

use std::collections::BTreeSet;
use std::io::Read;
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Default)]
pub struct Discovery {
    pub aliases: Vec<String>,
    pub warnings: Vec<String>,
}

pub fn default_path() -> PathBuf {
    dirs::home_dir().unwrap_or_default().join(".ssh/config")
}

pub fn discover(path: &Path) -> Discovery {
    discover_with_home(path, &dirs::home_dir().unwrap_or_default())
}

/// Explicit home makes Include resolution testable without personal files.
pub fn discover_with_home(path: &Path, user_home: &Path) -> Discovery {
    let mut reader = Reader {
        user_home,
        aliases: BTreeSet::new(),
        seen: BTreeSet::new(),
        bytes_left: 4 * 1024 * 1024,
        warnings: Vec::new(),
    };
    reader.read(path, 0);
    Discovery {
        aliases: reader.aliases.into_iter().collect(),
        warnings: reader.warnings,
    }
}

struct Reader<'a> {
    user_home: &'a Path,
    aliases: BTreeSet<String>,
    seen: BTreeSet<PathBuf>,
    bytes_left: usize,
    warnings: Vec<String>,
}

impl Reader<'_> {
    fn read(&mut self, path: &Path, depth: usize) {
        if depth > 16 || self.seen.len() >= 128 || self.bytes_left == 0 {
            self.warnings
                .push("SSH Include 超过深度 / 文件数 / 4 MiB 限制，已停止展开。".into());
            return;
        }
        let canonical = match std::fs::canonicalize(path) {
            Ok(path) => path,
            Err(error) => {
                self.warnings
                    .push(format!("无法读取 {}：{error}", path.display()));
                return;
            }
        };
        if !self.seen.insert(canonical) {
            return;
        }
        let read = || -> std::io::Result<String> {
            if !std::fs::metadata(path)?.is_file() {
                return Err(std::io::Error::other("SSH 配置不是普通文件"));
            }
            let file = std::fs::File::open(path)?;
            let mut text = String::new();
            file.take(self.bytes_left as u64 + 1)
                .read_to_string(&mut text)?;
            Ok(text)
        };
        let text = match read() {
            Ok(text) if text.len() <= self.bytes_left => text,
            Ok(_) => {
                self.bytes_left = 0;
                self.warnings.push("SSH 配置超过 4 MiB 读取限制。".into());
                return;
            }
            Err(error) => {
                self.warnings
                    .push(format!("无法读取 {}：{error}", path.display()));
                return;
            }
        };
        self.bytes_left -= text.len();
        for line in text.lines() {
            let line = line.trim();
            if line.starts_with('#') || line.is_empty() {
                continue;
            }
            let boundary = line
                .find(|c: char| c.is_whitespace() || c == '=')
                .unwrap_or(line.len());
            let key = &line[..boundary];
            let args = line[boundary..]
                .trim_start()
                .trim_start_matches('=')
                .trim_start();
            if !key.eq_ignore_ascii_case("host") && !key.eq_ignore_ascii_case("include") {
                continue;
            }
            let Some(args) = words(args) else {
                self.warnings
                    .push(format!("{} 中存在未闭合引号，已跳过该行。", path.display()));
                continue;
            };
            if key.eq_ignore_ascii_case("host") {
                for alias in args {
                    if crate::remote::valid_alias(&alias) {
                        self.aliases.insert(alias);
                    }
                }
            } else {
                for include in args {
                    if include.contains(['[', ']', '%', '$'])
                        || (include.starts_with('~') && !include.starts_with("~/"))
                    {
                        self.warnings.push(format!(
                            "暂不展开复杂 Include：{include}（连接时仍交给 SSH 解释）"
                        ));
                        continue;
                    }
                    let expanded = if let Some(rest) = include.strip_prefix("~/") {
                        self.user_home.join(rest)
                    } else if Path::new(&include).is_absolute() {
                        PathBuf::from(include)
                    } else {
                        // OpenSSH user config Includes are relative to ~/.ssh,
                        // not the including file, including when using -F.
                        self.user_home.join(".ssh").join(include)
                    };
                    match expand_glob(&expanded) {
                        Ok(paths) => {
                            for path in paths {
                                self.read(&path, depth + 1);
                            }
                        }
                        Err(error) => self.warnings.push(error),
                    }
                }
            }
        }
    }
}

fn words(text: &str) -> Option<Vec<String>> {
    let mut result = Vec::new();
    let mut word = String::new();
    let mut quoted = false;
    let mut escaped = false;
    for c in text.chars() {
        if escaped {
            word.push(c);
            escaped = false;
        } else if c == '\\' {
            escaped = true;
        } else if c == '"' {
            quoted = !quoted;
        } else if c == '#' && !quoted {
            break;
        } else if c.is_whitespace() && !quoted {
            if !word.is_empty() {
                result.push(std::mem::take(&mut word));
            }
        } else {
            word.push(c);
        }
    }
    if quoted || escaped {
        return None;
    }
    if !word.is_empty() {
        result.push(word);
    }
    Some(result)
}

fn glob_matches(pattern: &str, name: &str) -> bool {
    let pattern: Vec<char> = pattern.chars().collect();
    let name: Vec<char> = name.chars().collect();
    let (mut p, mut n, mut star, mut checkpoint) = (0, 0, None, 0);
    while n < name.len() {
        if p < pattern.len() && (pattern[p] == '?' || pattern[p] == name[n]) {
            p += 1;
            n += 1;
        } else if p < pattern.len() && pattern[p] == '*' {
            star = Some(p);
            p += 1;
            checkpoint = n;
        } else if let Some(s) = star {
            checkpoint += 1;
            n = checkpoint;
            p = s + 1;
        } else {
            return false;
        }
    }
    while p < pattern.len() && pattern[p] == '*' {
        p += 1;
    }
    p == pattern.len()
}

fn expand_glob(pattern: &Path) -> Result<Vec<PathBuf>, String> {
    let mut paths = vec![PathBuf::new()];
    for component in pattern.components() {
        if let Component::Normal(part) = component
            && part.to_string_lossy().contains(['*', '?'])
        {
            let mut next = Vec::new();
            for parent in paths {
                let entries = match std::fs::read_dir(&parent) {
                    Ok(entries) => entries,
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                    Err(error) => return Err(format!("读取 Include 目录失败：{error}")),
                };
                for (index, entry) in entries.enumerate() {
                    if index >= 2048 || next.len() >= 128 {
                        return Err("Include 通配匹配过多，已停止展开。".into());
                    }
                    let entry = entry.map_err(|error| error.to_string())?;
                    let name = entry.file_name();
                    let name = name.to_string_lossy();
                    let pattern = part.to_string_lossy();
                    if (!name.starts_with('.') || pattern.starts_with('.'))
                        && glob_matches(&pattern, &name)
                    {
                        next.push(entry.path());
                    }
                }
            }
            next.sort();
            paths = next;
        } else {
            for path in &mut paths {
                path.push(component.as_os_str());
            }
        }
    }
    Ok(paths)
}
