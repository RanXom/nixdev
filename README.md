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
```

The wizard asks for the project name, description, languages, optional tools, extra nixpkgs packages, target systems, and the shell to enter. Generation validates the flake, writes `.envrc`, updates `.gitignore`, initialises git when needed, locks the flake, and formats it when `nixfmt` or `alejandra` is available.

Packages are searched against a cached index built once per nixpkgs revision; if the cache is unavailable nixdev falls back to a live `nix search`.

`nix develop` hands off to your login shell. `nix develop -c <cmd>` still runs the command in bash, so scripts keep working.

## Notes

- Existing `flake.nix` files are backed up before replacement.
- Writes are atomic; the new file is parsed with `nix-instantiate` before it replaces anything.
- The package index and lock files live in `~/.cache/nixdev`.
- `--no-color`, `NO_COLOR`, and `FORCE_COLOR` behave as expected.

## License

MIT
