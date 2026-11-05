use std::path::PathBuf;
use std::process::{Command, Stdio};

fn bin() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_nixdev"));
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("NO_COLOR", "1");
    command
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("nixdev-test-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

#[test]
fn version_is_available() {
    let output = bin().arg("--version").output().unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("nixdev"));
}

#[test]
fn prompts_need_a_terminal() {
    let dir = scratch("tty");
    let output = bin()
        .args(["init", dir.to_str().unwrap()])
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr.contains("pass --yes"), "stderr: {stderr}");
    assert!(!dir.join("flake.nix").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn unknown_language_fails_before_nix_runs() {
    let dir = scratch("lang");
    let output = bin()
        .args([
            "init",
            dir.to_str().unwrap(),
            "--lang",
            "brainfuck",
            "--yes",
        ])
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr.contains("unknown language"), "stderr: {stderr}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn bad_attribute_path_is_rejected() {
    let dir = scratch("attr");
    let output = bin()
        .args(["init", dir.to_str().unwrap(), "--pkg", "bad attr", "--yes"])
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr.contains("not a valid Nix attribute path"),
        "stderr: {stderr}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
