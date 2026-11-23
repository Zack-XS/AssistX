//! Tool sandbox (docs/03 Sandbox, docs/06 T13).
//!
//! macOS: a `sandbox-exec` profile per run. Deny by default; read only system folders, the plugin
//! folder, the runner binary and the plugin's declared `needs.paths`; write only scratch, the
//! plugin's `state/` and `needs.paths`; network only to the run's proxy port on 127.0.0.1.
//! Linux (Landlock + seccomp) is not built yet: tools are refused there unless the operator
//! sets AGENT_UNSAFE_NO_SANDBOX=1.

use std::path::{Path, PathBuf};
use std::process::Command;

pub struct Spec {
    pub plugin_dir: PathBuf,
    pub scratch: PathBuf,
    pub exe: PathBuf,
    pub rw_paths: Vec<PathBuf>,
    pub proxy_port: Option<u16>,
    /// Linux: Unix socket (inside `scratch`) that reaches the run's proxy; the sandbox has no network.
    pub proxy_sock: Option<PathBuf>,
    /// Bundle ids this plugin may send Apple Events to (e.g. com.spotify.client).
    pub apple_events: Vec<String>,
    /// Folders to cover with an empty one (Linux): the server's data folder.
    pub hide: Vec<PathBuf>,
    /// Extra folders a tool may read and run programs from (`needs.programs`, resolved by the core).
    pub ro_paths: Vec<PathBuf>,
}

fn q(p: &Path) -> String {
    // SBPL string literal. Paths with quotes or backslashes are refused by `profile`.
    format!("\"{}\"", p.display())
}

fn real(p: &Path) -> PathBuf {
    std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

pub fn profile(s: &Spec) -> Result<String, String> {
    let all: Vec<&PathBuf> = [&s.plugin_dir, &s.scratch, &s.exe].into_iter().chain(s.rw_paths.iter()).chain(s.ro_paths.iter()).collect();
    if all.iter().any(|p| p.to_string_lossy().contains(['"', '\\', '\n'])) {
        return Err("path not allowed in sandbox profile".into());
    }
    let mut out = String::from("(version 1)\n(deny default)\n");
    out += "(allow process-fork)\n(allow process-exec)\n(allow signal (target same-sandbox))\n(allow sysctl-read)\n";
    out += "(allow file-read-metadata)\n";
    out += "(allow file-read* (literal \"/\") (subpath \"/usr\") (subpath \"/bin\") (subpath \"/sbin\") (subpath \"/System\") \
            (subpath \"/Library/Apple\") (subpath \"/private/etc\") (subpath \"/private/var/db/timezone\") (subpath \"/dev\") \
            (subpath \"/opt/homebrew\") (subpath \"/Library/Developer/CommandLineTools\"))\n";
    out += &format!("(allow file-read* (subpath {}) (literal {}) (subpath {}))\n", q(&real(&s.plugin_dir)), q(&real(&s.exe)), q(&real(&s.scratch)));
    out += &format!("(allow file-write* (subpath {}) (subpath {}) (literal \"/dev/null\") (literal \"/dev/tty\"))\n",
        q(&real(&s.scratch)), q(&real(&s.plugin_dir.join("state"))));
    for p in &s.rw_paths {
        out += &format!("(allow file-read* file-write* (subpath {}))\n", q(&real(p)));
    }
    for p in &s.ro_paths {
        out += &format!("(allow file-read* (subpath {}))\n", q(&real(p)));
    }
    if let Some(port) = s.proxy_port {
        out += &format!("(allow network-outbound (remote ip \"localhost:{port}\"))\n");
    }
    if !s.apple_events.is_empty() {
        out += "(allow mach-lookup (global-name \"com.apple.coreservices.appleevents\") (global-name \"com.apple.CoreServices.coreservicesd\") \
                (global-name \"com.apple.lsd.mapdb\") (global-name \"com.apple.distributed_notifications@Uv3\"))\n";
        out += "(allow ipc-posix-shm-read-data ipc-posix-shm-read-metadata)\n";
        for b in &s.apple_events {
            if !b.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-') {
                return Err(format!("bad bundle id '{b}'"));
            }
            out += &format!("(allow appleevent-send (appleevent-destination \"{b}\"))\n");
        }
    }
    Ok(out)
}

/// Which sandbox this process can start: `macos` (sandbox-exec) or `linux` (bubblewrap).
/// None when neither works (for example when already inside another sandbox).
pub fn probe() -> Option<&'static str> {
    let ok = |c: &mut Command| c.stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status().map(|s| s.success()).unwrap_or(false);
    if cfg!(target_os = "macos") && ok(Command::new("/usr/bin/sandbox-exec").args(["-p", "(version 1)(allow default)", "/usr/bin/true"])) {
        return Some("macos");
    }
    if cfg!(target_os = "linux") {
        let bwrap = bwrap_path()?;
        let mut c = Command::new(bwrap);
        c.args(["--unshare-all", "--die-with-parent", "--ro-bind", "/usr", "/usr", "--symlink", "usr/bin", "/bin", "--symlink", "usr/lib", "/lib", "--symlink", "usr/lib64", "/lib64", "/usr/bin/true"]);
        if ok(&mut c) {
            return Some("linux");
        }
    }
    None
}

fn bwrap_path() -> Option<&'static str> {
    ["/usr/bin/bwrap", "/bin/bwrap"].into_iter().find(|p| Path::new(p).exists())
}

