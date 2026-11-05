use std::io::{self, IsTerminal, Write};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};

use crate::index::{self, Entry, Index};
use crate::nix;
use crate::output::Out;
use crate::pipeline::{self, Options, Report, StepId};
use crate::plan::Plan;
use crate::profile;
use crate::theme;
use crate::util::{self, truncate};

const LOGO: [&str; 6] = [
    " ███╗   ██╗██╗██╗  ██╗██████╗ ███████╗██╗   ██╗",
    " ████╗  ██║██║╚██╗██╔╝██╔══██╗██╔════╝██║   ██║",
    " ██╔██╗ ██║██║ ╚███╔╝ ██║  ██║█████╗  ██║   ██║",
    " ██║╚██╗██║██║ ██╔██╗ ██║  ██║██╔══╝  ╚██╗ ██╔╝",
    " ██║ ╚████║██║██╔╝ ██╗██████╔╝███████╗ ╚████╔╝ ",
    " ╚═╝  ╚═══╝╚═╝╚═╝  ╚═╝╚═════╝ ╚══════╝  ╚═══╝ ",
];

type Item = (String, String, String);

pub fn run(
    mut plan: Plan,
    opts: Options,
    out: &Out,
    use_index: bool,
    validate: bool,
) -> Result<(), String> {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Err("interactive setup needs a terminal; pass --yes for a scripted run".into());
    }

    banner(out);

    let old_name = plan.name.clone();
    plan.name = util::sanitize_project_name(&input(out, "Project name", &plan.name)?);
    if plan.description == format!("{old_name} development environment") {
        plan.description = format!("{} development environment", plan.name);
    }
    plan.description = input(out, "Description", &plan.description)?;

    plan.languages = multi(
        out,
        "Languages",
        &profile::PROFILES
            .iter()
            .map(|p| (p.id.to_string(), p.label.to_string(), p.blurb.to_string()))
            .collect::<Vec<_>>(),
        &plan.languages,
        true,
    )?;

    plan.extras = multi(
        out,
        "Optional tools",
        &profile::EXTRAS
            .iter()
            .map(|e| {
                (
                    e.id.to_string(),
                    e.label.to_string(),
                    format!("{} · {}", e.category, e.blurb),
                )
            })
            .collect::<Vec<_>>(),
        &plan.extras,
        false,
    )?;

    plan.packages = packages(out, &plan, use_index)?;

    plan.systems = multi(
        out,
        "Systems",
        &[
            (
                "x86_64-linux".to_string(),
                "x86_64-linux".to_string(),
                "Intel/AMD Linux".to_string(),
            ),
            (
                "aarch64-linux".to_string(),
                "aarch64-linux".to_string(),
                "ARM64 Linux".to_string(),
            ),
            (
                "x86_64-darwin".to_string(),
                "x86_64-darwin".to_string(),
                "Intel macOS".to_string(),
            ),
            (
                "aarch64-darwin".to_string(),
                "aarch64-darwin".to_string(),
                "Apple Silicon macOS".to_string(),
            ),
            (
                "i686-linux".to_string(),
                "i686-linux".to_string(),
                "32-bit Linux".to_string(),
            ),
            (
                "armv7l-linux".to_string(),
                "armv7l-linux".to_string(),
                "32-bit ARM Linux".to_string(),
            ),
        ],
        &plan.systems,
        true,
    )?;

    let mut shells: Vec<Item> = profile::SHELLS
        .iter()
        .map(|shell| {
            let hint = if *shell == profile::detect_shell() {
                "your login shell".to_string()
            } else {
                String::new()
            };
            (shell.to_string(), shell.to_string(), hint)
        })
        .collect();
    shells.push((
        "none".to_string(),
        "none".to_string(),
        "stay in bash".to_string(),
    ));
    plan.shell = choose(out, "Shell to enter", &shells, &plan.shell)?;

    plan = plan.normalize()?;
    let exists = plan.dir.join("flake.nix").exists();
    summary(out, &plan, &opts, exists);

    if !confirm(out, "Write the flake?", true)? {
        out.info("nothing was written");
        return Ok(());
    }

    let mut opts = opts;
    if exists {
        opts.allow_overwrite = true;
    }
    let report = generate(out, &plan, &opts, validate)?;
    out.report(&report);
    celebrate(out);
    out.next_steps(&plan.dir);
    Ok(())
}

