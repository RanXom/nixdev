mod cli;
mod doctor;
mod index;
mod nix;
mod output;
mod pipeline;
mod plan;
mod profile;
mod render;
mod theme;
mod util;
mod wizard;
mod write;

use std::io::{IsTerminal, Write};
use std::path::Path;
use std::time::Duration;

use clap::{CommandFactory, Parser};

use crate::cli::{Cli, Command, InitArgs};
use crate::output::Out;
use crate::plan::Plan;
use crate::theme::ColorMode;

fn main() {
    install_signal_handlers();
    std::process::exit(real_main());
}

fn real_main() -> i32 {
    let cli = Cli::parse();
    if cli.version {
        println!("nixdev {}", env!("CARGO_PKG_VERSION"));
        return 0;
    }
    let mode = ColorMode::detect(cli.no_color);
    let out = Out::new(mode);
    let command = match (cli.dir, cli.command) {
        (Some(dir), _) => Command::Init(InitArgs::in_dir(dir)),
        (None, Some(command)) => command,
        (None, None) => Command::Init(InitArgs::defaults()),
    };

    match command {
        Command::Init(args) => run_init(args, &out),
        Command::Add(args) => run_add(args, &out),
        Command::Search(args) => run_search(args, &out),
        Command::Index(args) => run_index(args, &out),
        Command::Doctor => doctor::run(&out),
        Command::Completions { shell } => {
            let mut command = Cli::command();
            clap_complete::generate(shell, &mut command, "nixdev", &mut std::io::stdout());
            0
        }
    }
}

fn run_init(args: InitArgs, out: &Out) -> i32 {
    nix::reset_cancel();
    match init_inner(args, out) {
        Ok(()) => 0,
        Err(err) if err == "cancelled" => 130,
        Err(err) => {
            out.fail(&err);
            1
        }
    }
}

fn init_inner(args: InitArgs, out: &Out) -> Result<(), String> {
    let interactive =
        std::io::stdin().is_terminal() && std::io::stdout().is_terminal() && !args.yes;

    if !interactive && !args.yes && !args.print && !args.dry_run {
        return Err(
            "prompts need a terminal here; pass --yes to accept the defaults without asking".into(),
        );
    }

    let plan = build_plan(&args, out)?;

    if args.print {
        let text = plan.render()?;
        let mut stdout = std::io::stdout().lock();
        stdout
            .write_all(text.as_bytes())
            .map_err(|err| format!("could not write to stdout: {err}"))?;
        return Ok(());
    }

    if args.dry_run {
        out.banner();
        out.plan_summary(&plan);
        out.preview(&plan.render()?);
        out.info("dry run: nothing was written");
        return Ok(());
    }

    let opts = pipeline_options(&args);

    if interactive {
        return wizard::run(plan, opts, out, !args.no_index, !args.no_validate);
    }

    run_headless(&plan, &opts, out, !args.no_validate)
}

fn run_headless(
    plan: &Plan,
    opts: &pipeline::Options,
    out: &Out,
    validate: bool,
) -> Result<(), String> {
    out.banner();
    out.plan_summary(plan);
    if validate {
        for warning in check_packages(plan, out) {
            out.warn(&warning);
        }
    }
    let report = pipeline::run(plan, opts, |event| match event {
        pipeline::Event::Started(id) => out.started(id.label()),
        pipeline::Event::Finished(step) => out.finished(&step),
    })?;
    out.report(&report);
    out.next_steps(&plan.dir);
    Ok(())
}

