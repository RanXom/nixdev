use unicode_width::UnicodeWidthStr;

use crate::util::{truncate, valid_attr, valid_env_key};

pub const MANAGED_START: &str = "# >>> nixdev:managed >>>";
pub const MANAGED_END: &str = "# <<< nixdev:managed <<<";

#[derive(Clone, Default)]
pub struct Meta {
    pub project: String,
    pub description: String,
    pub languages: Vec<String>,
    pub shell: String,
    pub generated: String,
}

#[derive(Clone)]
pub struct Spec {
    pub meta: Meta,
    pub nixpkgs_url: String,
    pub nixpkgs_note: String,
    pub systems: Vec<String>,
    pub packages: Vec<String>,
    pub env: Vec<(String, String)>,
    pub shell_pkg: Option<String>,
}

impl Spec {
    pub fn validate(&self) -> Result<(), String> {
        if self.systems.is_empty() {
            return Err("at least one system must be selected".into());
        }
        for system in &self.systems {
            if !system
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
            {
                return Err(format!("invalid system `{system}`"));
            }
        }
        if self.nixpkgs_url.trim().is_empty()
            || self.nixpkgs_url.contains('"')
            || self.nixpkgs_url.chars().any(char::is_control)
        {
            return Err("invalid nixpkgs input url".into());
        }
        for pkg in &self.packages {
            if !valid_attr(pkg) {
                return Err(format!("`{pkg}` is not a valid Nix attribute path"));
            }
        }
        for (key, value) in &self.env {
            if !valid_env_key(key) {
                return Err(format!("`{key}` is not a valid environment variable"));
            }
            if value.trim().is_empty() {
                return Err(format!("environment variable `{key}` has no value"));
            }
        }
        Ok(())
    }
}

pub fn render(spec: &Spec) -> String {
    let mut out = String::with_capacity(8192);
    let title = if spec.meta.description.trim().is_empty() {
        format!("{} development environment", spec.meta.project)
    } else {
        spec.meta.description.trim().to_string()
    };

    out.push_str(&header(spec, &title));

    out.push_str("{\n");
    out.push_str(&format!(
        "  description = \"{}\";\n\n",
        escape_string(&title)
    ));

    out.push_str("  inputs = {\n");
    if !spec.nixpkgs_note.is_empty() {
        out.push_str(&format!("    # {}\n", flat(&spec.nixpkgs_note)));
    }
    out.push_str(&format!(
        "    nixpkgs.url = \"{}\";\n",
        escape_string(&spec.nixpkgs_url)
    ));
    out.push_str("  };\n\n");

    out.push_str("  outputs =\n");
    out.push_str("    { nixpkgs, ... }:\n");
    out.push_str("    let\n");
    out.push_str("      systems = [\n");
    for system in &spec.systems {
        out.push_str(&format!("        \"{system}\"\n"));
    }
    out.push_str("      ];\n");
    out.push_str("      eachSystem = nixpkgs.lib.genAttrs systems;\n");
    out.push_str("    in\n");
    out.push_str("    {\n");
    out.push_str("      devShells = eachSystem (\n");
    out.push_str("        system:\n");
    out.push_str("        let\n");
    out.push_str("          pkgs = nixpkgs.legacyPackages.${system};\n");
    out.push_str("        in\n");
    out.push_str("        {\n");
    out.push_str("          default = pkgs.mkShell {\n");
    out.push_str("            packages = with pkgs; [\n");
    out.push_str(&format!("              {MANAGED_START}\n"));
    for pkg in &spec.packages {
        out.push_str(&format!("              {pkg}\n"));
    }
    out.push_str(&format!("              {MANAGED_END}\n"));
    out.push_str("            ];\n");

    if !spec.env.is_empty() {
        out.push_str("\n            env = {\n");
        for (key, value) in &spec.env {
            out.push_str(&format!("              {key} = {value};\n"));
        }
        out.push_str("            };\n");
    }

    out.push_str(&shell_hook(spec));

    out.push_str("          };\n");
    out.push_str("        }\n");
    out.push_str("      );\n\n");
    out.push_str(
        "      formatter = eachSystem (system: nixpkgs.legacyPackages.${system}.nixfmt);\n",
    );
    out.push_str("    };\n");
    out.push_str("}\n");
    out
}

// ───────────────────────── shell hook ─────────────────────────

/// Inner width of the banner box, in terminal columns.
const WIDTH: usize = 60;