fn paint(out: &Out, rgb: (u8, u8, u8), text: &str) -> String {
    theme::ansi_fg(out.mode(), rgb, text)
}

fn bold(out: &Out, text: &str) -> String {
    theme::ansi_bold(out.mode(), text)
}

fn dim(out: &Out, text: &str) -> String {
    theme::ansi_dim(out.mode(), text)
}

fn banner(out: &Out) {
    let width = term_width();
    println!();
    if width >= 60 {
        for (row, line) in LOGO.iter().enumerate() {
            let mut painted = String::new();
            for (column, ch) in line.chars().enumerate() {
                let hue = 205.0 + row as f32 * 8.0 + column as f32 * 2.6;
                painted.push_str(&theme::ansi_fg(
                    out.mode(),
                    theme::hsl(hue, 0.65, 0.68),
                    &ch.to_string(),
                ));
            }
            println!("{painted}");
        }
    } else {
        let mut painted = String::new();
        for (index, ch) in "nixdev".chars().enumerate() {
            let hue = 205.0 + index as f32 * 9.0;
            painted.push_str(&theme::ansi_fg(
                out.mode(),
                theme::hsl(hue, 0.65, 0.68),
                &ch.to_string(),
            ));
        }
        println!("  {painted}");
    }
    println!("  {}", dim(out, "a dev shell, without the guesswork"));
    println!();
}

fn term_width() -> usize {
    crossterm::terminal::size()
        .ok()
        .map(|(width, _)| width as usize)
        .or_else(|| {
            std::env::var("COLUMNS")
                .ok()
                .and_then(|columns| columns.parse().ok())
        })
        .unwrap_or(80)
        .clamp(32, 120)
}

fn read_line(out: &Out, label: &str, hint: Option<&str>) -> Result<String, String> {
    print!("  {} {}", paint(out, theme::ACCENT, "?"), bold(out, label));
    if let Some(hint) = hint {
        print!(" {}", dim(out, &format!("({hint})")));
    }
    print!(" ");
    io::stdout()
        .flush()
        .map_err(|err| format!("could not write the prompt: {err}"))?;

    let mut line = String::new();
    let read = io::stdin()
        .read_line(&mut line)
        .map_err(|err| format!("could not read your answer: {err}"))?;
    if read == 0 {
        return Err("cancelled".into());
    }
    Ok(line.trim().to_string())
}

fn input(out: &Out, label: &str, default_value: &str) -> Result<String, String> {
    let value = read_line(out, label, Some(default_value))?;
    Ok(if value.is_empty() {
        default_value.to_string()
    } else {
        value
    })
}

fn confirm(out: &Out, label: &str, default_value: bool) -> Result<bool, String> {
    let hint = if default_value { "Y/n" } else { "y/N" };
    loop {
        let answer = read_line(out, label, Some(hint))?.to_lowercase();
        match answer.as_str() {
            "" => return Ok(default_value),
            "y" | "yes" => return Ok(true),
            "n" | "no" => return Ok(false),
            _ => out.warn("answer y or n"),
        }
    }
}

struct RawMode;

impl RawMode {
    fn new() -> Result<Self, String> {
        enable_raw_mode().map_err(|err| format!("could not take over the keyboard: {err}"))?;
        Ok(Self)
    }
}

impl Drop for RawMode {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
    }
}

