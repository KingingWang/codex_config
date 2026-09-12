//! Finding and restarting the Codex `app-server` processes that clients (the
//! ChatGPT desktop app, Zed, a manual terminal…) keep running in the
//! background.
//!
//! Why this exists: `config.toml` and the model catalog are read *once* when an
//! app-server starts. After editing them here, the running servers keep using
//! the old values until they are restarted. These helpers let the user restart
//! the right server(s) without quitting the whole client app.
//!
//! Everything here is deliberately conservative:
//! * We never touch `app-server proxy` (an SSH forwarder) or `app-server
//!   daemon` (a different, self-managing lifecycle).
//! * We never touch our own process.
//! * Scanning is read-only; killing happens only when the user asks for it.

use std::collections::HashMap;

use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, Signal, System, UpdateKind};

/// Which client is responsible for a given app-server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Host {
    ChatGptDesktop,
    Zed,
    /// A client we recognised by name (e.g. VS Code) — the string is shown as-is.
    Named(String),
    /// Could not attribute it to a known GUI client (manual terminal, unknown…).
    Unknown,
}

impl Host {
    pub fn label(&self) -> String {
        match self {
            Host::ChatGptDesktop => "ChatGPT 桌面应用".to_string(),
            Host::Zed => "Zed".to_string(),
            Host::Named(name) => name.clone(),
            Host::Unknown => "其它 / 手动启动".to_string(),
        }
    }

    /// Whether the host is known to respawn its app-server child automatically
    /// after we kill it. Only these are safe to "restart" by killing.
    pub fn auto_respawns(&self) -> bool {
        matches!(self, Host::ChatGptDesktop | Host::Zed | Host::Named(_))
    }
}

/// One logical app-server instance (its native binary plus the node shell that
/// launched it, grouped together).
#[derive(Debug, Clone)]
pub struct ServerInstance {
    /// PID of the native `codex … app-server` process — the one we kill.
    pub pid: u32,
    /// PID of the `node … codex … app-server` shell that owns the native one,
    /// if present.
    pub shell_pid: Option<u32>,
    pub host: Host,
    /// Short, human-friendly command summary for display.
    pub cmd_summary: String,
    /// True when this instance is (an ancestor of) our own process — i.e. the
    /// server hosting the session we are running inside. Killing it would end
    /// the current task, so the UI protects it.
    pub is_our_host: bool,
}

/// A raw process row we care about.
struct Row {
    cmd: String,
    parent: Option<u32>,
    native: bool,
}

fn is_app_server(cmd: &str) -> bool {
    let tokens: Vec<&str> = cmd.split_whitespace().collect();
    // `app-server` must be a real argument, not a substring of some path.
    if !tokens.iter().any(|t| *t == "app-server") {
        return false;
    }
    // Exclude the SSH forwarder and the self-managing daemon.
    if cmd.contains("app-server proxy")
        || cmd.contains("app-server daemon")
        || cmd.contains("pid-update-loop")
    {
        return false;
    }
    // The launching binary (first token) must actually be codex (either the
    // native binary or the `node …/codex` wrapper) — not a shell / cargo / any
    // other process whose arguments merely happen to mention "app-server".
    let Some(first) = tokens.first() else { return false };
    let bin = first.rsplit('/').next().unwrap_or(first);
    let launcher_is_codex = bin == "codex"
        || bin == "codex.exe"
        || bin.starts_with("codex-");
    // The `node …/codex app-server` wrapper: node is the binary, but the next
    // token is a path ending in `/codex`.
    let node_launches_codex = (bin == "node" || bin == "node.exe")
        && tokens
            .iter()
            .any(|t| t.ends_with("/codex") || t.ends_with("/codex.js") || *t == "codex");
    launcher_is_codex || node_launches_codex
}

/// The native codex binary (not the node wrapper) — this is the process that
/// actually reads the config, and the one we restart.
fn is_native(cmd: &str) -> bool {
    cmd.contains("bin/codex") || cmd.contains("codex-darwin") || cmd.contains("codex-linux")
        || cmd.contains("codex-windows") || cmd.contains("codex.exe")
}