// Escapes are written for bash `$'…'` strings and use the 256-colour palette,
// which every modern terminal understands.
const RST: &str = "\\e[0m";
const BOLD: &str = "\\e[1m";
const DIM: &str = "\\e[2m";
const BOX: &str = "\\e[38;5;60m";
const ACC: &str = "\\e[38;5;111m";
const OK: &str = "\\e[38;5;114m";
const MUTED: &str = "\\e[38;5;245m";
const TEXT: &str = "\\e[38;5;189m";
const TITLE: &str = "\\e[1;38;5;189m";
const HEAD: &str = "\\e[1;38;5;111m";
const GRADIENT: [u8; 6] = [45, 75, 111, 147, 183, 219];

/// Encode text for a bash `$'…'` string. `$`, `'` and `\` can never reach the
/// output unescaped, so neither the shell nor Nix can interpret user text.
fn ansi_c(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '\'' => out.push_str("\\x27"),
            '$' => out.push_str("\\x24"),
            c if c.is_control() => out.push(' '),
            c => out.push(c),
        }
    }
    out
}

/// One banner row: `segments` are (style, text) pairs, padded to the box width.
fn row(segments: &[(&str, &str)]) -> String {
    let avail = WIDTH - 2;
    let mut used = 0usize;
    let mut body = String::new();
    for (style, text) in segments {
        let text = truncate(text, avail.saturating_sub(used));
        used += text.width();
        body.push_str(style);
        body.push_str(&ansi_c(&text));
        body.push_str(RST);
    }
    let pad = " ".repeat(avail.saturating_sub(used));
    format!("nd_out $'  {BOX}│{RST}  {body}{pad}{BOX}│{RST}'")
}

fn top() -> String {
    let version = format!("v{}", env!("CARGO_PKG_VERSION"));
    let title: String = "nixdev"
        .chars()
        .zip(GRADIENT)
        .map(|(ch, color)| format!("\\e[1;38;5;{color}m{ch}"))
        .collect();
    let used = 2 + 6 + 1 + version.width() + 1;
    let dashes = "─".repeat(WIDTH.saturating_sub(used));
    format!("nd_out $'  {BOX}╭─ {title}{RST} {DIM}{version}{RST} {BOX}{dashes}╮{RST}'")
}

fn bottom() -> String {
    let dashes = "─".repeat(WIDTH);
    format!("nd_out $'  {BOX}╰{dashes}╯{RST}'")
}

fn compact(project: &str, detail: &str) -> String {
    format!(
        "nd_out $'  {ACC}◈{RST} {TITLE}{}{RST}  {DIM}{}{RST}'",
        ansi_c(&truncate(project, 24)),
        ansi_c(&truncate(detail, 48))
    )
}

fn add(sh: &mut String, line: &str) {
    sh.push_str(line);
    sh.push('\n');
}

/// Escape a plain script for a Nix indented (`''…''`) string.
fn nix_indented(script: &str) -> String {
    script.replace("''", "'''").replace("${", "''${")
}