fn build_plan(args: &InitArgs, out: &Out) -> Result<Plan, String> {
    if !args.dir.exists() {
        if !args.print && !args.dry_run {
            std::fs::create_dir_all(&args.dir)
                .map_err(|err| format!("could not create `{}`: {err}", args.dir.display()))?;
        }
    } else if !args.dir.is_dir() {
        return Err(format!("`{}` is not a directory", args.dir.display()));
    }

    plan::validate_languages(&args.lang)?;
    plan::validate_extras(&args.extras)?;
    for pkg in &args.packages {
        if !util::valid_attr(pkg) {
            return Err(format!("`{pkg}` is not a valid Nix attribute path"));
        }
    }

    let mut plan = Plan::seed(args.dir.clone());
    if let Some(name) = &args.name {
        plan.name = util::sanitize_project_name(name);
        plan.description = format!("{} development environment", plan.name);
    }
    if let Some(description) = &args.description {
        plan.description = description.clone();
    }
    plan.languages = dedupe(args.lang.clone());
    plan.extras = dedupe(args.extras.clone());
    plan.packages = args.packages.clone();
    if !args.systems.is_empty() {
        plan.systems = args.systems.clone();
    }
    if let Some(shell) = &args.shell {
        plan.shell = shell.clone();
    }
    let mut plan = plan.normalize()?;

    let reference = args.nixpkgs.as_deref().unwrap_or("nixpkgs");
    let pinned = if args.print || args.dry_run {
        eprintln!("nixdev: resolving {reference}");
        nix::resolve_nixpkgs(reference).map_err(|err| err.to_string())?
    } else {
        out.spin(&format!("resolving {reference}"), || {
            nix::resolve_nixpkgs(reference)
        })
        .map_err(|err| err.to_string())?
    };
    plan.rev = pinned.rev.clone();
    plan.nixpkgs_url = if reference == "nixpkgs" {
        format!("github:NixOS/nixpkgs/{}", pinned.rev)
    } else {
        plan::pin_url(reference, &pinned.rev)
    };
    plan.nixpkgs_note = format!("{reference} pinned on {}", pinned.date);
    plan.render()?;
    Ok(plan)
}

fn pipeline_options(args: &InitArgs) -> pipeline::Options {
    pipeline::Options {
        allow_overwrite: args.force,
        backup: !args.no_backup,
        gitignore: !args.no_gitignore,
        envrc: !args.no_envrc,
        git: !args.no_git,
        lock: !args.no_lock,
        format: !args.no_format,
        check: args.check,
    }
}

fn check_packages(plan: &Plan, out: &Out) -> Vec<String> {
    if plan.to_spec().packages.is_empty() {
        return Vec::new();
    }
    out.spin("checking packages", || plan::package_warnings(plan))
}

fn run_add(args: cli::AddArgs, out: &Out) -> i32 {
    match add_inner(args, out) {
        Ok(()) => 0,
        Err(err) => {
            out.fail(&err);
            1
        }
    }
}

fn add_inner(args: cli::AddArgs, out: &Out) -> Result<(), String> {
    let path = args.dir.join("flake.nix");
    if !path.is_file() {
        return Err(format!("`{}` does not exist", path.display()));
    }
    let contents = write::read_flake(&path)?;
    let updated = render::add_packages(&contents, &args.packages)?;
    if updated == contents {
        out.info("all of those packages are already in the flake");
        return Ok(());
    }
    validate_text(&args.dir, &updated)?;

    if args.dry_run {
        print!("{updated}");
        return Ok(());
    }

    let report = write::write_atomic(&path, &updated, true)
        .map_err(|err| format!("could not update flake.nix: {err}"))?;
    out.info(&format!(
        "added {} to {}",
        args.packages.join(", "),
        report.path.display()
    ));
    if let Some(backup) = report.backup {
        out.info(&format!("previous version kept at {}", backup.display()));
    }

    if !args.no_format {
        match nix::format_file(&path) {
            Ok(Some(formatter)) => out.info(&format!("formatted with {formatter}")),
            Ok(None) => {}
            Err(err) => out.warn(&format!("could not format: {err}")),
        }
    }
    Ok(())
}

fn validate_text(dir: &Path, text: &str) -> Result<(), String> {
    if nix::find_in_path("nix-instantiate").is_none() {
        return Ok(());
    }
    let temp = dir.join(format!(".nixdev.check.{}.nix", std::process::id()));
    std::fs::write(&temp, text).map_err(|err| format!("could not write a check file: {err}"))?;
    let result = nix::parse_file(&temp);
    let _ = std::fs::remove_file(&temp);
    match result {
        Ok(None) => Ok(()),
        Ok(Some(stderr)) => Err(format!(
            "the updated flake would not parse; nothing was changed:\n{}",
            stderr.trim()
        )),
        Err(err) => Err(format!("could not validate the updated flake: {err}")),
    }
}

