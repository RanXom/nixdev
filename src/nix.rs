use std::env;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde::Deserialize;

use crate::index::Entry;
use crate::util::{format_date, now_secs};

pub static CANCELED: AtomicBool = AtomicBool::new(false);

pub fn reset_cancel() {
    CANCELED.store(false, Ordering::SeqCst);
}

pub fn is_canceled() -> bool {
    CANCELED.load(Ordering::SeqCst)
}

pub fn cancel_all() {
    CANCELED.store(true, Ordering::SeqCst);
    let pids: Vec<u32> = children().lock().map(|g| g.clone()).unwrap_or_default();
    for pid in pids {
        unsafe {
            libc::killpg(pid as i32, libc::SIGTERM);
        }
    }
}

fn children() -> &'static Mutex<Vec<u32>> {
    static CHILDREN: OnceLock<Mutex<Vec<u32>>> = OnceLock::new();
    CHILDREN.get_or_init(|| Mutex::new(Vec::new()))
}

#[derive(Debug)]
pub enum NixError {
    Missing(String),
    Canceled,
    Timeout(Duration),
    Failed { command: String, stderr: String },
    Io(io::Error),
    Parse(String),
}

impl fmt::Display for NixError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing(program) => write!(f, "`{program}` is not installed or not on PATH"),
            Self::Canceled => write!(f, "cancelled"),
            Self::Timeout(d) => write!(f, "command timed out after {}s", d.as_secs()),
            Self::Failed { command, stderr } => {
                let lines: Vec<&str> = stderr
                    .lines()
                    .map(str::trim)
                    .filter(|line| !line.is_empty())
                    .collect();
                if lines.is_empty() {
                    write!(f, "`{command}` failed")
                } else {
                    let tail = lines[lines.len().saturating_sub(3)..].join("\n");
                    write!(f, "`{command}` failed:\n{tail}")
                }
            }
            Self::Io(err) => write!(f, "{err}"),
            Self::Parse(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for NixError {}

impl From<io::Error> for NixError {
    fn from(err: io::Error) -> Self {
        Self::Io(err)
    }
}

pub struct RunOutput {
    pub stdout: String,
}

pub fn nix_bin() -> String {
    env::var("NIXDEV_NIX").unwrap_or_else(|_| "nix".to_string())
}

fn scratch(tag: &str) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    env::temp_dir().join(format!("nixdev-run-{}-{n}.{tag}", std::process::id()))
}

struct Cleanup(Vec<PathBuf>);

impl Drop for Cleanup {
    fn drop(&mut self) {
        for path in &self.0 {
            let _ = fs::remove_file(path);
        }
    }
}

struct ChildGuard(u32);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if let Ok(mut pids) = children().lock() {
            pids.retain(|pid| *pid != self.0);
        }
    }
}

pub fn run(
    program: &str,
    args: &[String],
    cwd: Option<&Path>,
    timeout: Duration,
) -> Result<RunOutput, NixError> {
    if is_canceled() {
        return Err(NixError::Canceled);
    }
    let out_path = scratch("out");
    let err_path = scratch("err");
    let cleanup = Cleanup(vec![out_path.clone(), err_path.clone()]);

    let out_file = fs::File::create(&out_path)?;
    let err_file = fs::File::create(&err_path)?;

    let mut command = Command::new(program);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::from(out_file))
        .stderr(Stdio::from(err_file));
    if let Some(dir) = cwd {
        command.current_dir(dir);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }

    let mut child = command.spawn().map_err(|err| {
        if err.kind() == io::ErrorKind::NotFound {
            NixError::Missing(program.to_string())
        } else {
            NixError::Io(err)
        }
    })?;
    let pid = child.id();
    if let Ok(mut pids) = children().lock() {
        pids.push(pid);
    }
    let _guard = ChildGuard(pid);

    let deadline = Instant::now() + timeout;
    loop {
        if is_canceled() {
            kill_group(pid, libc::SIGTERM);
            let _ = child.wait();
            return Err(NixError::Canceled);
        }
        match child.try_wait()? {
            Some(status) => {
                let stdout = read_lossy(&out_path);
                let stderr = read_lossy(&err_path);
                drop(cleanup);
                if status.success() {
                    return Ok(RunOutput { stdout });
                }
                return Err(NixError::Failed {
                    command: format!("{program} {}", args.join(" ")),
                    stderr,
                });
            }
            None => {
                if Instant::now() >= deadline {
                    kill_group(pid, libc::SIGKILL);
                    let _ = child.wait();
                    return Err(NixError::Timeout(timeout));
                }
                std::thread::sleep(Duration::from_millis(25));
            }
        }
    }
}

