use crate::util::valid_attr;
use crate::util::valid_env_key;

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
        if self.nixpkgs_url.trim().is_empty() || self.nixpkgs_url.contains('"') {
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
    let mut out = String::with_capacity(4096);
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
        out.push_str(&format!("    # {}\n", spec.nixpkgs_note));
    }
    out.push_str(&format!("    nixpkgs.url = \"{}\";\n", spec.nixpkgs_url));
    out.push_str("  };\n\n");

    out.push_str("  outputs =\n");
    out.push_str("    { self, nixpkgs }:\n");
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

fn shell_hook(spec: &Spec) -> String {
    let mut out = String::with_capacity(2048);
    out.push_str("\n            shellHook = ''\n");
    out.push_str(
        "              # nixdev: greet interactive shells only; `nix develop -c` stays quiet\n",
    );
    out.push_str(
        "              if [[ $- == *i* ]] && [[ -z \"''${NIXDEV_SHELL_ACTIVE:-}\" ]]; then\n",
    );
    out.push_str("                export NIXDEV_SHELL_ACTIVE=1\n\n");
    out.push_str("                # Mute a fastfetch greeting from the user's shell rc.\n");
    out.push_str(
        "                nixdev_shim=\"''${TMPDIR:-/tmp}/nixdev-shim-''${USER:-user}\"\n",
    );
    out.push_str("                mkdir -p \"''$nixdev_shim\"\n");
    out.push_str(
        "                printf '#!/bin/sh\\nexit 0\\n' > \"''$nixdev_shim/fastfetch\"\n",
    );
    out.push_str("                chmod +x \"''$nixdev_shim/fastfetch\"\n");
    out.push_str("                case \":''$PATH:\" in\n");
    out.push_str("                  *\":''$nixdev_shim:\"*) ;;\n");
    out.push_str("                  *) export PATH=\"''$nixdev_shim:''$PATH\" ;;\n");
    out.push_str("                esac\n\n");
    out.push_str("                # Clean slate, then the nixdev banner.\n");
    out.push_str("                clear 2>/dev/null || printf '\\033[2J\\033[H'\n");
    out.push_str("                printf '\\n'\n");
    out.push_str(
        "                printf '\\033[38;2;122;162;247m  \u{256d}\u{2500} nixdev \u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{256e}\\033[0m\\n'\n",
    );

    let accent = "\\033[38;2;122;162;247m";
    let reset = "\\033[0m";
    let mut rows: Vec<String> = Vec::new();
    rows.push(format!("\\033[1m{}{}", spec.meta.project, reset));
    let languages = if spec.meta.languages.is_empty() {
        "no languages selected".to_string()
    } else {
        spec.meta.languages.join(" \u{00b7} ")
    };
    rows.push(languages);
    rows.push("type `exit` to leave the shell".to_string());
    for row in rows {
        let row = row.replace('\'', "");
        out.push_str("                printf '");
        out.push_str(accent);
        out.push_str("  \u{2502}");
        out.push_str(reset);
        out.push_str("  ");
        out.push_str(&row);
        out.push_str("\\n'\n");
    }
    out.push_str(
        "                printf '\\033[38;2;122;162;247m  \u{2570}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{256f}\\033[0m\\n'\n",
    );
    out.push_str("                printf '\\n'\n");

    if let Some(shell) = &spec.shell_pkg
        && let Some(binary) = shell_binary(shell)
    {
        out.push_str(&format!(
            "                exec ${{pkgs.{shell}}}/bin/{binary}\n"
        ));
    }
    out.push_str("              fi\n");
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

fn one_line(text: &str) -> String {
    let flat: String = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    crate::util::truncate(flat.trim(), 68)
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
                generated: "2026-11-04".into(),
            },
            nixpkgs_url: "github:NixOS/nixpkgs/abc123".into(),
            nixpkgs_note: "nixos-unstable as of 2026-11-04".into(),
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
        assert!(text.contains("exec ${pkgs.fish}/bin/fish"));
        assert!(text.contains("''${NIXDEV_SHELL_ACTIVE:-}"));
        assert!(text.contains("RUST_SRC_PATH = \"${pkgs.rustPlatform.rustLibSrc}\";"));
        assert_eq!(text.matches('{').count(), text.matches('}').count());
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
    fn rejects_bad_specs() {
        let mut spec = sample();
        spec.packages.push("foo [".into());
        assert!(spec.validate().is_err());

        let mut spec = sample();
        spec.systems.clear();
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
