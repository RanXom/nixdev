use std::io::{self, IsTerminal, Write};
use std::time::Duration;

use crate::pipeline::{Report, Status, StepResult};
use crate::plan::Plan;
use crate::theme::{self, ColorMode};

pub struct Out {
    mode: ColorMode,
    interactive: bool,
}

impl Out {
    pub fn new(mode: ColorMode) -> Self {
        Self {
            mode,
            interactive: io::stdout().is_terminal(),
        }
    }

    pub fn mode(&self) -> ColorMode {
        self.mode
    }

    pub fn is_interactive(&self) -> bool {
        self.interactive
    }

    fn paint(&self, rgb: (u8, u8, u8), text: &str) -> String {
        theme::ansi_fg(self.mode, rgb, text)
    }

    fn bold(&self, text: &str) -> String {
        theme::ansi_bold(self.mode, text)
    }

    fn dim(&self, text: &str) -> String {
        theme::ansi_dim(self.mode, text)
    }

    pub fn banner(&self) {
        if !self.interactive {
            return;
        }
        println!();
        println!(
            "  {}",
            format_args!(
                "{}  {}",
                self.bold(&self.paint(theme::ACCENT, "nixdev")),
                self.dim("dev shells without the guesswork")
            )
        );
        println!();
    }

    pub fn plan_summary(&self, plan: &Plan) {
        let label = |text: &str| self.dim(&format!("{text:<10}"));
        println!("  {}{}", label("project"), plan.name);
        println!("  {}{}", label("shell"), plan.shell);
        println!("  {}{}", label("systems"), plan.systems.join(", "));
        let languages = plan.language_labels();
        if !languages.is_empty() {
            println!("  {}{}", label("languages"), languages.join(", "));
        }
        let packages = plan.to_spec().packages.len();
        if packages > 0 {
            println!("  {}{}", label("packages"), packages);
        }
        println!();
    }

    pub fn started(&self, label: &str) {
        if !self.interactive {
            return;
        }
        print!("  {} {label}…", self.dim("·"));
        let _ = io::stdout().flush();
    }

    pub fn finished(&self, step: &StepResult) {
        if self.interactive {
            print!("\r\x1b[2K");
        }
        let (symbol, color) = match step.status {
            Status::Ok => ("✓", theme::SUCCESS),
            Status::Skipped => ("–", theme::MUTED),
            Status::Warn => ("!", theme::WARN),
            Status::Failed => ("✗", theme::ERROR),
        };
        println!(
            "  {} {:<26} {}",
            self.paint(color, symbol),
            step.id.label(),
            self.dim(&format!(
                "{}{}",
                step.detail,
                if step.elapsed >= Duration::from_millis(800) {
                    format!(" ({})", format_duration(step.elapsed))
                } else {
                    String::new()
                }
            ))
        );
    }

    pub fn info(&self, message: &str) {
        println!("  {} {message}", self.paint(theme::ACCENT, "•"));
    }

    pub fn clear_line(&self) {
        if self.interactive {
            print!("\r\x1b[2K");
            let _ = io::stdout().flush();
        }
    }

    pub fn warn(&self, message: &str) {
        println!("  {} {message}", self.paint(theme::WARN, "!"));
    }

    pub fn error(&self, message: &str) {
        println!("  {} {message}", self.paint(theme::ERROR, "✗"));
    }

    pub fn fail(&self, message: &str) {
        eprintln!("  {} {message}", self.paint(theme::ERROR, "✗"));
    }

    pub fn report(&self, report: &Report) {
        println!();
        println!(
            "  {} {}",
            self.paint(theme::SUCCESS, "flake ready"),
            self.dim(&format!("in {}", format_duration(report.elapsed)))
        );
        println!("  {}{}", self.dim("file      "), report.path.display());
        if let Some(backup) = &report.backup {
            println!("  {}{}", self.dim("backup    "), backup.display());
        }
        if report.git_initialized {
            println!("  {}{}", self.dim("git       "), "repository created");
        }
        println!(
            "  {}{}",
            self.dim("next      "),
            format!("cd {} && nix develop", report.path.parent().unwrap_or(&report.path).display())
        );
        println!();
    }

    pub fn preview(&self, text: &str) {
        let rule = "─".repeat(72);
        println!("  {}", self.dim(&rule));
        for line in text.lines() {
            println!("  {}", highlight_line(self.mode, line));
        }
        println!("  {}", self.dim(&rule));
        println!();
    }

    pub fn next_steps(&self, dir: &std::path::Path) {
        println!(
            "  {}",
            self.dim(&format!(
                "hint: run `cd {} && nix develop`, or `direnv allow` if you use direnv",
                dir.display()
            ))
        );
    }
}

pub fn format_duration(duration: Duration) -> String {
    let secs = duration.as_secs_f64();
    if secs < 1.0 {
        format!("{}ms", duration.as_millis())
    } else if secs < 60.0 {
        format!("{secs:.1}s")
    } else {
        format!("{}m{:02}s", duration.as_secs() / 60, duration.as_secs() % 60)
    }
}

fn highlight_line(mode: ColorMode, line: &str) -> String {
    if !mode.colored() {
        return line.to_string();
    }
    let trimmed = line.trim_start();
    if trimmed.starts_with('#') {
        return theme::ansi_fg(mode, theme::MUTED, line);
    }
    let mut out = String::with_capacity(line.len());
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        if ch == '"' {
            let start = i;
            i += 1;
            while i < chars.len() {
                if chars[i] == '\\' {
                    i += 2;
                    continue;
                }
                if chars[i] == '"' {
                    i += 1;
                    break;
                }
                i += 1;
            }
            let literal: String = chars[start..i].iter().collect();
            out.push_str(&theme::ansi_fg(mode, theme::WARN, &literal));
            continue;
        }
        if ch.is_ascii_digit() {
            out.push_str(&theme::ansi_fg(mode, theme::ACCENT2, &ch.to_string()));
        } else {
            out.push(ch);
        }
        i += 1;
    }
    out
}
