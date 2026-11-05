use std::collections::BTreeMap;

#[derive(Clone, Copy)]
pub struct Profile {
    pub id: &'static str,
    pub label: &'static str,
    pub blurb: &'static str,
    pub packages: &'static [&'static str],
    pub env: &'static [(&'static str, &'static str)],
}

pub const PROFILES: &[Profile] = &[
    Profile {
        id: "rust",
        label: "Rust",
        blurb: "cargo, rustc, clippy, rustfmt, rust-analyzer",
        packages: &["cargo", "rustc", "rustfmt", "clippy", "rust-analyzer"],
        env: &[("RUST_SRC_PATH", "\"${pkgs.rustPlatform.rustLibSrc}\"")],
    },
    Profile {
        id: "c",
        label: "C",
        blurb: "gcc, make, cmake, gdb, clangd",
        packages: &[
            "gcc",
            "gnumake",
            "cmake",
            "pkg-config",
            "gdb",
            "clang-tools",
            "ccache",
        ],
        env: &[("LIBCLANG_PATH", "\"${pkgs.llvmPackages.libclang.lib}/lib\"")],
    },
    Profile {
        id: "cpp",
        label: "C++",
        blurb: "gcc, make, cmake, gdb, clangd",
        packages: &[
            "gcc",
            "gnumake",
            "cmake",
            "pkg-config",
            "gdb",
            "clang-tools",
            "ccache",
        ],
        env: &[("LIBCLANG_PATH", "\"${pkgs.llvmPackages.libclang.lib}/lib\"")],
    },
    Profile {
        id: "go",
        label: "Go",
        blurb: "go, gopls, gotools, delve",
        packages: &["go", "gopls", "gotools", "delve"],
        env: &[],
    },
    Profile {
        id: "python",
        label: "Python",
        blurb: "python3, uv, ruff, pyright",
        packages: &["python3", "uv", "ruff", "pyright"],
        env: &[],
    },
    Profile {
        id: "node",
        label: "JavaScript / TypeScript",
        blurb: "nodejs, pnpm, tsc, eslint, prettier",
        packages: &[
            "nodejs_22",
            "pnpm",
            "typescript",
            "typescript-language-server",
            "eslint",
            "prettier",
        ],
        env: &[],
    },
    Profile {
        id: "nix",
        label: "Nix",
        blurb: "nixd, nil, nixfmt, statix, deadnix",
        packages: &["nixd", "nil", "nixfmt-rfc-style", "statix", "deadnix"],
        env: &[],
    },
    Profile {
        id: "zig",
        label: "Zig",
        blurb: "zig, zls",
        packages: &["zig", "zls"],
        env: &[],
    },
    Profile {
        id: "lua",
        label: "Lua",
        blurb: "lua5.4, luajit, lua-language-server, stylua",
        packages: &["lua5_4", "luajit", "lua-language-server", "stylua"],
        env: &[],
    },
    Profile {
        id: "haskell",
        label: "Haskell",
        blurb: "ghc, cabal, HLS, hlint, ormolu",
        packages: &[
            "ghc",
            "cabal-install",
            "haskell-language-server",
            "hlint",
            "ormolu",
        ],
        env: &[],
    },
    Profile {
        id: "java",
        label: "Java",
        blurb: "jdk21, gradle, maven, jdtls",
        packages: &[
            "jdk21",
            "gradle",
            "maven",
            "jdt-language-server",
            "google-java-format",
        ],
        env: &[],
    },
    Profile {
        id: "kotlin",
        label: "Kotlin",
        blurb: "kotlin, ktlint, kotlin-language-server",
        packages: &["kotlin", "ktlint", "kotlin-language-server"],
        env: &[],
    },
    Profile {
        id: "ruby",
        label: "Ruby",
        blurb: "ruby, bundler, ruby-lsp, rubocop",
        packages: &["ruby", "bundler", "ruby-lsp", "rubocop"],
        env: &[],
    },
    Profile {
        id: "elixir",
        label: "Elixir",
        blurb: "elixir, elixir-ls",
        packages: &["elixir", "elixir-ls"],
        env: &[],
    },
    Profile {
        id: "ocaml",
        label: "OCaml",
        blurb: "ocaml, dune, ocaml-lsp, utop",
        packages: &[
            "ocaml",
            "dune",
            "ocamlPackages.ocaml-lsp",
            "ocamlPackages.utop",
        ],
        env: &[],
    },
    Profile {
        id: "shell",
        label: "Shell scripting",
        blurb: "shellcheck, shfmt, bash-language-server",
        packages: &["shellcheck", "shfmt", "bash-language-server"],
        env: &[],
    },
    Profile {
        id: "terraform",
        label: "Terraform",
        blurb: "terraform, terraform-ls, tflint",
        packages: &["terraform", "terraform-ls", "tflint"],
        env: &[],
    },
    Profile {
        id: "dotnet",
        label: "C# / .NET",
        blurb: "dotnet-sdk, csharp-ls",
        packages: &["dotnet-sdk", "csharp-ls"],
        env: &[],
    },
];