fn read_key() -> Result<KeyCode, String> {
    loop {
        match event::read().map_err(|err| format!("could not read the keyboard: {err}"))? {
            Event::Key(key) if key.kind != KeyEventKind::Release => {
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && matches!(key.code, KeyCode::Char('c') | KeyCode::Char('d'))
                {
                    return Err("cancelled".into());
                }
                return Ok(key.code);
            }
            _ => continue,
        }
    }
}

fn draw_items(
    out: &Out,
    items: &[Item],
    cursor: usize,
    selected: Option<&[bool]>,
    drawn: usize,
) -> Result<usize, String> {
    let mut stdout = io::stdout();
    if drawn > 0 {
        write!(stdout, "\x1b[{drawn}A").map_err(io_error)?;
    }
    let width = term_width();
    let visible = items.len().min(12);
    let start = (cursor + 1).saturating_sub(visible);
    for (offset, item) in items.iter().enumerate().skip(start).take(visible) {
        let here = offset == cursor;
        let marker = if here {
            paint(out, theme::ACCENT, "❯ ")
        } else {
            "  ".to_string()
        };
        let check = match selected {
            Some(flags) if flags[offset] => paint(out, theme::SUCCESS, "[x] "),
            Some(_) => dim(out, "[ ] "),
            None => String::new(),
        };
        let checked = selected.is_some_and(|flags| flags[offset]);
        let label = format!("{:<24}", truncate(&item.1, 24));
        let label = if here {
            paint(out, theme::TEXT, &bold(out, &label))
        } else if checked {
            paint(out, theme::TEXT, &label)
        } else {
            paint(out, theme::MUTED, &label)
        };
        let hint = truncate(&item.2, width.saturating_sub(34));
        writeln!(
            stdout,
            "\r\x1b[2K  {marker}{check}{label}  {}",
            dim(out, &hint)
        )
        .map_err(io_error)?;
    }
    stdout.flush().map_err(io_error)?;
    Ok(visible)
}

fn clear_items(drawn: usize) -> Result<(), String> {
    let mut stdout = io::stdout();
    if drawn == 0 {
        return Ok(());
    }
    write!(stdout, "\x1b[{drawn}A").map_err(io_error)?;
    for _ in 0..drawn {
        write!(stdout, "\r\x1b[2K\n").map_err(io_error)?;
    }
    write!(stdout, "\x1b[{drawn}A").map_err(io_error)?;
    stdout.flush().map_err(io_error)?;
    Ok(())
}

fn io_error(err: io::Error) -> String {
    format!("could not draw the prompt: {err}")
}

fn selected_labels(items: &[Item], ids: &[String]) -> String {
    let labels: Vec<&str> = items
        .iter()
        .filter(|item| ids.contains(&item.0))
        .map(|item| item.1.as_str())
        .collect();
    if labels.is_empty() {
        "none".to_string()
    } else {
        labels.join(", ")
    }
}

fn multi(
    out: &Out,
    label: &str,
    items: &[Item],
    defaults: &[String],
    require_one: bool,
) -> Result<Vec<String>, String> {
    let mut selected: Vec<bool> = items
        .iter()
        .map(|item| defaults.contains(&item.0))
        .collect();
    let mut cursor = selected.iter().position(|flag| *flag).unwrap_or(0);
    let mut drawn = 0usize;

    println!(
        "  {} {}  {}",
        paint(out, theme::ACCENT, "?"),
        bold(out, label),
        dim(out, "space toggle · enter confirm")
    );
    let raw = RawMode::new()?;
    loop {
        drawn = draw_items(out, items, cursor, Some(&selected), drawn)?;
        match read_key()? {
            KeyCode::Up | KeyCode::Char('k') => {
                cursor = if cursor == 0 {
                    items.len() - 1
                } else {
                    cursor - 1
                };
            }
            KeyCode::Down | KeyCode::Char('j') => {
                cursor = if cursor + 1 == items.len() {
                    0
                } else {
                    cursor + 1
                };
            }
            KeyCode::Char(' ') => selected[cursor] = !selected[cursor],
            KeyCode::Enter => {
                if require_one && selected.iter().all(|flag| !*flag) {
                    selected[cursor] = true;
                }
                break;
            }
            KeyCode::Esc => {
                drop(raw);
                clear_items(drawn)?;
                return Err("cancelled".into());
            }
            _ => {}
        }
    }
    drop(raw);
    clear_items(drawn)?;

    let ids: Vec<String> = items
        .iter()
        .zip(&selected)
        .filter(|(_, flag)| **flag)
        .map(|(item, _)| item.0.clone())
        .collect();
    println!(
        "  {} {}  {}",
        paint(out, theme::SUCCESS, "✔"),
        bold(out, label),
        dim(out, &selected_labels(items, &ids))
    );
    Ok(ids)
}

