use std::collections::hash_map::DefaultHasher;
use std::fmt;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime};

use fs2::FileExt;

#[derive(Debug)]
pub enum WriteError {
    MissingDir(PathBuf),
    Locked(PathBuf),
    BrokenSymlink(PathBuf),
    Io { context: String, source: io::Error },
}

impl fmt::Display for WriteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingDir(dir) => write!(f, "`{}` does not exist", dir.display()),
            Self::Locked(dir) => write!(
                f,
                "another nixdev run is writing to `{}`; try again in a moment",
                dir.display()
            ),
            Self::BrokenSymlink(path) => write!(
                f,
                "`{}` is a symlink that points nowhere; fix or remove it first",
                path.display()
            ),
            Self::Io { context, source } => write!(f, "{context}: {source}"),
        }
    }
}

impl std::error::Error for WriteError {}

fn io_err(context: impl Into<String>) -> impl FnOnce(io::Error) -> WriteError {
    let context = context.into();
    move |source| WriteError::Io { context, source }
}

pub struct WriteReport {
    pub path: PathBuf,
    pub backup: Option<PathBuf>,
    pub via_symlink: bool,
}

pub fn write_atomic(path: &Path, contents: &str, backup: bool) -> Result<WriteReport, WriteError> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let parent = if parent.as_os_str().is_empty() {
        Path::new(".")
    } else {
        parent
    };
    if !parent.is_dir() {
        return Err(WriteError::MissingDir(parent.to_path_buf()));
    }

    let (target, via_symlink) = resolve_target(path)?;
    let target_parent = target.parent().unwrap_or(parent);

    let _lock = DirLock::acquire(target_parent)?;
    cleanup_stale_temps(target_parent);

    let existing_mode = fs::metadata(&target).ok().map(|m| m.permissions());

    let mut backup_path = None;
    if backup && target.exists() {
        backup_path = Some(make_backup(&target)?);
    }

    let file_name = target
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "flake.nix".to_string());
    let temp = target_parent.join(format!(".nixdev.tmp.{}.{}", std::process::id(), counter()));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)
        .map_err(io_err(format!("could not create `{}`", temp.display())))?;

    let result = (|| -> Result<(), WriteError> {
        file.write_all(contents.as_bytes())
            .map_err(io_err("could not write temporary flake"))?;
        file.sync_all()
            .map_err(io_err("could not flush temporary flake"))?;
        if let Some(mode) = existing_mode.clone() {
            let _ = file.set_permissions(mode);
        }
        drop(file);
        fs::rename(&temp, &target)
            .map_err(io_err(format!("could not move `{}` into place", file_name)))?;
        if let Ok(dir) = fs::File::open(target_parent) {
            let _ = dir.sync_all();
        }
        Ok(())
    })();

    if let Err(err) = result {
        let _ = fs::remove_file(&temp);
        return Err(err);
    }

    Ok(WriteReport {
        path: target,
        backup: backup_path,
        via_symlink,
    })
}

fn resolve_target(path: &Path) -> Result<(PathBuf, bool), WriteError> {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_symlink() => {
            let link = fs::read_link(path).map_err(io_err("could not read symlink"))?;
            let base = path.parent().unwrap_or_else(|| Path::new("."));
            let resolved = if link.is_absolute() {
                link
            } else {
                base.join(link)
            };
            let target = resolved
                .canonicalize()
                .map_err(|_| WriteError::BrokenSymlink(path.to_path_buf()))?;
            Ok((target, true))
        }
        _ => Ok((path.to_path_buf(), false)),
    }
}

fn make_backup(target: &Path) -> Result<PathBuf, WriteError> {
    let stamp = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let name = target
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "flake.nix".to_string());
    let parent = target.parent().unwrap_or_else(|| Path::new("."));
    let backup = parent.join(format!("{name}.nixdev-backup-{stamp}"));
    fs::copy(target, &backup).map_err(io_err("could not back up existing flake"))?;
    prune_backups(parent, &name);
    Ok(backup)
}

fn prune_backups(dir: &Path, name: &str) {
    let prefix = format!("{name}.nixdev-backup-");
    let mut backups: Vec<PathBuf> = match fs::read_dir(dir) {
        Ok(entries) => entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.file_name()
                    .map(|n| n.to_string_lossy().starts_with(&prefix))
                    .unwrap_or(false)
            })
            .collect(),
        Err(_) => return,
    };
    backups.sort();
    while backups.len() > 5 {
        let oldest = backups.remove(0);
        let _ = fs::remove_file(oldest);
    }
}

fn cleanup_stale_temps(dir: &Path) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some(rest) = name.strip_prefix(".nixdev.tmp.") else {
            continue;
        };
        let pid: Option<u32> = rest.split('.').next().and_then(|p| p.parse().ok());
        let old = entry
            .metadata()
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|m| SystemTime::now().duration_since(m).ok())
            .is_some_and(|age| age > Duration::from_secs(600));
        let Some(pid) = pid else { continue };
        if !pid_alive(pid) && old {
            let _ = fs::remove_file(entry.path());
        }
    }
}

fn pid_alive(pid: u32) -> bool {
    #[cfg(target_os = "linux")]
    {
        Path::new(&format!("/proc/{pid}")).exists()
    }
    #[cfg(not(target_os = "linux"))]
    {
        unsafe { libc::kill(pid as i32, 0) == 0 }
    }
}

fn counter() -> u64 {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    COUNTER.fetch_add(1, Ordering::Relaxed)
}

struct DirLock {
    path: PathBuf,
}