fn host_of(mut pid: u32, all: &HashMap<u32, Row>) -> Host {
    // Attribute the host by the GUI app's *main executable* (under
    // `.app/Contents/MacOS/…`). We deliberately ignore matches under
    // `/Resources/`: e.g. Zed reuses `ChatGPT.app/Contents/Resources/codex`,
    // so its app-server command line mentions "ChatGPT.app" even though Zed is
    // the real host. Requiring `/Contents/MacOS/` avoids that false positive.
    for _ in 0..16 {
        let Some(row) = all.get(&pid) else { break };
        let c = &row.cmd;
        if c.contains("ChatGPT.app/Contents/MacOS/") {
            return Host::ChatGptDesktop;
        }
        if c.contains("Zed.app/Contents/MacOS/") {
            return Host::Zed;
        }
        if c.contains("Code.app/Contents/MacOS/")
            || c.contains("Visual Studio Code.app/Contents/MacOS/")
            || c.contains("Code Helper")
        {
            return Host::Named("VS Code".to_string());
        }
        match row.parent {
            Some(p) => pid = p,
            None => break,
        }
    }
    Host::Unknown
}

fn ancestors(mut pid: u32, all: &HashMap<u32, Row>) -> Vec<u32> {
    let mut out = Vec::new();
    for _ in 0..64 {
        let Some(row) = all.get(&pid) else { break };
        match row.parent {
            Some(p) => {
                out.push(p);
                pid = p;
            }
            None => break,
        }
    }
    out
}

fn shorten(cmd: &str) -> String {
    // Collapse to something readable: keep the binary tail + notable flags.
    let flags: Vec<&str> = cmd
        .split_whitespace()
        .filter(|t| t.contains("app-server") || t.starts_with("--") || *t == "-c")
        .take(4)
        .collect();
    let bin = cmd
        .split_whitespace()
        .next()
        .and_then(|p| p.rsplit('/').next())
        .unwrap_or("codex");
    if flags.is_empty() {
        bin.to_string()
    } else {
        format!("{bin} … {}", flags.join(" "))
    }
}

/// Scan all processes and return the app-server instances we found.
///
/// This is read-only. `our_pid` is normally `std::process::id()`; instances
/// that are ancestors of it are flagged `is_our_host` so the UI can protect
/// them.
pub fn scan() -> Vec<ServerInstance> {
    scan_with(&mut System::new(), std::process::id())
}

/// Testable core of [`scan`].
pub fn scan_with(sys: &mut System, our_pid: u32) -> Vec<ServerInstance> {
    sys.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing().with_cmd(UpdateKind::Always),
    );

    let mut all: HashMap<u32, Row> = HashMap::new();
    for (pid, p) in sys.processes() {
        let cmd = p
            .cmd()
            .iter()
            .map(|s| s.to_string_lossy())
            .collect::<Vec<_>>()
            .join(" ");
        all.insert(
            pid.as_u32(),
            Row {
                native: is_native(&cmd),
                cmd,
                parent: p.parent().map(|x| x.as_u32()),
            },
        );
    }

    let our_ancestors = ancestors(our_pid, &all);

    // Prefer the native binary as the canonical instance. When a native process
    // is present, its node shell parent is folded into the same instance and we
    // do not emit the shell separately.
    let mut shells_covered: Vec<u32> = Vec::new();
    let mut instances: Vec<ServerInstance> = Vec::new();

    for (pid, row) in &all {
        if *pid == our_pid || !is_app_server(&row.cmd) || !row.native {
            continue;
        }
        let shell_pid = row.parent.filter(|p| {
            all.get(p)
                .map(|r| is_app_server(&r.cmd) && !r.native)
                .unwrap_or(false)
        });
        if let Some(sp) = shell_pid {
            shells_covered.push(sp);
        }
        // The host is best determined from the shell's ancestry (the shell is
        // the child the GUI actually launched).
        let host_probe_pid = shell_pid.unwrap_or(*pid);
        let host = host_of(host_probe_pid, &all);
        let is_our_host = our_ancestors.contains(pid)
            || shell_pid.map(|s| our_ancestors.contains(&s)).unwrap_or(false)
            || *pid == our_pid;
        instances.push(ServerInstance {
            pid: *pid,
            shell_pid,
            host,
            cmd_summary: shorten(&row.cmd),
            is_our_host,
        });
    }

    // Emit any app-server node shells that have no native child of their own
    // (unusual, but keep them visible rather than hiding a server).
    for (pid, row) in &all {
        if *pid == our_pid || !is_app_server(&row.cmd) || row.native {
            continue;
        }
        if shells_covered.contains(pid) {
            continue;
        }
        let host = host_of(*pid, &all);
        let is_our_host = our_ancestors.contains(pid) || *pid == our_pid;
        instances.push(ServerInstance {
            pid: *pid,
            shell_pid: None,
            host,
            cmd_summary: shorten(&row.cmd),
            is_our_host,
        });
    }

    instances.sort_by_key(|i| i.pid);
    instances
}