fn choose(out: &Out, label: &str, items: &[Item], default_value: &str) -> Result<String, String> {
    let mut cursor = items
        .iter()
        .position(|item| item.0 == default_value)
        .unwrap_or(0);
    let mut drawn = 0usize;

    println!(
        "  {} {}  {}",
        paint(out, theme::ACCENT, "?"),
        bold(out, label),
        dim(out, "↑/↓ move · enter select")
    );
    let raw = RawMode::new()?;
    loop {
        drawn = draw_items(out, items, cursor, None, drawn)?;
        match read_key()? {
            KeyCode::Up | KeyCode::Char('k') => {
                cursor = if cursor == 0 {
                    items.len() - 1
                } else {
                    cursor - 1
                };
            }
            KeyCode::Down | KeyCode::Char('j') => {
                cursor = if cursor + 1 == items.len() {
                    0
                } else {
                    cursor + 1
                };
            }
            KeyCode::Enter => break,
            KeyCode::Esc => {
                drop(raw);
                clear_items(drawn)?;
                return Err("cancelled".into());
            }
            _ => {}
        }
    }
    drop(raw);
    clear_items(drawn)?;

    let item = &items[cursor];
    println!(
        "  {} {}  {}",
        paint(out, theme::SUCCESS, "✔"),
        bold(out, label),
        dim(out, &item.1)
    );
    Ok(item.0.clone())
}

fn packages(out: &Out, plan: &Plan, use_index: bool) -> Result<Vec<String>, String> {
    let mut added = plan.packages.clone();
    if !added.is_empty() {
        out.info(&format!("already included: {}", added.join(", ")));
    }

    let system = nix::current_system();
    let mut index: Option<Index> = None;
    let mut live = true;
    if use_index && !plan.rev.is_empty() {
        if let Some(cached) = index::load(&plan.rev, &system) {
            index = Some(cached);
            live = false;
        } else {
            let rev = plan.rev.clone();
            match out.spin("indexing nixpkgs (one time)", || {
                index::load_or_build(&rev, &system)
            }) {
                Ok(built) => {
                    index = Some(built);
                    live = false;
                }
                Err(err) => {
                    out.warn(&format!("package index unavailable: {err}"));
                }
            }
        }
    }

    loop {
        let query = read_line(out, "Search nixpkgs", Some("blank to finish"))?;
        if query.is_empty() {
            break;
        }

        let mut hits: Vec<Entry> = match (&index, live) {
            (Some(index), false) => index.search(&query, 8).into_iter().cloned().collect(),
            _ => Vec::new(),
        };
        if hits.is_empty() {
            let term = query.clone();
            let found = out.spin(&format!("searching nixpkgs for {term}"), || {
                nix::live_search(&term, Duration::from_secs(300))
            });
            match found {
                Ok(entries) => {
                    hits = entries.into_iter().take(8).collect();
                    live = true;
                }
                Err(err) => {
                    out.warn(&format!("search failed: {err}"));
                    continue;
                }
            }
        }

        if hits.is_empty() {
            out.info("no packages matched");
            continue;
        }

        println!();
        for (position, entry) in hits.iter().enumerate() {
            println!(
                "  {} {} {} {}",
                dim(out, &format!("{:>2}.", position + 1)),
                paint(
                    out,
                    theme::ACCENT,
                    &format!("{:<32}", truncate(&entry.attr, 32))
                ),
                dim(out, &format!("{:<10}", truncate(&entry.version, 10))),
                dim(
                    out,
                    &truncate(&entry.description, term_width().saturating_sub(52))
                )
            );
        }
        println!();

        let pick = read_line(out, "Add which number", Some("blank to search again"))?;
        if pick.is_empty() {
            continue;
        }
        match pick.parse::<usize>() {
            Ok(number) if number >= 1 && number <= hits.len() => {
                let attr = hits[number - 1].attr.clone();
                if added.contains(&attr) {
                    out.info(&format!("{attr} is already in the list"));
                } else {
                    out.info(&format!("added {}", paint(out, theme::SUCCESS, &attr)));
                    added.push(attr);
                }
            }
            _ => out.warn("that is not one of the numbers above"),
        }
    }
    Ok(added)
}