impl DirLock {
    fn acquire(dir: &Path) -> Result<Self, WriteError> {
        let path = lock_path(dir);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(io_err("could not create the lock directory"))?;
        }
        let file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .map_err(io_err("could not create the lock file"))?;
        let mut waited = Duration::ZERO;
        loop {
            match file.try_lock_exclusive() {
                Ok(()) => return Ok(Self { path }),
                Err(_) if waited < Duration::from_secs(3) => {
                    std::thread::sleep(Duration::from_millis(100));
                    waited += Duration::from_millis(100);
                }
                Err(_) => return Err(WriteError::Locked(dir.to_path_buf())),
            }
        }
    }
}

impl Drop for DirLock {
    fn drop(&mut self) {
        if let Ok(file) = fs::File::open(&self.path) {
            let _ = file.unlock();
        }
    }
}

fn state_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("XDG_CACHE_HOME") {
        return PathBuf::from(dir).join("nixdev");
    }
    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home).join(".cache").join("nixdev");
    }
    std::env::temp_dir().join("nixdev")
}

fn lock_path(dir: &Path) -> PathBuf {
    let canonical = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());
    let mut hasher = DefaultHasher::new();
    canonical.hash(&mut hasher);
    state_dir()
        .join("locks")
        .join(format!("{:016x}.lock", hasher.finish()))
}

pub fn intent_to_add(dir: &Path, file: &str) -> Result<(), String> {
    let inside = std::process::Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["rev-parse", "--is-inside-work-tree"])
        .output();
    match inside {
        Ok(output) if output.status.success() => {}
        _ => return Ok(()),
    }
    let tracked = std::process::Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["ls-files", "--error-unmatch", "--", file])
        .output();
    if tracked
        .map(|output| output.status.success())
        .unwrap_or(false)
    {
        return Ok(());
    }
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["add", "-N", "--", file])
        .output()
        .map_err(|err| format!("could not run git add: {err}"))?;
    if !output.status.success() {
        return Err(format!(
            "git could not stage `{file}`: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(())
}

pub fn is_git_repo(dir: &Path) -> bool {
    dir.ancestors()
        .any(|ancestor| ancestor.join(".git").exists())
}

pub fn git_init(dir: &Path) -> Result<bool, String> {
    if is_git_repo(dir) {
        return Ok(false);
    }
    let output = std::process::Command::new("git")
        .arg("init")
        .arg("-q")
        .current_dir(dir)
        .output()
        .map_err(|err| format!("could not run git: {err}"))?;
    if !output.status.success() {
        return Err(format!(
            "git init failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(true)
}

pub fn append_gitignore(dir: &Path, lines: &[&str]) -> Result<bool, String> {
    let path = dir.join(".gitignore");
    let existing = fs::read_to_string(&path).unwrap_or_default();
    let mut additions = Vec::new();
    for line in lines {
        if !existing.lines().any(|l| l.trim() == *line) {
            additions.push(*line);
        }
    }
    if additions.is_empty() {
        return Ok(false);
    }
    let mut content = existing.clone();
    if !content.is_empty() && !content.ends_with('\n') {
        content.push('\n');
    }
    if !content.is_empty() {
        content.push('\n');
    }
    content.push_str("# nixdev\n");
    for line in additions {
        content.push_str(line);
        content.push('\n');
    }
    fs::write(&path, content).map_err(|err| format!("could not update .gitignore: {err}"))?;
    Ok(true)
}

pub fn write_envrc(dir: &Path) -> Result<bool, String> {
    let path = dir.join(".envrc");
    if path.exists() {
        return Ok(false);
    }
    let body = "use flake\n";
    fs::write(&path, body).map_err(|err| format!("could not write .envrc: {err}"))?;
    Ok(true)
}

pub fn read_flake(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|err| format!("could not read `{}`: {err}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn tempdir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "nixdev-test-{tag}-{}-{}",
            std::process::id(),
            counter()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn creates_and_replaces_files() {
        let dir = tempdir("replace");
        let path = dir.join("flake.nix");
        let first = write_atomic(&path, "one\n", true).unwrap();
        assert!(first.backup.is_none());
        assert_eq!(fs::read_to_string(&path).unwrap(), "one\n");

        let second = write_atomic(&path, "two\n", true).unwrap();
        assert!(second.backup.is_some());
        assert_eq!(fs::read_to_string(&path).unwrap(), "two\n");
        assert_eq!(fs::read_to_string(second.backup.unwrap()).unwrap(), "one\n");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn keeps_existing_permissions() {
        let dir = tempdir("perms");
        let path = dir.join("flake.nix");
        fs::write(&path, "x\n").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
        write_atomic(&path, "y\n", false).unwrap();
        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o640);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn refuses_missing_directory() {
        let dir = tempdir("missing");
        let path = dir.join("nope").join("flake.nix");
        assert!(matches!(
            write_atomic(&path, "x", false),
            Err(WriteError::MissingDir(_))
        ));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn appends_gitignore_lines_once() {
        let dir = tempdir("ignore");
        fs::write(dir.join(".gitignore"), "target\n").unwrap();
        assert!(append_gitignore(&dir, &[".direnv/", ".nixdev.lock"]).unwrap());
        assert!(!append_gitignore(&dir, &[".direnv/", ".nixdev.lock"]).unwrap());
        let text = fs::read_to_string(dir.join(".gitignore")).unwrap();
        assert_eq!(text.matches(".direnv/").count(), 1);
        fs::remove_dir_all(dir).unwrap();
    }
}
