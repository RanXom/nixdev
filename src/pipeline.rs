use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::nix::{self, NixError};
use crate::plan::Plan;
use crate::render;
use crate::write;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepId {
    Flake,
    Parse,
    Gitignore,
    Envrc,
    Git,
    Lock,
    Format,
    Check,
}

impl StepId {
    pub fn label(self) -> &'static str {
        match self {
            Self::Flake => "write flake.nix",
            Self::Parse => "validate nix syntax",
            Self::Gitignore => "update .gitignore",
            Self::Envrc => "write .envrc",
            Self::Git => "initialise git repository",
            Self::Lock => "generate flake.lock",
            Self::Format => "format flake.nix",
            Self::Check => "run nix flake check",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Ok,
    Skipped,
    Warn,
    Failed,
}

#[derive(Clone, Debug)]
pub struct StepResult {
    pub id: StepId,
    pub status: Status,
    pub detail: String,
    pub elapsed: Duration,
}

#[derive(Clone)]
pub struct Options {
    pub allow_overwrite: bool,
    pub backup: bool,
    pub gitignore: bool,
    pub envrc: bool,
    pub git: bool,
    pub lock: bool,
    pub format: bool,
    pub check: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            allow_overwrite: false,
            backup: true,
            gitignore: true,
            envrc: true,
            git: true,
            lock: true,
            format: true,
            check: false,
        }
    }
}

pub enum Event {
    Started(StepId),
    Finished(StepResult),
}

pub struct Report {
    pub path: PathBuf,
    pub backup: Option<PathBuf>,
    pub steps: Vec<StepResult>,
    pub elapsed: Duration,
    pub git_initialized: bool,
    pub envrc_written: bool,
    pub gitignore_updated: bool,
}