pub struct Extra {
    pub id: &'static str,
    pub label: &'static str,
    pub blurb: &'static str,
    pub category: &'static str,
    /// Languages this tool is useful for; empty means every project.
    pub languages: &'static [&'static str],
    pub packages: &'static [&'static str],
}

const DEBUGGERS: &[&str] = &["c", "cpp", "rust", "zig"];
const BACKENDS: &[&str] = &[
    "python", "node", "ruby", "elixir", "go", "java", "kotlin", "ocaml", "dotnet",
];

pub const EXTRAS: &[Extra] = &[
    Extra {
        id: "gdb",
        label: "gdb",
        blurb: "GNU debugger",
        category: "debugging",
        languages: DEBUGGERS,
        packages: &["gdb"],
    },
    Extra {
        id: "lldb",
        label: "lldb",
        blurb: "LLVM debugger",
        category: "debugging",
        languages: DEBUGGERS,
        packages: &["lldb"],
    },
    Extra {
        id: "valgrind",
        label: "valgrind",
        blurb: "memory error detector",
        category: "debugging",
        languages: DEBUGGERS,
        packages: &["valgrind"],
    },
    Extra {
        id: "strace",
        label: "strace / ltrace",
        blurb: "syscall and library call tracers",
        category: "debugging",
        languages: DEBUGGERS,
        packages: &["strace", "ltrace"],
    },
    Extra {
        id: "sqlite",
        label: "sqlite",
        blurb: "embedded SQL database and CLI",
        category: "databases",
        languages: BACKENDS,
        packages: &["sqlite"],
    },
    Extra {
        id: "postgres",
        label: "postgresql",
        blurb: "PostgreSQL client and server",
        category: "databases",
        languages: BACKENDS,
        packages: &["postgresql"],
    },
    Extra {
        id: "redis",
        label: "redis",
        blurb: "in-memory data store and CLI",
        category: "databases",
        languages: BACKENDS,
        packages: &["redis"],
    },
    Extra {
        id: "mariadb",
        label: "mariadb",
        blurb: "MySQL-compatible server and client",
        category: "databases",
        languages: BACKENDS,
        packages: &["mariadb"],
    },
    Extra {
        id: "docker",
        label: "docker client",
        blurb: "docker CLI against a local daemon",
        category: "containers",
        languages: &[],
        packages: &["docker-client"],
    },
    Extra {
        id: "podman",
        label: "podman",
        blurb: "daemonless container engine",
        category: "containers",
        languages: &[],
        packages: &["podman"],
    },
    Extra {
        id: "kubectl",
        label: "kubectl / helm",
        blurb: "Kubernetes CLI and package manager",
        category: "containers",
        languages: &[],
        packages: &["kubectl", "kubernetes-helm"],
    },
    Extra {
        id: "k9s",
        label: "k9s",
        blurb: "terminal UI for Kubernetes",
        category: "containers",
        languages: &[],
        packages: &["k9s"],
    },
    Extra {
        id: "just",
        label: "just",
        blurb: "command runner",
        category: "build",
        languages: &[],
        packages: &["just"],
    },
    Extra {
        id: "ninja",
        label: "ninja / meson",
        blurb: "fast build system and its meta-build tool",
        category: "build",
        languages: &["c", "cpp", "rust"],
        packages: &["ninja", "meson"],
    },
    Extra {
        id: "curl",
        label: "curl / wget",
        blurb: "HTTP clients",
        category: "network",
        languages: &[],
        packages: &["curl", "wget"],
    },
    Extra {
        id: "httpie",
        label: "httpie / grpcurl",
        blurb: "human-friendly HTTP and gRPC clients",
        category: "network",
        languages: &[],
        packages: &["httpie", "grpcurl"],
    },
    Extra {
        id: "jq",
        label: "jq / yq",
        blurb: "JSON and YAML processors",
        category: "network",
        languages: &[],
        packages: &["jq", "yq"],
    },
    Extra {
        id: "pandoc",
        label: "pandoc",
        blurb: "document converter",
        category: "docs",
        languages: &[],
        packages: &["pandoc"],
    },
    Extra {
        id: "tectonic",
        label: "tectonic",
        blurb: "self-contained LaTeX engine",
        category: "docs",
        languages: &[],
        packages: &["tectonic"],
    },
    Extra {
        id: "graphviz",
        label: "graphviz",
        blurb: "graph visualization tools",
        category: "docs",
        languages: &[],
        packages: &["graphviz"],
    },
    Extra {
        id: "ffmpeg",
        label: "ffmpeg",
        blurb: "audio and video toolkit",
        category: "media",
        languages: &[],
        packages: &["ffmpeg"],
    },
    Extra {
        id: "imagemagick",
        label: "imagemagick",
        blurb: "image conversion and editing",
        category: "media",
        languages: &[],
        packages: &["imagemagick"],
    },
    Extra {
        id: "neovim",
        label: "neovim",
        blurb: "modal text editor",
        category: "editors",
        languages: &[],
        packages: &["neovim"],
    },
    Extra {
        id: "helix",
        label: "helix",
        blurb: "post-modern modal editor",
        category: "editors",
        languages: &[],
        packages: &["helix"],
    },
    Extra {
        id: "cargo-watch",
        label: "cargo watch",
        blurb: "rebuild and rerun on change",
        category: "rust",
        languages: &["rust"],
        packages: &["cargo-watch"],
    },
    Extra {
        id: "nextest",
        label: "cargo nextest",
        blurb: "faster test runner",
        category: "rust",
        languages: &["rust"],
        packages: &["cargo-nextest"],
    },
    Extra {
        id: "cargo-audit",
        label: "cargo audit",
        blurb: "scan dependencies for advisories",
        category: "rust",
        languages: &["rust"],
        packages: &["cargo-audit"],
    },
    Extra {
        id: "bear",
        label: "bear",
        blurb: "generate compile_commands.json",
        category: "c/c++",
        languages: &["c", "cpp"],
        packages: &["bear"],
    },
    Extra {
        id: "cppcheck",
        label: "cppcheck",
        blurb: "static analysis for C and C++",
        category: "c/c++",
        languages: &["c", "cpp"],
        packages: &["cppcheck"],
    },
    Extra {
        id: "ipython",
        label: "ipython",
        blurb: "interactive Python REPL",
        category: "python",
        languages: &["python"],
        packages: &["python3Packages.ipython"],
    },
    Extra {
        id: "pytest",
        label: "pytest",
        blurb: "test runner",
        category: "python",
        languages: &["python"],
        packages: &["python3Packages.pytest"],
    },
    Extra {
        id: "golangci-lint",
        label: "golangci-lint",
        blurb: "meta linter for Go",
        category: "go",
        languages: &["go"],
        packages: &["golangci-lint"],
    },
    Extra {
        id: "bun",
        label: "bun",
        blurb: "fast JS runtime and package manager",
        category: "javascript",
        languages: &["node"],
        packages: &["bun"],
    },
    Extra {
        id: "nix-tree",
        label: "nix-tree",
        blurb: "explore store closures",
        category: "nix",
        languages: &["nix"],
        packages: &["nix-tree"],
    },
    Extra {
        id: "nom",
        label: "nix-output-monitor",
        blurb: "readable nix build output",
        category: "nix",
        languages: &["nix"],
        packages: &["nix-output-monitor"],
    },
    Extra {
        id: "luarocks",
        label: "luarocks",
        blurb: "Lua package manager",
        category: "lua",
        languages: &["lua"],
        packages: &["luarocks"],
    },
    Extra {
        id: "ghcid",
        label: "ghcid",
        blurb: "reload GHCi on file changes",
        category: "haskell",
        languages: &["haskell"],
        packages: &["ghcid"],
    },
    Extra {
        id: "gradle",
        label: "gradle",
        blurb: "build automation for the JVM",
        category: "jvm",
        languages: &["java", "kotlin"],
        packages: &["gradle"],
    },
    Extra {
        id: "merlin",
        label: "merlin",
        blurb: "editor integration for OCaml",
        category: "ocaml",
        languages: &["ocaml"],
        packages: &["ocamlPackages.merlin"],
    },
    Extra {
        id: "shellharden",
        label: "shellharden",
        blurb: "writes safer shell scripts",
        category: "shell",
        languages: &["shell"],
        packages: &["shellharden"],
    },
    Extra {
        id: "terraform-docs",
        label: "terraform-docs",
        blurb: "generate module documentation",
        category: "terraform",
        languages: &["terraform"],
        packages: &["terraform-docs"],
    },
];