fn kill_group(pid: u32, signal: i32) {
    unsafe {
        libc::killpg(pid as i32, signal);
    }
}

fn read_lossy(path: &Path) -> String {
    match fs::read(path) {
        Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
        Err(_) => String::new(),
    }
}

pub fn nix_args(parts: &[&str]) -> Vec<String> {
    let mut args = vec![
        "--extra-experimental-features".to_string(),
        "nix-command flakes".to_string(),
    ];
    args.extend(parts.iter().map(|s| s.to_string()));
    args
}

pub struct Preflight {
    pub version: String,
}

pub fn preflight() -> Result<Preflight, NixError> {
    let out = run(
        &nix_bin(),
        &["--version".to_string()],
        None,
        Duration::from_secs(15),
    )?;
    let version = out
        .stdout
        .split_whitespace()
        .last()
        .unwrap_or("unknown")
        .to_string();
    let mut parts = version.split('.');
    let major: u32 = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
    let minor: u32 = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
    if major < 2 || (major == 2 && minor < 4) {
        return Err(NixError::Parse(format!(
            "nix {version} is too old; flakes need Nix 2.4 or newer"
        )));
    }
    Ok(Preflight { version })
}

#[derive(Deserialize)]
struct MetaJson {
    locked: Option<LockedJson>,
}

#[derive(Deserialize)]
struct LockedJson {
    rev: Option<String>,
}

pub struct Pinned {
    pub rev: String,
    pub date: String,
}

pub fn resolve_nixpkgs(flake: &str) -> Result<Pinned, NixError> {
    let args = nix_args(&["flake", "metadata", "--json", flake]);
    let out = run(&nix_bin(), &args, None, Duration::from_secs(180))?;
    let parsed: MetaJson = serde_json::from_str(&out.stdout)
        .map_err(|err| NixError::Parse(format!("could not read nixpkgs metadata: {err}")))?;
    let rev = parsed
        .locked
        .and_then(|l| l.rev)
        .ok_or_else(|| NixError::Parse("nixpkgs is not a locked flake input".into()))?;
    Ok(Pinned {
        rev,
        date: format_date(now_secs()),
    })
}

pub fn live_search(term: &str, timeout: Duration) -> Result<Vec<Entry>, NixError> {
    let args = nix_args(&["search", "nixpkgs", term, "--json"]);
    let out = run(&nix_bin(), &args, None, timeout)?;
    let raw: serde_json::Value = serde_json::from_str(&out.stdout)
        .map_err(|err| NixError::Parse(format!("could not parse search results: {err}")))?;
    let mut entries = Vec::new();
    if let Some(map) = raw.as_object() {
        let system = current_system();
        for (key, value) in map {
            let attr = attr_from_key(key, &system);
            entries.push(crate::index::entry_from_json(attr, value));
        }
    }
    entries.sort_by(|a, b| a.attr.cmp(&b.attr));
    Ok(entries)
}

pub fn attr_from_key(key: &str, system: &str) -> String {
    let prefix = format!("legacyPackages.{system}.");
    if let Some(rest) = key.strip_prefix(&prefix) {
        return rest.to_string();
    }
    let mut parts = key.split('.');
    if parts.next() == Some("legacyPackages") && parts.next().is_some() {
        return parts.collect::<Vec<_>>().join(".");
    }
    key.to_string()
}