fn summary(out: &Out, plan: &Plan, opts: &Options, exists: bool) {
    let width = term_width();
    let row = |label: &str, value: &str| {
        println!(
            "  {} {}",
            dim(out, &format!("{label:<12}")),
            truncate(value, width.saturating_sub(14))
        );
    };
    println!();
    println!("  {}", bold(out, "Summary"));
    row("project", &plan.name);
    row("description", &plan.description);
    let languages = plan.language_labels();
    row(
        "languages",
        &if languages.is_empty() {
            "none".to_string()
        } else {
            languages.join(", ")
        },
    );
    if !plan.extras.is_empty() {
        let labels: Vec<&str> = plan
            .extras
            .iter()
            .filter_map(|id| profile::extra(id).map(|extra| extra.label))
            .collect();
        row("tools", &labels.join(", "));
    }
    row("packages", &plan.to_spec().packages.len().to_string());
    row("systems", &plan.systems.join(", "));
    row("shell", &plan.shell);

    let mut files = vec!["flake.nix"];
    if opts.gitignore {
        files.push(".gitignore");
    }
    if opts.envrc {
        files.push(".envrc");
    }
    if opts.lock {
        files.push("flake.lock");
    }
    row("files", &files.join(", "));
    if exists {
        row(
            "existing",
            if opts.backup {
                "flake.nix will be replaced, a backup is kept"
            } else {
                "flake.nix will be replaced without a backup"
            },
        );
    }
    println!();
}

fn total_steps(opts: &Options) -> usize {
    let side = [
        opts.gitignore,
        opts.envrc,
        opts.git,
        opts.lock,
        opts.format,
        opts.check,
    ];
    2 + side.iter().filter(|enabled| **enabled).count()
}

enum Message {
    Event(pipeline::Event),
    Done(Result<Report, String>),
}

fn generate(out: &Out, plan: &Plan, opts: &Options, validate: bool) -> Result<Report, String> {
    if validate {
        let warnings = out.spin("checking packages", || crate::plan::package_warnings(plan));
        for warning in warnings {
            out.warn(&warning);
        }
    }

    let total = total_steps(opts);
    let (tx, rx) = mpsc::channel();
    let worker_plan = plan.clone();
    let worker_opts = opts.clone();
    thread::spawn(move || {
        let result = pipeline::run(&worker_plan, &worker_opts, |event| {
            let _ = tx.send(Message::Event(event));
        });
        let _ = tx.send(Message::Done(result));
    });

    let mut finished = 0usize;
    let mut current: Option<StepId> = None;
    let mut frame = 0usize;
    let mut status_drawn = false;
    loop {
        match rx.recv_timeout(Duration::from_millis(80)) {
            Ok(Message::Event(pipeline::Event::Started(step))) => {
                current = Some(step);
                draw_status(out, current, finished, total, frame);
                status_drawn = true;
            }
            Ok(Message::Event(pipeline::Event::Finished(step))) => {
                out.finished(&step);
                finished += 1;
                current = None;
                status_drawn = false;
            }
            Ok(Message::Done(result)) => {
                if status_drawn {
                    out.clear_line();
                }
                return result;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                frame += 1;
                draw_status(out, current, finished, total, frame);
                status_drawn = true;
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Err("generation stopped unexpectedly".into());
            }
        }
    }
}