fn run_search(args: cli::SearchArgs, out: &Out) -> i32 {
    match search_inner(args, out) {
        Ok(()) => 0,
        Err(err) => {
            out.fail(&err);
            1
        }
    }
}

fn search_inner(args: cli::SearchArgs, out: &Out) -> Result<(), String> {
    let term = args.term.unwrap_or_default();
    let limit = args.limit.clamp(1, 500);
    let system = nix::current_system();
    let mut entries = Vec::new();

    if !args.live
        && let Ok(pinned) = nix::resolve_nixpkgs("nixpkgs")
    {
        if let Some(index) = index::load(&pinned.rev, &system) {
            entries = index.search(&term, limit).into_iter().cloned().collect();
        } else {
            out.started(&format!("indexing nixpkgs for {system}"));
            match index::load_or_build(&pinned.rev, &system) {
                Ok(index) => {
                    out.clear_line();
                    entries = index.search(&term, limit).into_iter().cloned().collect();
                }
                Err(err) => {
                    out.clear_line();
                    out.warn(&format!("could not build the index: {err}"));
                }
            }
        }
    }

    if entries.is_empty() {
        entries = nix::live_search(&term, Duration::from_secs(300))
            .map_err(|err| format!("could not search nixpkgs: {err}"))?;
    }

    if args.json {
        let text = serde_json::to_string_pretty(&entries)
            .map_err(|err| format!("could not encode results: {err}"))?;
        println!("{text}");
        return Ok(());
    }

    if entries.is_empty() {
        out.info("no packages matched");
        return Ok(());
    }

    let terminal_width = std::env::var("COLUMNS")
        .ok()
        .and_then(|c| c.parse::<usize>().ok())
        .unwrap_or(120);
    for entry in entries.iter().take(limit) {
        let attr_width = 34.min(terminal_width.saturating_sub(20));
        let version_width = 12;
        let description_width = terminal_width
            .saturating_sub(attr_width + version_width + 8)
            .max(20);
        println!(
            "  {} {} {}",
            theme::ansi_fg(
                out.mode(),
                theme::ACCENT,
                &format!("{:<attr_width$}", util::truncate(&entry.attr, attr_width))
            ),
            theme::ansi_dim(
                out.mode(),
                &format!(
                    "{:<version_width$}",
                    util::truncate(&entry.version, version_width)
                )
            ),
            util::truncate(&entry.description, description_width)
        );
    }
    println!();
    out.info(&format!(
        "{} result{}; add one with `nixdev add <attr>`",
        entries.len(),
        if entries.len() == 1 { "" } else { "s" }
    ));
    Ok(())
}

fn run_index(args: cli::IndexArgs, out: &Out) -> i32 {
    match index_inner(args, out) {
        Ok(()) => 0,
        Err(err) => {
            out.fail(&err);
            1
        }
    }
}

fn index_inner(args: cli::IndexArgs, out: &Out) -> Result<(), String> {
    let system = args.system.unwrap_or_else(nix::current_system);
    let rev = match args.rev {
        Some(rev) => rev,
        None => {
            nix::resolve_nixpkgs("nixpkgs")
                .map_err(|err| err.to_string())?
                .rev
        }
    };
    if !args.refresh && index::is_cached(&rev, &system) {
        out.info(&format!(
            "index is already cached at {}",
            index::index_path(&rev, &system).display()
        ));
        return Ok(());
    }
    out.started(&format!("indexing nixpkgs for {system}"));
    let index = index::build(&rev, &system)?;
    let path = index::save(&index)?;
    out.clear_line();
    out.info(&format!(
        "indexed {} packages into {}",
        index.len(),
        path.display()
    ));
    Ok(())
}

fn dedupe(items: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for item in items {
        let item = item.trim().to_string();
        if !item.is_empty() && !out.contains(&item) {
            out.push(item);
        }
    }
    out
}

fn install_signal_handlers() {
    #[cfg(unix)]
    {
        use signal_hook::consts::{SIGHUP, SIGINT, SIGQUIT, SIGTERM};
        if let Ok(mut signals) =
            signal_hook::iterator::Signals::new([SIGINT, SIGTERM, SIGHUP, SIGQUIT])
        {
            std::thread::spawn(move || {
                let _ = signals.forever().next();
                nix::cancel_all();
                std::process::exit(130);
            });
        }
    }
}