/// Result of trying to restart one instance.
#[derive(Debug, Clone)]
pub struct RestartOutcome {
    pub pid: u32,
    pub host: String,
    pub ok: bool,
    pub message: String,
}

/// Kill one instance's native process (and its node shell, if any), so the host
/// app respawns a fresh server that re-reads the config.
///
/// Refuses to touch an instance flagged `is_our_host`.
pub fn restart_instance(inst: &ServerInstance) -> RestartOutcome {
    if inst.is_our_host {
        return RestartOutcome {
            pid: inst.pid,
            host: inst.host.label(),
            ok: false,
            message: "跳过：这是托管当前会话的服务，重启它会中断你正在进行的任务。".to_string(),
        };
    }
    let mut sys = System::new();
    sys.refresh_processes(ProcessesToUpdate::All, true);

    let killed = kill_pid(&sys, inst.pid);
    // Killing the native child is enough for the host to notice and respawn; we
    // deliberately leave the node shell alone unless the native one is gone and
    // a lone shell remains.
    if killed {
        RestartOutcome {
            pid: inst.pid,
            host: inst.host.label(),
            ok: true,
            message: if inst.host.auto_respawns() {
                format!("已结束进程 {}，{} 会自动拉起一个读取新配置的服务。", inst.pid, inst.host.label())
            } else {
                format!("已结束进程 {}。请手动重新启动它以应用新配置。", inst.pid)
            },
        }
    } else {
        RestartOutcome {
            pid: inst.pid,
            host: inst.host.label(),
            ok: false,
            message: format!("无法结束进程 {}（可能已退出，或权限不足）。", inst.pid),
        }
    }
}

fn kill_pid(sys: &System, pid: u32) -> bool {
    if let Some(p) = sys.process(Pid::from_u32(pid)) {
        // Graceful terminate first; the host treats it as a normal child exit.
        if p.kill_with(Signal::Term).unwrap_or(false) {
            return true;
        }
        return p.kill();
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn row(cmd: &str, parent: Option<u32>) -> Row {
        Row { native: is_native(cmd), cmd: cmd.to_string(), parent }
    }

    #[test]
    fn app_server_argument_must_be_a_real_token() {
        // Real codex launchers.
        assert!(is_app_server("/Applications/ChatGPT.app/Contents/Resources/codex app-server"));
        assert!(is_app_server(
            "node /Applications/ChatGPT.app/Contents/Resources//codex app-server"
        ));
        assert!(is_app_server(
            ".../codex-darwin-arm64/bin/codex -c app-server --analytics-default-enabled"
        ));
        // A shell whose cwd merely mentions the words must NOT match.
        assert!(!is_app_server(
            "zsh -lc cd /Users/x/codex_config && cargo run --example app-server"
        ));
        // Excluded lifecycles.
        assert!(!is_app_server("/x/codex app-server proxy"));
        assert!(!is_app_server("/x/codex app-server daemon"));
    }

    #[test]
    fn zed_reusing_the_chatgpt_binary_is_still_attributed_to_zed() {
        // Zed launches `ChatGPT.app/Contents/Resources/codex`, so the command
        // line mentions ChatGPT.app — but the GUI host is Zed, found by walking
        // parents to `Zed.app/Contents/MacOS/zed`.
        let mut all: HashMap<u32, Row> = HashMap::new();
        all.insert(10, row("node /Applications/ChatGPT.app/Contents/Resources//codex app-server", Some(20)));
        all.insert(20, row("/opt/homebrew/.../node .../codex-acp/...", Some(30)));
        all.insert(30, row("/Applications/Zed.app/Contents/MacOS/zed", Some(1)));
        assert_eq!(host_of(10, &all), Host::Zed);
    }

    #[test]
    fn chatgpt_desktop_is_attributed_by_its_main_executable() {
        let mut all: HashMap<u32, Row> = HashMap::new();
        all.insert(10, row("node /Applications/ChatGPT.app/Contents/Resources//codex app-server", Some(20)));
        all.insert(20, row("/Applications/ChatGPT.app/Contents/MacOS/ChatGPT", Some(1)));
        assert_eq!(host_of(10, &all), Host::ChatGptDesktop);
    }
}
