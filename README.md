# brau

`brau` is a fuzzy Homebrew search CLI written in Rust. It builds a local
cache of formula and cask metadata, guesses the package you probably meant, shows
more package context than `brew search`, and can install the chosen result
directly from inside the CLI. It can also act as a cozy wrapper for regular
Homebrew commands.

`search` is the default command, so `brau ripgrap` and
`brau search ripgrap` do the same thing. Bare Homebrew verbs like
`brau update` or `brau cleanup --prune=all` pass straight through to
`brew` with the same formatting and motion touches.

## What it does

- Fuzzy-searches both formulae and casks
- Handles typos, aliases, display names, and partial queries
- Shows descriptions, versions, taps, aliases, homepages, and dependencies
- Installs the best match directly with `brau install <query>`
- Uninstalls packages directly with `brau uninstall <query>`
- Passes regular Homebrew commands through with extra formatting and animation
- Caches Homebrew metadata locally to keep later searches fast

## Examples

```bash
brau ripgrap
brau vscode --cask
brau info docker desktop
brau install rg
brau uninstall ripgrep --yes
brau update
brau cleanup --prune=all
brau brew --version
brau install google chrome --cask
brau install ripgrep --no-finale
brau refresh
```

## Commands

```text
brau [OPTIONS] <query...>
brau search [OPTIONS] <query...>
brau info [OPTIONS] <query...>
brau install [OPTIONS] <query...>
brau uninstall [OPTIONS] <query...>
brau brew <brew-command...>
brau refresh
```

Useful flags:

- `--formula` searches only formulae
- `--cask` searches only casks
- `--refresh` rebuilds the local metadata cache first
- `--no-anim` disables the motion touches
- `--no-finale` disables the post-install ASCII finale
- `-n, --limit <count>` changes the number of search results
- `-y, --yes` skips the install or uninstall confirmation prompt
- `--dry-run` prints the brew command without running it

For direct Homebrew passthrough, you can either:

- run a bare brew command like `brau update`
- use explicit passthrough like `brau brew services list`
- use `brau brew --version` for brew's global flag-style commands

## How it works

On the first run, or whenever the cache is stale, `brau` refreshes its index
with:

```bash
brew info --json=v2 --eval-all
```

That metadata is stored locally under the normal cache directory:

- macOS: `~/Library/Caches/brau/catalog-v1.json`
- Linux: `$XDG_CACHE_HOME/brau/catalog-v1.json` or `~/.cache/brau/catalog-v1.json`

The first refresh can take a little while because it indexes the full Homebrew
catalog. Later searches read from the cache.

## Local development

```bash
cargo fmt
cargo test
cargo run -- ripgrap
cargo run -- install rg --dry-run
cargo run -- uninstall ripgrep --yes --dry-run
cargo run -- update
cargo run -- brew --version
```

## Homebrew tap

This repository includes a tap formula at `Formula/brau.rb`, so the repo can
act as its own tap once it is published to GitHub.

Assuming the repo lives at `https://github.com/pancake/brau`, users can install it with:

```bash
brew tap pancake/brau https://github.com/pancake/brau
brew install brau
```

Right now the included formula tracks the `main` branch so the tap works before
you cut a tagged release. Once you publish stable release tags, update the
formula to point at a versioned tag or archive URL for better reproducibility.

If you publish under a different GitHub owner or repo slug, update the tap
command plus the `homepage`, `url`, and `head` entries in `Formula/brau.rb`.