fn draw_status(out: &Out, current: Option<StepId>, finished: usize, total: usize, frame: usize) {
    const FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
    let label = current.map(StepId::label).unwrap_or("working");
    let ratio = finished as f32 / total.max(1) as f32;
    let width = 22usize;
    let filled = (ratio * width as f32).round() as usize;
    let bar = format!(
        "{}{}",
        paint(out, theme::SUCCESS, &"█".repeat(filled)),
        dim(out, &"░".repeat(width - filled))
    );
    print!(
        "\r\x1b[2K  {} {}  {}  {}/{}",
        paint(out, theme::ACCENT2, FRAMES[frame % FRAMES.len()]),
        bold(out, label),
        bar,
        finished + usize::from(current.is_some()),
        total
    );
    let _ = io::stdout().flush();
}

fn celebrate(out: &Out) {
    if !out.is_interactive() || !out.mode().colored() {
        return;
    }
    const ROWS: usize = 3;
    let width = term_width().min(72);
    for _ in 0..ROWS {
        println!();
    }

    let glyphs = ['✦', '✧', '◆', '●', '★', '•'];
    let palette = [
        theme::ACCENT,
        theme::ACCENT2,
        theme::SUCCESS,
        theme::WARN,
        theme::ERROR,
    ];
    let mut seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos() as u64)
        .unwrap_or(0x9e37_79b9_7f4a_7c15)
        | 1;
    let mut random = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed >> 33) as f32 / (1u64 << 31) as f32
    };

    let mut stdout = io::stdout();
    for _ in 0..12 {
        let _ = write!(stdout, "\x1b[{ROWS}A");
        for _ in 0..ROWS {
            let mut line = String::new();
            for _ in 0..width {
                if random() < 0.055 {
                    let glyph = glyphs[(random() * glyphs.len() as f32) as usize % glyphs.len()];
                    let color = palette[(random() * palette.len() as f32) as usize % palette.len()];
                    line.push_str(&theme::ansi_fg(out.mode(), color, &glyph.to_string()));
                } else {
                    line.push(' ');
                }
            }
            let _ = write!(stdout, "\r\x1b[2K{line}\n");
        }
        let _ = stdout.flush();
        thread::sleep(Duration::from_millis(55));
    }

    let _ = write!(stdout, "\x1b[{ROWS}A");
    let text = "✦   happy hacking   ✦";
    let padding = width.saturating_sub(text.chars().count()) / 2;
    let mut line = " ".repeat(padding);
    for (index, ch) in text.chars().enumerate() {
        let hue = 205.0 + index as f32 * 11.0;
        line.push_str(&theme::ansi_fg(
            out.mode(),
            theme::hsl(hue, 0.62, 0.68),
            &ch.to_string(),
        ));
    }
    let _ = write!(stdout, "\r\x1b[2K\n");
    let _ = write!(stdout, "\r\x1b[2K{line}\n");
    let _ = write!(stdout, "\r\x1b[2K\n");
    let _ = stdout.flush();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_only_enabled_steps() {
        let mut opts = Options::default();
        assert_eq!(total_steps(&opts), 7);
        opts.check = true;
        assert_eq!(total_steps(&opts), 8);
        opts.lock = false;
        assert_eq!(total_steps(&opts), 7);
    }

    #[test]
    fn summaries_use_friendly_names() {
        let items = vec![
            ("rust".to_string(), "Rust".to_string(), String::new()),
            ("c".to_string(), "C".to_string(), String::new()),
        ];
        assert_eq!(selected_labels(&items, &["c".to_string()]), "C");
        assert_eq!(selected_labels(&items, &[]), "none");
    }
}