pub fn run<F: FnMut(Event)>(plan: &Plan, opts: &Options, mut sink: F) -> Result<Report, String> {
    let started = Instant::now();
    let spec = plan.to_spec();
    spec.validate()?;

    if !plan.dir.is_dir() {
        return Err(format!("`{}` is not a directory", plan.dir.display()));
    }

    let path = plan.dir.join("flake.nix");
    let existed = path.exists();
    if existed && !opts.allow_overwrite {
        return Err(format!(
            "`{}` already exists; pass --force to replace it, or use `nixdev add`",
            path.display()
        ));
    }
    let previous = if existed {
        fs::read_to_string(&path).ok()
    } else {
        None
    };

    let contents = render::render(&spec);
    let mut report = Report {
        path: path.clone(),
        backup: None,
        steps: Vec::new(),
        elapsed: Duration::ZERO,
        git_initialized: false,
        envrc_written: false,
        gitignore_updated: false,
    };

    sink(Event::Started(StepId::Flake));
    let step_started = Instant::now();
    let written = write::write_atomic(&path, &contents, opts.backup)
        .map_err(|err| format!("could not write flake.nix: {err}"))?;
    report.backup = written.backup.clone();
    finish(
        &mut report,
        &mut sink,
        StepId::Flake,
        Status::Ok,
        format!(
            "{}{}",
            path.display(),
            if written.via_symlink {
                " (via symlink)"
            } else {
                ""
            }
        ),
        step_started,
    );

    sink(Event::Started(StepId::Parse));
    let step_started = Instant::now();
    if nix::find_in_path("nix-instantiate").is_none() {
        finish(
            &mut report,
            &mut sink,
            StepId::Parse,
            Status::Skipped,
            "nix-instantiate is not installed".into(),
            step_started,
        );
    } else {
        match nix::parse_file(&path) {
            Ok(None) => finish(
                &mut report,
                &mut sink,
                StepId::Parse,
                Status::Ok,
                "the generated file parses".into(),
                step_started,
            ),
            Ok(Some(stderr)) => {
                finish(
                    &mut report,
                    &mut sink,
                    StepId::Parse,
                    Status::Failed,
                    first_line(&stderr),
                    step_started,
                );
                rollback(&path, previous.as_deref(), existed);
                return Err(
                    "the generated flake is not valid Nix; the previous file was restored".into(),
                );
            }
            Err(NixError::Canceled) => return Err("cancelled".into()),
            Err(err) => finish(
                &mut report,
                &mut sink,
                StepId::Parse,
                Status::Warn,
                err.to_string(),
                step_started,
            ),
        }
    }

    if opts.gitignore {
        sink(Event::Started(StepId::Gitignore));
        let step_started = Instant::now();
        match write::append_gitignore(
            &plan.dir,
            &[".direnv/", ".nixdev.tmp.*", "flake.nix.nixdev-backup-*"],
        ) {
            Ok(true) => {
                report.gitignore_updated = true;
                finish(
                    &mut report,
                    &mut sink,
                    StepId::Gitignore,
                    Status::Ok,
                    "added nixdev entries".into(),
                    step_started,
                );
            }
            Ok(false) => finish(
                &mut report,
                &mut sink,
                StepId::Gitignore,
                Status::Skipped,
                "already up to date".into(),
                step_started,
            ),
            Err(err) => finish(
                &mut report,
                &mut sink,
                StepId::Gitignore,
                Status::Warn,
                err,
                step_started,
            ),
        }
    }

    if opts.envrc {
        sink(Event::Started(StepId::Envrc));
        let step_started = Instant::now();
        match write::write_envrc(&plan.dir) {
            Ok(true) => {
                report.envrc_written = true;
                finish(
                    &mut report,
                    &mut sink,
                    StepId::Envrc,
                    Status::Ok,
                    "direnv will load the shell automatically".into(),
                    step_started,
                );
            }
            Ok(false) => finish(
                &mut report,
                &mut sink,
                StepId::Envrc,
                Status::Skipped,
                "kept the existing .envrc".into(),
                step_started,
            ),
            Err(err) => finish(
                &mut report,
                &mut sink,
                StepId::Envrc,
                Status::Warn,
                err,
                step_started,
            ),
        }
    }

    if opts.git {
        sink(Event::Started(StepId::Git));
        let step_started = Instant::now();
        match write::git_init(&plan.dir) {
            Ok(true) => {
                report.git_initialized = true;
                finish(
                    &mut report,
                    &mut sink,
                    StepId::Git,
                    Status::Ok,
                    "created .git".into(),
                    step_started,
                );
            }
            Ok(false) => finish(
                &mut report,
                &mut sink,
                StepId::Git,
                Status::Skipped,
                "already inside a git repository".into(),
                step_started,
            ),
            Err(err) => finish(
                &mut report,
                &mut sink,
                StepId::Git,
                Status::Warn,
                err,
                step_started,
            ),
        }
    }

    if opts.lock {
        sink(Event::Started(StepId::Lock));
        let step_started = Instant::now();
        let staged = write::intent_to_add(&plan.dir, "flake.nix");
        match nix::flake_lock(&plan.dir) {
            Ok(()) => finish(
                &mut report,
                &mut sink,
                StepId::Lock,
                Status::Ok,
                "pinned the inputs for reproducible shells".into(),
                step_started,
            ),
            Err(NixError::Canceled) => return Err("cancelled".into()),
            Err(err) => {
                let hint = match &staged {
                    Err(stage_err) => format!("; {stage_err}"),
                    Ok(()) => String::new(),
                };
                finish(
                    &mut report,
                    &mut sink,
                    StepId::Lock,
                    Status::Warn,
                    format!("{err}{hint}; run `nix flake lock` later"),
                    step_started,
                )
            }
        }
    }

    if opts.format {
        sink(Event::Started(StepId::Format));
        let step_started = Instant::now();
        match nix::format_file(&path) {
            Ok(Some(formatter)) => {
                let reformatted = nix::parse_file(&path);
                if matches!(reformatted, Ok(Some(_))) {
                    let _ = fs::write(&path, &contents);
                    finish(
                        &mut report,
                        &mut sink,
                        StepId::Format,
                        Status::Warn,
                        format!("{formatter} produced invalid output; kept the plain version"),
                        step_started,
                    );
                } else {
                    finish(
                        &mut report,
                        &mut sink,
                        StepId::Format,
                        Status::Ok,
                        format!("formatted with {formatter}"),
                        step_started,
                    );
                }
            }
            Ok(None) => finish(
                &mut report,
                &mut sink,
                StepId::Format,
                Status::Skipped,
                "install nixfmt or alejandra to enable this".into(),
                step_started,
            ),
            Err(NixError::Canceled) => return Err("cancelled".into()),
            Err(err) => finish(
                &mut report,
                &mut sink,
                StepId::Format,
                Status::Warn,
                err.to_string(),
                step_started,
            ),
        }
    }

    if opts.check {
        sink(Event::Started(StepId::Check));
        let step_started = Instant::now();
        match nix::flake_check(&plan.dir) {
            Ok(()) => finish(
                &mut report,
                &mut sink,
                StepId::Check,
                Status::Ok,
                "flake evaluates cleanly".into(),
                step_started,
            ),
            Err(NixError::Canceled) => return Err("cancelled".into()),
            Err(err) => finish(
                &mut report,
                &mut sink,
                StepId::Check,
                Status::Warn,
                err.to_string(),
                step_started,
            ),
        }
    }

    report.elapsed = started.elapsed();
    Ok(report)
}

fn finish<F: FnMut(Event)>(
    report: &mut Report,
    sink: &mut F,
    id: StepId,
    status: Status,
    detail: String,
    started: Instant,
) {
    let step = StepResult {
        id,
        status,
        detail,
        elapsed: started.elapsed(),
    };
    report.steps.push(step.clone());
    sink(Event::Finished(step));
}

fn rollback(path: &Path, previous: Option<&str>, existed: bool) {
    match previous {
        Some(text) => {
            let _ = fs::write(path, text);
        }
        None if !existed => {
            let _ = fs::remove_file(path);
        }
        None => {}
    }
}

fn first_line(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("invalid syntax")
        .to_string()
}
