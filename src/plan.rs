use std::path::PathBuf;

use crate::profile;
use crate::render::{Meta, Spec};
use crate::util::{format_date, now_secs, project_name_from_dir, sanitize_project_name};

#[derive(Clone)]
pub struct Plan {
    pub dir: PathBuf,
    pub name: String,
    pub description: String,
    pub languages: Vec<String>,
    pub extras: Vec<String>,
    pub packages: Vec<String>,
    pub systems: Vec<String>,
    pub shell: String,
    pub nixpkgs_url: String,
    pub nixpkgs_note: String,
    pub rev: String,
    pub generated: String,
}

impl Plan {
    pub fn seed(dir: PathBuf) -> Self {
        let name = project_name_from_dir(&dir);
        let system = crate::nix::current_system();
        Self {
            dir,
            description: format!("{name} development environment"),
            name,
            languages: Vec::new(),
            extras: Vec::new(),
            packages: Vec::new(),
            systems: vec![system],
            shell: profile::detect_shell(),
            nixpkgs_url: String::new(),
            nixpkgs_note: String::new(),
            rev: String::new(),
            generated: format_date(now_secs()),
        }
    }

    pub fn normalize(mut self) -> Result<Self, String> {
        self.name = sanitize_project_name(&self.name);
        if self.description.trim().is_empty() {
            self.description = format!("{} development environment", self.name);
        }
        self.description = self.description.trim().to_string();
        self.languages.retain(|id| profile::profile(id).is_some());
        self.extras.retain(|id| profile::extra(id).is_some());
        self.systems.sort();
        self.systems.dedup();
        if self.systems.is_empty() {
            self.systems.push(crate::nix::current_system());
        }
        self.packages.retain(|p| !p.trim().is_empty());
        self.packages.sort();
        self.packages.dedup();
        if self.shell != "none" && profile::shell_package(&self.shell).is_none() {
            return Err(format!(
                "unknown shell `{}`; pick one of {} or none",
                self.shell,
                profile::SHELLS.join(", ")
            ));
        }
        Ok(self)
    }

    pub fn resolved(&self) -> profile::Resolved {
        profile::resolve(&self.languages, &self.extras)
    }

    pub fn language_labels(&self) -> Vec<String> {
        self.languages
            .iter()
            .map(|id| {
                profile::profile(id)
                    .map(|p| p.label.to_string())
                    .unwrap_or_else(|| id.clone())
            })
            .collect()
    }

    pub fn to_spec(&self) -> Spec {
        let resolved = self.resolved();
        let mut packages = resolved.packages;
        for pkg in &self.packages {
            if !packages.contains(pkg) {
                packages.push(pkg.clone());
            }
        }
        let shell_pkg = if self.shell == "none" {
            None
        } else {
            profile::shell_package(&self.shell).map(|s| s.to_string())
        };
        Spec {
            meta: Meta {
                project: self.name.clone(),
                description: self.description.clone(),
                languages: self.language_labels(),
                shell: self.shell.clone(),
                generated: self.generated.clone(),
            },
            nixpkgs_url: self.nixpkgs_url.clone(),
            nixpkgs_note: self.nixpkgs_note.clone(),
            systems: self.systems.clone(),
            packages,
            env: resolved.env.into_iter().collect(),
            shell_pkg,
        }
    }

    pub fn render(&self) -> Result<String, String> {
        let spec = self.to_spec();
        spec.validate()?;
        Ok(crate::render::render(&spec))
    }
}

pub fn package_warnings(plan: &Plan) -> Vec<String> {
    let spec = plan.to_spec();
    if spec.packages.is_empty() {
        return Vec::new();
    }
    let mut warnings = Vec::new();

    let flake = if plan.nixpkgs_url.is_empty() {
        "nixpkgs"
    } else {
        plan.nixpkgs_url.as_str()
    };

    for system in &plan.systems {
        if crate::nix::is_canceled() {
            break;
        }

        match crate::nix::system_supported(flake, system) {
            Ok(false) => warnings.push(format!("nixpkgs does not support the system `{system}`")),
            Ok(true) => match crate::nix::has_attrs(flake, system, &spec.packages) {
                Ok(results) => {
                    for (pkg, exists) in spec.packages.iter().zip(results) {
                        if !exists {
                            warnings.push(format!("`{pkg}` is not available on {system}"));
                        }
                    }
                }
                Err(crate::nix::NixError::Canceled) => break,
                Err(err) => warnings.push(format!("could not verify packages on {system}: {err}")),
            },
            Err(crate::nix::NixError::Canceled) => break,
            Err(err) => warnings.push(format!("could not check the system `{system}`: {err}")),
        }
    }
    warnings
}

pub fn validate_languages(ids: &[String]) -> Result<(), String> {
    let unknown: Vec<&String> = ids
        .iter()
        .filter(|id| profile::profile(id).is_none())
        .collect();
    if unknown.is_empty() {
        return Ok(());
    }
    let names: Vec<String> = unknown.iter().map(|s| format!("`{s}`")).collect();
    let known: Vec<&str> = profile::PROFILES.iter().map(|p| p.id).collect();
    Err(format!(
        "unknown language {}; available: {}",
        names.join(", "),
        known.join(", ")
    ))
}

pub fn validate_extras(ids: &[String]) -> Result<(), String> {
    let unknown: Vec<&String> = ids
        .iter()
        .filter(|id| profile::extra(id).is_none())
        .collect();
    if unknown.is_empty() {
        return Ok(());
    }
    let names: Vec<String> = unknown.iter().map(|s| format!("`{s}`")).collect();
    let known: Vec<&str> = profile::EXTRAS.iter().map(|e| e.id).collect();
    Err(format!(
        "unknown extra {}; available: {}",
        names.join(", "),
        known.join(", ")
    ))
}

pub fn pin_url(reference: &str, rev: &str) -> String {
    if let Some(rest) = reference.strip_prefix("github:") {
        let mut parts = rest.split('/');
        if let (Some(owner), Some(repo), _) = (parts.next(), parts.next(), parts.next()) {
            return format!("github:{owner}/{repo}/{rev}");
        }
    }
    reference.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reuses_project_names_and_defaults_shell() {
        let plan = Plan::seed(PathBuf::from("/tmp/my-project"))
            .normalize()
            .unwrap();
        assert_eq!(plan.name, "my-project");
        assert!(!plan.systems.is_empty());
        assert!(!plan.shell.is_empty());
    }

    #[test]
    fn rejects_unknown_languages() {
        assert!(validate_languages(&["rust".into()]).is_ok());
        assert!(validate_languages(&["brainfuck".into()]).is_err());
    }

    #[test]
    fn rewrites_github_refs_to_revisions() {
        assert_eq!(
            pin_url("github:NixOS/nixpkgs/nixos-unstable", "abc"),
            "github:NixOS/nixpkgs/abc"
        );
        assert_eq!(pin_url("/local/path", "abc"), "/local/path");
    }
}
