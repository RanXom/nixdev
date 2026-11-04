use std::fs;
use std::io::IsTerminal;

use crate::index;
use crate::nix;
use crate::output::Out;
use crate::theme;

pub fn run(out: &Out) -> i32 {
    out.banner();
    println!("  {}", theme::ansi_bold(out.mode(), "environment"));

    let mut failures = 0;
    let mut check = |ok: bool, label: &str, detail: String, fatal: bool| {
        if ok {
            println!("  {} {label:<26} {}", theme::ansi_fg(out.mode(), theme::SUCCESS, "✓"), detail);
        } else {
            let symbol = if fatal { "✗" } else { "!" };
            let color = if fatal { theme::ERROR } else { theme::WARN };
            println!(
                "  {} {label:<26} {}",
                theme::ansi_fg(out.mode(), color, symbol),
                detail
            );
            if fatal {
                failures += 1;
            }
        }
    };

    match nix::preflight() {
        Ok(info) => check(true, "nix", format!("version {}", info.version), true),
        Err(err) => check(false, "nix", err.to_string(), true),
    }

    let flakes = nix::run(
        &nix::nix_bin(),
        &["config".into(), "show".into(), "experimental-features".into()],
        None,
        std::time::Duration::from_secs(15),
    )
    .map(|out| out.stdout.to_lowercase().contains("flakes"))
    .unwrap_or(false);
    check(
        flakes,
        "flakes",
        if flakes {
            "enabled".into()
        } else {
            "not in experimental-features; nixdev passes the flags per command".into()
        },
        false,
    );

    check(
        nix::find_in_path("nix-instantiate").is_some(),
        "nix-instantiate",
        "used to syntax-check generated flakes".into(),
        false,
    );
    check(
        nix::find_in_path("git").is_some(),
        "git",
        "needed for `git init` and .gitignore updates".into(),
        false,
    );

    let formatter = ["nixfmt", "alejandra"]
        .iter()
        .find(|name| nix::find_in_path(name).is_some());
    check(
        formatter.is_some(),
        "formatter",
        match formatter {
            Some(name) => format!("{name} found"),
            None => "nixfmt or alejandra not installed; flakes stay unformatted".into(),
        },
        false,
    );

    let direnv = nix::find_in_path("direnv").is_some();
    check(
        direnv,
        "direnv",
        if direnv {
            "available; `direnv allow` will load the shell".into()
        } else {
            "not installed; .envrc is still written for later".into()
        },
        false,
    );

    let cache = index::cache_dir();
    let writable = fs::create_dir_all(&cache)
        .and_then(|_| {
            let probe = cache.join(".probe");
            fs::write(&probe, b"ok")?;
            fs::remove_file(&probe)
        })
        .is_ok();
    check(
        writable,
        "cache directory",
        cache.display().to_string(),
        false,
    );

    check(
        std::io::stdin().is_terminal() && std::io::stdout().is_terminal(),
        "terminal",
        format!("{} colours", colour_name(out.mode())),
        false,
    );

    println!();
    if failures > 0 {
        out.error("nixdev cannot run until the failures above are fixed");
        1
    } else {
        out.info("everything nixdev needs is available");
        0
    }
}

fn colour_name(mode: theme::ColorMode) -> &'static str {
    match mode {
        theme::ColorMode::TrueColor => "truecolor",
        theme::ColorMode::Ansi256 => "256",
        theme::ColorMode::Ansi16 => "16",
        theme::ColorMode::None => "no",
    }
}
