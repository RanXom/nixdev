use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "nixdev",
    args_conflicts_with_subcommands = true,
    version,
    disable_version_flag = true,
    about = "Generate Nix dev-shell flakes without the guesswork",
    long_about = "nixdev asks a few questions, then writes a pinned flake.nix with the toolchains, \
                  language servers and packages you picked. Run it with no arguments to start."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

    /// Project directory; shorthand for `nixdev init DIR`
    #[arg(value_name = "DIR")]
    pub dir: Option<PathBuf>,

    #[arg(long, global = true, help = "Disable colour output")]
    pub no_color: bool,

    /// Print version information
    #[arg(short = 'v', long = "version", action = clap::ArgAction::Version)]
    pub version: bool,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Create a flake.nix for a project
    Init(InitArgs),
    /// Add packages to an existing flake.nix
    Add(AddArgs),
    /// Search nixpkgs for packages
    Search(SearchArgs),
    /// Build or refresh the local package index
    Index(IndexArgs),
    /// Check that nixdev has everything it needs
    Doctor,
    /// Print shell completions
    Completions {
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
}

#[derive(Args, Debug)]
pub struct InitArgs {
    /// Project directory (created if missing)
    #[arg(default_value = ".")]
    pub dir: PathBuf,

    /// Project name written into the flake header
    #[arg(long)]
    pub name: Option<String>,

    /// One-line project description
    #[arg(long)]
    pub description: Option<String>,

    /// Languages: rust,c,cpp,go,python,node,nix,zig,lua,haskell,java,kotlin,ruby,elixir,ocaml,shell,terraform,dotnet
    #[arg(long, value_delimiter = ',', value_name = "LIST")]
    pub lang: Vec<String>,

    /// Optional tool bundles: gdb,lldb,valgrind,strace,sqlite,postgres,redis,mariadb,docker,podman,kubectl,k9s,just,ninja,curl,httpie,jq,pandoc,tectonic,graphviz,ffmpeg,imagemagick,neovim,helix
    #[arg(long = "extra", value_delimiter = ',', value_name = "LIST")]
    pub extras: Vec<String>,

    /// Extra nixpkgs attribute paths, for example yyjson,dbus,python3Packages.requests
    #[arg(long = "pkg", value_delimiter = ',', value_name = "LIST")]
    pub packages: Vec<String>,

    /// Target systems, for example x86_64-linux,aarch64-darwin
    #[arg(long, value_delimiter = ',', value_name = "LIST")]
    pub systems: Vec<String>,

    /// Shell to enter from the dev shell (fish, zsh, bash, nu, none)
    #[arg(long)]
    pub shell: Option<String>,

    /// Nixpkgs flake reference or revision to pin
    #[arg(long)]
    pub nixpkgs: Option<String>,

    /// Skip the wizard and accept defaults
    #[arg(short = 'y', long)]
    pub yes: bool,

    /// Replace an existing flake.nix
    #[arg(long)]
    pub force: bool,

    /// Print the flake to stdout instead of writing it
    #[arg(long)]
    pub print: bool,

    /// Show what would be written without touching the filesystem
    #[arg(long)]
    pub dry_run: bool,

    /// Do not build the package index
    #[arg(long)]
    pub no_index: bool,

    /// Do not run nix flake lock
    #[arg(long)]
    pub no_lock: bool,

    /// Do not initialise a git repository
    #[arg(long)]
    pub no_git: bool,

    /// Do not write a .envrc
    #[arg(long)]
    pub no_envrc: bool,

    /// Do not update .gitignore
    #[arg(long)]
    pub no_gitignore: bool,

    /// Do not run nixfmt or alejandra
    #[arg(long)]
    pub no_format: bool,

    /// Run a full nix flake check after writing
    #[arg(long)]
    pub check: bool,

    /// Do not keep a backup of an existing flake.nix
    #[arg(long)]
    pub no_backup: bool,

    /// Do not validate that selected packages exist
    #[arg(long)]
    pub no_validate: bool,
}

#[derive(Args, Debug)]
pub struct AddArgs {
    /// Attribute paths to add, for example yyjson or python3Packages.requests
    #[arg(required = true)]
    pub packages: Vec<String>,

    /// Project directory containing flake.nix
    #[arg(long, default_value = ".")]
    pub dir: PathBuf,

    /// Allow editing when no managed block exists
    #[arg(long)]
    pub force: bool,

    /// Show the updated flake without writing
    #[arg(long)]
    pub dry_run: bool,

    /// Do not run nixfmt or alejandra
    #[arg(long)]
    pub no_format: bool,
}

#[derive(Args, Debug)]
pub struct SearchArgs {
    /// Search term; omit to list popular packages
    pub term: Option<String>,

    /// Maximum number of results
    #[arg(long, default_value_t = 20)]
    pub limit: usize,

    /// Search nixpkgs directly instead of using the local index
    #[arg(long)]
    pub live: bool,

    /// Print results as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct IndexArgs {
    /// Rebuild even when a cached index exists
    #[arg(long)]
    pub refresh: bool,

    /// System to index
    #[arg(long)]
    pub system: Option<String>,

    /// Nixpkgs revision to index
    #[arg(long)]
    pub rev: Option<String>,
}

impl InitArgs {
    pub fn in_dir(dir: PathBuf) -> Self {
        let mut args = Self::defaults();
        args.dir = dir;
        args
    }

    pub fn defaults() -> Self {
        Self {
            dir: std::path::PathBuf::from("."),
            name: None,
            description: None,
            lang: Vec::new(),
            extras: Vec::new(),
            packages: Vec::new(),
            systems: Vec::new(),
            shell: None,
            nixpkgs: None,
            yes: false,
            force: false,
            print: false,
            dry_run: false,
            no_index: false,
            no_lock: false,
            no_git: false,
            no_envrc: false,
            no_gitignore: false,
            no_format: false,
            check: false,
            no_backup: false,
            no_validate: false,
        }
    }
}
