# nixdev

A small CLI that writes a pinned `flake.nix` dev shell for your project. Answer a few prompts, get the toolchains, language servers, formatters, and packages you picked.

```
$ nixdev my-app
```

## Install

With Cargo:

```
cargo install --path .
```

With Nix:

```
nix profile install .
```

## Use

```
nixdev [dir]                 # guided setup, same as `nixdev init [dir]`
nixdev init . --yes          # accept all defaults, no prompts
nixdev init . --lang rust,c --pkg yyjson --no-index
nixdev add dbus              # add packages to the managed block
nixdev search json           # query the cached nixpkgs index
nixdev doctor                # check nix, git, formatters, cache
nixdev -v                    # print the version (also --version)
```

The wizard asks for the project name, description, languages, optional tools, extra nixpkgs packages, target systems, and the shell to enter. Optional tools are grouped by the languages you selected first, then general-purpose tools below them. Extra nixpkgs packages can be searched for in the same flow — the prompt says `other nixpkgs to be included` so it is clear why it is there.

Generation validates the flake, writes `.envrc`, updates `.gitignore`, initialises git when needed, locks the flake, and formats it when `nixfmt` or `alejandra` is available.

Packages are searched against a cached index built once per nixpkgs revision; if the cache is unavailable nixdev falls back to a live `nix search`.

## Dev shell

Entering the shell with `nix develop` clears the terminal and shows a nixdev banner with the project name, languages, and a hint for leaving. An existing `fastfetch` greeting from your shell rc is muted for the session so the two greetings do not stack. The hook only runs for interactive shells; `nix develop -c <cmd>` stays quiet and still runs the command in bash, so scripts keep working.

## Notes

- Existing `flake.nix` files are backed up before replacement.
- Writes are atomic; the new file is parsed with `nix-instantiate` before it replaces anything.
- The package index and lock files live in `~/.cache/nixdev`.
- `--no-color`, `NO_COLOR`, and `FORCE_COLOR` behave as expected.

## License

MIT