/// Extras that match the chosen languages, grouped in language order.
pub fn extras_for(languages: &[String]) -> Vec<(&'static str, &'static Extra)> {
    let mut out: Vec<(&'static str, &'static Extra)> = Vec::new();
    for language in languages {
        let Some(profile) = profile(language) else {
            continue;
        };
        for extra in EXTRAS {
            if extra.languages.contains(&language.as_str())
                && !out.iter().any(|(_, picked)| picked.id == extra.id)
            {
                out.push((profile.label, extra));
            }
        }
    }
    out
}

pub fn general_extras() -> impl Iterator<Item = &'static Extra> {
    EXTRAS.iter().filter(|extra| extra.languages.is_empty())
}

pub fn profile(id: &str) -> Option<&'static Profile> {
    PROFILES.iter().find(|p| p.id == id)
}

pub fn extra(id: &str) -> Option<&'static Extra> {
    EXTRAS.iter().find(|e| e.id == id)
}

#[derive(Default)]
pub struct Resolved {
    pub packages: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub unknown: Vec<String>,
}

pub fn resolve(languages: &[String], extras: &[String]) -> Resolved {
    let mut out = Resolved::default();
    for id in languages {
        match profile(id) {
            Some(p) => {
                for pkg in p.packages {
                    push_unique(&mut out.packages, pkg);
                }
                for (key, value) in p.env {
                    out.env.insert((*key).to_string(), (*value).to_string());
                }
            }
            None => out.unknown.push(id.clone()),
        }
    }
    for id in extras {
        match extra(id) {
            Some(e) => {
                for pkg in e.packages {
                    push_unique(&mut out.packages, pkg);
                }
            }
            None => out.unknown.push(id.clone()),
        }
    }
    out
}