/// bubblewrap arguments: a new, empty root with only the folders a tool may see.
/// No network at all (a plugin that needs the network is refused here, see `command`), new
/// user, pid, ipc and uts namespaces, and the child dies with its parent.
pub fn bwrap_args(s: &Spec) -> Vec<String> {
    let mut a: Vec<String> = ["--unshare-all", "--die-with-parent", "--new-session"].map(String::from).to_vec();
    let mut add = |xs: &[&str]| a.extend(xs.iter().map(|x| x.to_string()));
    add(&["--ro-bind", "/usr", "/usr"]);
    // Debian and most distros link these into /usr; bind the real folder where they are not links.
    for p in ["/bin", "/sbin", "/lib", "/lib64", "/lib32"] {
        match std::fs::read_link(p) {
            Ok(t) => add(&["--symlink", &t.to_string_lossy(), p]),
            Err(_) if Path::new(p).is_dir() => add(&["--ro-bind", p, p]),
            Err(_) => {}
        }
    }
    for p in ["/etc/ld.so.cache", "/etc/localtime", "/etc/alternatives", "/etc/ssl", "/etc/passwd", "/etc/group", "/usr/share/zoneinfo"] {
        add(&["--ro-bind-try", p, p]);
    }
    add(&["--proc", "/proc", "--dev", "/dev", "--tmpfs", "/tmp"]);
    let sp = |p: &Path| real(p).display().to_string();
    for p in &s.ro_paths {
        add(&["--ro-bind-try", &sp(p), &sp(p)]);
    }
    // Later binds win, so the wide folders go first and the narrow, read-only ones after them.
    for p in &s.rw_paths {
        if p.exists() {
            add(&["--bind", &sp(p), &sp(p)]);
        }
    }
    // A wide folder (the home) may hold the server's own data: tokens, secrets, the database.
    // Cover it with an empty folder, then put back only this run's scratch.
    for p in &s.hide {
        // Never cover a folder the tool was given (the data folder may itself be the work folder).
        let given = s.rw_paths.iter().any(|r| real(r).starts_with(real(p)));
        if p.exists() && !given {
            add(&["--tmpfs", &sp(p)]);
        }
    }
    add(&["--ro-bind", &sp(&s.plugin_dir), &sp(&s.plugin_dir)]);
    add(&["--ro-bind", &sp(&s.exe), &sp(&s.exe)]);
    add(&["--bind", &sp(&s.scratch), &sp(&s.scratch)]);
    let state = s.plugin_dir.join("state");
    if state.is_dir() {
        add(&["--bind", &sp(&state), &sp(&state)]);
    }
    add(&["--chdir", &sp(&s.plugin_dir)]);
    a
}

/// Wrap `argv` so it runs inside the sandbox. `mode` comes from config.
pub fn command(mode: &str, spec: &Spec, argv: &[String]) -> Result<Command, String> {
    let (prog, rest) = argv.split_first().ok_or("empty command")?;
    match mode {
        "macos" => {
            let mut c = Command::new("/usr/bin/sandbox-exec");
            c.arg("-p").arg(profile(spec)?).arg(prog).args(rest);
            Ok(c)
        }
        "linux" => {
            // The sandbox has no network. A network plugin reaches its proxy through a socket file
            // in its scratch folder (see `plugins::start_bridge`).
            if spec.proxy_port.is_some() && spec.proxy_sock.is_none() {
                return Err("this plugin needs the network, but no proxy socket was given".into());
            }
            if !spec.apple_events.is_empty() {
                return Err("this plugin needs macOS".into());
            }
            let mut c = Command::new(bwrap_path().ok_or("bubblewrap (bwrap) is not installed")?);
            c.args(bwrap_args(spec)).arg("--").arg(prog).args(rest);
            Ok(c)
        }
        "off" => {
            let mut c = Command::new(prog);
            c.args(rest);
            Ok(c)
        }
        _ => Err("no sandbox on this system; tools are disabled (set AGENT_UNSAFE_NO_SANDBOX=1 to run them unsandboxed)".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bwrap_hides_data_and_keeps_narrow_binds_last() {
        let base = std::env::temp_dir().join(format!("sbx-{}", std::process::id()));
        let (home, data, plug) = (base.join("home"), base.join("home/data"), base.join("home/plugins/terminal"));
        let scratch = data.join("run/x");
        for d in [&home, &plug, &scratch] {
            std::fs::create_dir_all(d).unwrap();
        }
        let exe = base.join("exe");
        std::fs::write(&exe, "").unwrap();
        let s = Spec { plugin_dir: plug.clone(), scratch: scratch.clone(), exe, rw_paths: vec![home.clone()], proxy_port: None, proxy_sock: None, apple_events: vec![], hide: vec![data.clone()], ro_paths: vec![] };
        let a = bwrap_args(&s);
        let at = |flag: &str, p: &Path| a.windows(2).position(|w| w[0] == flag && w[1] == real(p).display().to_string()).unwrap_or_else(|| panic!("{flag} {p:?} in {a:?}"));
        assert!(a.contains(&"--unshare-all".to_string()) && !a.contains(&"--share-net".to_string()), "no network");
        assert!(at("--bind", &home) < at("--tmpfs", &data), "data is covered after the home is bound");
        assert!(at("--tmpfs", &data) < at("--bind", &scratch), "only the scratch comes back");
        assert!(at("--bind", &home) < at("--ro-bind", &plug), "the plugin folder stays read-only inside the home");
        let net = Spec { proxy_port: Some(1), ..s };
        assert!(command("linux", &net, &["x".into()]).unwrap_err().contains("network"));
        let _ = std::fs::remove_dir_all(&base);
    }
}