fn shell_hook(spec: &Spec) -> String {
    let project = spec.meta.project.as_str();
    let description = if spec.meta.description.trim().is_empty() {
        "development environment"
    } else {
        spec.meta.description.trim()
    };
    let languages = if spec.meta.languages.is_empty() {
        "none".to_string()
    } else {
        spec.meta.languages.join(" · ")
    };
    let systems = spec.systems.join(", ");
    let shell = if spec.meta.shell.is_empty() {
        "default"
    } else {
        spec.meta.shell.as_str()
    };
    let nixpkgs = if spec.nixpkgs_note.is_empty() {
        spec.nixpkgs_url.as_str()
    } else {
        spec.nixpkgs_note.as_str()
    };
    let packages = format!("{} packages", spec.packages.len());

    let mut sh = String::with_capacity(4096);
    add(
        &mut sh,
        "# nixdev: banner and shell hand-off. Runs once, and only for an",
    );
    add(
        &mut sh,
        "# interactive `nix develop` (never `nix develop -c ...` or direnv).",
    );
    add(
        &mut sh,
        r#"if [[ $- == *i* && -t 0 && -t 1 && -z "${NIXDEV_SHELL_ACTIVE:-}" ]]; then"#,
    );
    add(&mut sh, "  export NIXDEV_SHELL_ACTIVE=1");
    add(&mut sh, "");
    add(
        &mut sh,
        "  # Honour NO_COLOR and dumb terminals by stripping SGR codes at print time.",
    );
    add(&mut sh, "  nd_plain=");
    add(
        &mut sh,
        r#"  if [[ -n "${NO_COLOR:-}" || "${TERM:-dumb}" == dumb ]]; then nd_plain=1; fi"#,
    );
    add(&mut sh, "  nd_out() {");
    add(&mut sh, r#"    local s="$1" re=$'\033\\[[0-9;]*m'"#);
    add(&mut sh, r#"    if [[ -n "$nd_plain" ]]; then"#);
    add(
        &mut sh,
        r#"      while [[ "$s" =~ $re ]]; do s="${s//"${BASH_REMATCH[0]}"/}"; done"#,
    );
    add(&mut sh, "    fi");
    add(&mut sh, r#"    printf '%s\n' "$s""#);
    add(&mut sh, "  }");
    add(&mut sh, "");
    add(
        &mut sh,
        r#"  if [[ -z "${NIXDEV_NO_CLEAR:-}" ]]; then printf '\033[2J\033[H'; fi"#,
    );
    add(&mut sh, "");
    add(
        &mut sh,
        r#"  nd_cols=$(stty size 2>/dev/null | cut -d' ' -f2)"#,
    );
    add(&mut sh, r#"  [[ "$nd_cols" =~ ^[0-9]+$ ]] || nd_cols=80"#);
    add(&mut sh, r#"  printf '\n'"#);
    add(&mut sh, "  if (( nd_cols >= 66 )); then");
    add(&mut sh, &format!("    {}", top()));
    add(&mut sh, &format!("    {}", row(&[])));
    add(
        &mut sh,
        &format!("    {}", row(&[(ACC, "◈  "), (TITLE, project)])),
    );
    add(
        &mut sh,
        &format!("    {}", row(&[(DIM, "   "), (DIM, description)])),
    );
    add(&mut sh, &format!("    {}", row(&[])));
    add(&mut sh, &format!("    {}", row(&[(HEAD, "ENVIRONMENT")])));
    add(
        &mut sh,
        &format!(
            "    {}",
            row(&[
                (BOX, "├─ "),
                (MUTED, "system    "),
                (TEXT, systems.as_str())
            ])
        ),
    );
    add(
        &mut sh,
        &format!(
            "    {}",
            row(&[(BOX, "├─ "), (MUTED, "nixpkgs   "), (TEXT, nixpkgs)])
        ),
    );
    add(
        &mut sh,
        &format!(
            "    {}",
            row(&[(BOX, "└─ "), (MUTED, "shell     "), (TEXT, shell)])
        ),
    );
    add(&mut sh, &format!("    {}", row(&[])));
    add(&mut sh, &format!("    {}", row(&[(HEAD, "TOOLCHAIN")])));
    add(
        &mut sh,
        &format!(
            "    {}",
            row(&[
                (BOX, "├─ "),
                (MUTED, "languages "),
                (TEXT, languages.as_str())
            ])
        ),
    );
    add(
        &mut sh,
        &format!(
            "    {}",
            row(&[
                (BOX, "└─ "),
                (MUTED, "packages  "),
                (TEXT, packages.as_str())
            ])
        ),
    );
    add(&mut sh, &format!("    {}", row(&[])));
    add(&mut sh, &format!("    {}", bottom()));
    add(&mut sh, "  else");
    add(
        &mut sh,
        &format!(
            "    {}",
            compact(project, &format!("{shell} · {languages}"))
        ),
    );
    add(&mut sh, "  fi");
    add(
        &mut sh,
        &format!(
            "  nd_out $'\\n  {OK}●{RST} {BOLD}development shell ready{RST}  {DIM}· type exit to leave{RST}\\n'"
        ),
    );
    add(&mut sh, "");
    add(&mut sh, "  unset -f nd_out");
    add(&mut sh, "  unset nd_plain nd_cols");

    if let Some(pkg) = &spec.shell_pkg
        && let Some(binary) = shell_binary(pkg)
        && pkg != "bashInteractive"
    {
        add(&mut sh, "");
        add(
            &mut sh,
            "  # Hand over to the selected shell. SHELL is exported because `nix develop`",
        );
        add(
            &mut sh,
            "  # leaves it pointing at bash, which confuses tmux, editors and new tabs.",
        );
        add(&mut sh, r#"  nd_bin="@SHELL_BIN@""#);
        add(&mut sh, r#"  if [[ -x "$nd_bin" ]]; then"#);
        add(&mut sh, r#"    export SHELL="$nd_bin""#);
        add(&mut sh, "    nd_args=()");
        if binary == "fish" {
            add(
                &mut sh,
                "    # Replace a user-defined fish_greeting after config.fish has loaded.",
            );
            add(&mut sh, r#"    if [[ -z "${NIXDEV_GREETING:-}" ]]; then"#);
            add(
                &mut sh,
                "      nd_args=(--init-command 'function fish_greeting; end')",
            );
            add(&mut sh, "    fi");
        }
        add(&mut sh, r#"    exec "$nd_bin" "${nd_args[@]}""#);
        add(&mut sh, "  fi");
        add(
            &mut sh,
            r#"  printf 'nixdev: %s is not available; staying in bash\n' "$nd_bin" >&2"#,
        );
    }
    add(&mut sh, "fi");

    let escaped = match (
        &spec.shell_pkg,
        spec.shell_pkg.as_deref().and_then(shell_binary),
    ) {
        (Some(pkg), Some(binary)) => {
            nix_indented(&sh).replace("@SHELL_BIN@", &format!("${{pkgs.{pkg}}}/bin/{binary}"))
        }
        _ => nix_indented(&sh),
    };

    let mut out = String::with_capacity(escaped.len() + 256);
    out.push_str("\n            shellHook = ''\n");
    for line in escaped.lines() {
        if !line.is_empty() {
            out.push_str("              ");
            out.push_str(line);
        }
        out.push('\n');
    }
    out.push_str("            '';\n");
    out
}

fn header(spec: &Spec, title: &str) -> String {
    let rule = "-".repeat(72);
    let mut out = String::new();
    out.push_str(&format!("# nixdev {}\n", "-".repeat(64)));
    out.push_str(&format!("# project:     {}\n", spec.meta.project));
    out.push_str(&format!("# description: {}\n", one_line(title)));
    if !spec.meta.languages.is_empty() {
        out.push_str(&format!(
            "# languages:   {}\n",
            spec.meta.languages.join(", ")
        ));
    }
    if !spec.meta.shell.is_empty() && spec.meta.shell != "none" {
        out.push_str(&format!("# shell:       {}\n", spec.meta.shell));
    }
    if !spec.meta.generated.is_empty() {
        out.push_str(&format!("# generated:   {}\n", spec.meta.generated));
    }
    out.push_str(&format!("# {rule}\n\n"));
    out
}

fn flat(text: &str) -> String {
    let flat: String = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    flat.trim().to_string()
}

fn one_line(text: &str) -> String {
    truncate(&flat(text), 68)
}

fn shell_binary(shell: &str) -> Option<&'static str> {
    match shell {
        "bashInteractive" => Some("bash"),
        "nushell" => Some("nu"),
        "fish" => Some("fish"),
        "zsh" => Some("zsh"),
        "elvish" => Some("elvish"),
        "xonsh" => Some("xonsh"),
        "tcsh" => Some("tcsh"),
        "dash" => Some("dash"),
        _ => None,
    }
}

pub fn escape_string(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 8);
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '$' if chars.peek() == Some(&'{') => {
                out.push_str("\\${");
                chars.next();
            }
            c if c.is_control() => out.push(' '),
            c => out.push(c),
        }
    }
    out
}

pub fn has_managed_block(contents: &str) -> bool {
    contents.contains(MANAGED_START) && contents.contains(MANAGED_END)
}

pub fn add_packages(contents: &str, packages: &[String]) -> Result<String, String> {
    if !has_managed_block(contents) {
        return Err(format!(
            "no `{MANAGED_START}` block found; add packages by hand or regenerate with init"
        ));
    }
    let mut lines: Vec<String> = contents.lines().map(|l| l.to_string()).collect();
    let end_index = lines
        .iter()
        .position(|l| l.trim() == MANAGED_END)
        .ok_or_else(|| "managed block is missing its end marker".to_string())?;
    let indent: String = lines[end_index]
        .chars()
        .take_while(|c| c.is_whitespace())
        .collect();
    let mut inserted = 0;
    for pkg in packages {
        if !valid_attr(pkg) {
            return Err(format!("`{pkg}` is not a valid Nix attribute path"));
        }
        if lines.iter().any(|l| l.trim() == pkg) {
            continue;
        }
        lines.insert(end_index + inserted, format!("{indent}{pkg}"));
        inserted += 1;
    }
    if inserted == 0 {
        return Ok(contents.to_string());
    }
    let mut out = lines.join("\n");
    if contents.ends_with('\n') {
        out.push('\n');
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::shell_package;

    fn sample() -> Spec {
        Spec {
            meta: Meta {
                project: "demo".into(),
                description: "demo shell".into(),
                languages: vec!["rust".into()],
                shell: "fish".into(),
                generated: "2026-11-05".into(),
            },
            nixpkgs_url: "github:NixOS/nixpkgs/abc123".into(),
            nixpkgs_note: "nixos-unstable pinned on 2026-11-05".into(),
            systems: vec!["x86_64-linux".into()],
            packages: vec!["cargo".into(), "rust-analyzer".into()],
            env: vec![(
                "RUST_SRC_PATH".into(),
                "\"${pkgs.rustPlatform.rustLibSrc}\"".into(),
            )],
            shell_pkg: Some(shell_package("fish").unwrap().to_string()),
        }
    }

    #[test]
    fn renders_valid_structure() {
        let text = render(&sample());
        assert!(text.contains("description = \"demo shell\""));
        assert!(text.contains(MANAGED_START));
        assert!(text.contains("nd_bin=\"${pkgs.fish}/bin/fish\""));
        assert!(text.contains("exec \"$nd_bin\""));
        assert!(text.contains("''${NIXDEV_SHELL_ACTIVE:-}"));
        assert!(text.contains("--init-command 'function fish_greeting; end'"));
        assert!(text.contains("RUST_SRC_PATH = \"${pkgs.rustPlatform.rustLibSrc}\";"));
        assert!(!text.contains("mktemp"), "the PATH shim must be gone");
        assert_eq!(text.matches('{').count(), text.matches('}').count());
    }

    #[test]
    fn hook_only_runs_for_interactive_sessions() {
        let text = render(&sample());
        assert!(text.contains("$- == *i* && -t 0 && -t 1"));
    }

    #[test]
    fn bash_and_none_do_not_exec() {
        let mut spec = sample();
        spec.meta.shell = "bash".into();
        spec.shell_pkg = Some(shell_package("bash").unwrap().to_string());
        assert!(!render(&spec).contains("exec "));
        spec.meta.shell = "none".into();
        spec.shell_pkg = None;
        assert!(!render(&spec).contains("exec "));
    }

    #[test]
    fn escapes_interpolation_in_description() {
        let mut spec = sample();
        spec.meta.description = "evil ${pkgs.hello} \"quote\"".into();
        let text = render(&spec);
        assert!(text.contains("\\${pkgs.hello}"));
        assert!(text.contains("\\\"quote\\\""));
    }

    #[test]
    fn hostile_text_never_reaches_nix_or_the_shell() {
        let mut spec = sample();
        spec.meta.description = "it's ${pkgs.hello} ''x $(id) `id` \\ \"q\"".into();
        spec.meta.project = "pro'ject".into();
        spec.nixpkgs_note = "note ${pkgs.hello}\nnewline".into();
        let text = render(&spec);
        for line in text.lines().filter(|l| l.contains("nd_out $'")) {
            assert!(!line.contains("${"), "interpolation leaked: {line}");
            assert!(!line.contains("''"), "nix string terminator leaked: {line}");
            assert!(!line.contains("$("), "command substitution leaked: {line}");
        }
        assert!(!text.contains("note ${pkgs.hello}\nnewline"));
    }

    #[test]
    fn banner_rows_share_one_width() {
        let widths: Vec<usize> = [row(&[]), row(&[(ACC, "◈  "), (TITLE, "project")])]
            .iter()
            .map(|r| {
                let body = r.trim_start_matches("nd_out $'").trim_end_matches('\'');
                let mut plain = String::new();
                let mut chars = body.chars().peekable();
                while let Some(c) = chars.next() {
                    if c == '\\' && chars.peek() == Some(&'e') {
                        for n in chars.by_ref() {
                            if n == 'm' {
                                break;
                            }
                        }
                    } else {
                        plain.push(c);
                    }
                }
                plain.width()
            })
            .collect();
        assert_eq!(widths[0], widths[1]);
    }

    #[test]
    fn rejects_bad_specs() {
        let mut spec = sample();
        spec.packages.push("foo [".into());
        assert!(spec.validate().is_err());

        let mut spec = sample();
        spec.systems.clear();
        assert!(spec.validate().is_err());

        let mut spec = sample();
        spec.nixpkgs_url = "github:a/b\nc".into();
        assert!(spec.validate().is_err());
    }

    #[test]
    fn adds_packages_inside_block() {
        let text = render(&sample());
        let updated = add_packages(&text, &["pkg-config".into()]).unwrap();
        assert!(updated.contains("              pkg-config\n"));
        assert_eq!(updated.matches(MANAGED_END).count(), 1);
        let again = add_packages(&updated, &["pkg-config".into()]).unwrap();
        assert_eq!(again, updated);
    }

    #[test]
    fn add_without_block_fails() {
        assert!(add_packages("{}", &["cargo".into()]).is_err());
    }
}