fn push_unique(packages: &mut Vec<String>, pkg: &str) {
    if !packages.iter().any(|p| p == pkg) {
        packages.push(pkg.to_string());
    }
}

pub fn shell_package(shell: &str) -> Option<&'static str> {
    Some(match shell {
        "fish" => "fish",
        "zsh" => "zsh",
        "bash" => "bashInteractive",
        "nu" => "nushell",
        "elvish" => "elvish",
        "xonsh" => "xonsh",
        "tcsh" => "tcsh",
        "dash" => "dash",
        _ => return None,
    })
}

pub fn detect_shell() -> String {
    let shell = std::env::var("SHELL").unwrap_or_default();
    let name = std::path::Path::new(&shell)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("bash")
        .to_string();
    if shell_package(&name).is_some() {
        name
    } else {
        "bash".to_string()
    }
}

pub const SHELLS: &[&str] = &[
    "fish", "zsh", "bash", "nu", "elvish", "xonsh", "tcsh", "dash",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profiles_have_unique_ids() {
        let mut ids: Vec<_> = PROFILES.iter().map(|p| p.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), PROFILES.len());
    }

    #[test]
    fn resolving_merges_and_dedupes() {
        let out = resolve(&["c".into(), "cpp".into()], &["gdb".into()]);
        let gdb = out.packages.iter().filter(|p| *p == "gdb").count();
        assert_eq!(gdb, 1);
        assert!(out.env.contains_key("LIBCLANG_PATH"));
    }

    #[test]
    fn unknown_ids_are_reported() {
        let out = resolve(&["nope".into()], &[]);
        assert_eq!(out.unknown, vec!["nope"]);
    }

    #[test]
    fn detects_shell_from_environment() {
        unsafe { std::env::set_var("SHELL", "/usr/bin/fish") };
        assert_eq!(detect_shell(), "fish");
        unsafe { std::env::set_var("SHELL", "/usr/bin/weirdsh") };
        assert_eq!(detect_shell(), "bash");
    }
}