pub fn current_system() -> String {
    let arch = env::consts::ARCH;
    let os = env::consts::OS;
    let mapped_arch = match arch {
        "x86_64" => "x86_64",
        "aarch64" => "aarch64",
        "x86" => "i686",
        "arm" => "armv7l",
        other => other,
    };
    let mapped_os = match os {
        "linux" => "linux",
        "macos" => "darwin",
        other => other,
    };
    format!("{mapped_arch}-{mapped_os}")
}

pub fn has_attrs(system: &str, attrs: &[String]) -> Result<Vec<bool>, NixError> {
    if attrs.is_empty() {
        return Ok(Vec::new());
    }
    let paths: Vec<String> = attrs
        .iter()
        .map(|attr| {
            let segments: Vec<String> = attr
                .split('.')
                .map(|s| format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"")))
                .collect();
            format!("[{}]", segments.join(" "))
        })
        .collect();
    let expr = format!(
        "pkgs: map (path: pkgs.lib.hasAttrByPath path pkgs) [{}]",
        paths.join(" ")
    );
    let args = nix_args(&[
        "eval",
        "--json",
        &format!("nixpkgs#legacyPackages.{system}"),
        "--apply",
        &expr,
    ]);
    let out = run(&nix_bin(), &args, None, Duration::from_secs(180))?;
    serde_json::from_str(&out.stdout)
        .map_err(|err| NixError::Parse(format!("could not parse attribute check: {err}")))
}

pub fn system_supported(system: &str) -> Result<bool, NixError> {
    let args = nix_args(&[
        "eval",
        "--json",
        &format!("nixpkgs#legacyPackages.{system}"),
        "--apply",
        "pkgs: builtins.isAttrs pkgs",
    ]);
    match run(&nix_bin(), &args, None, Duration::from_secs(120)) {
        Ok(out) => Ok(out.stdout.trim() == "true"),
        Err(NixError::Canceled) => Err(NixError::Canceled),
        Err(_) => Ok(false),
    }
}

pub fn parse_file(path: &Path) -> Result<Option<String>, NixError> {
    if find_in_path("nix-instantiate").is_none() {
        return Ok(None);
    }
    let args = vec!["--parse".to_string(), path.display().to_string()];
    match run("nix-instantiate", &args, None, Duration::from_secs(30)) {
        Ok(_) => Ok(None),
        Err(NixError::Canceled) => Err(NixError::Canceled),
        Err(NixError::Failed { stderr, .. }) => Ok(Some(stderr)),
        Err(err) => Err(err),
    }
}

pub fn format_file(path: &Path) -> Result<Option<String>, NixError> {
    for formatter in ["nixfmt", "alejandra"] {
        if find_in_path(formatter).is_some() {
            let args = vec![path.display().to_string()];
            run(formatter, &args, None, Duration::from_secs(60))?;
            return Ok(Some(formatter.to_string()));
        }
    }
    Ok(None)
}

pub fn flake_lock(dir: &Path) -> Result<(), NixError> {
    let args = nix_args(&["flake", "lock"]);
    run(&nix_bin(), &args, Some(dir), Duration::from_secs(300))?;
    Ok(())
}

pub fn flake_check(dir: &Path) -> Result<(), NixError> {
    let args = nix_args(&["flake", "check", "--no-build"]);
    run(&nix_bin(), &args, Some(dir), Duration::from_secs(600))?;
    Ok(())
}

pub fn find_in_path(name: &str) -> Option<PathBuf> {
    let path = env::var_os("PATH")?;
    for dir in env::split_paths(&path) {
        let candidate = dir.join(name);
        if is_executable(&candidate) {
            return Some(candidate);
        }
    }
    None
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    fs::metadata(path)
        .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_flake_attr_prefix() {
        assert_eq!(
            attr_from_key("legacyPackages.x86_64-linux.yyjson", "x86_64-linux"),
            "yyjson"
        );
        assert_eq!(
            attr_from_key(
                "legacyPackages.aarch64-darwin.python3Packages.requests",
                "aarch64-darwin"
            ),
            "python3Packages.requests"
        );
    }

    #[test]
    fn current_system_looks_like_a_double() {
        let system = current_system();
        assert!(system.contains('-'));
    }
}
