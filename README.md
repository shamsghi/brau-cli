# brewfind

`brewfind` is a fuzzy Homebrew search CLI written in Rust. It builds a local
cache of formula and cask metadata, guesses the package you probably meant, shows
more package context than `brew search`, and can install the chosen result
directly from inside the CLI.

`search` is the default command, so `brewfind ripgrap` and
`brewfind search ripgrap` do the same thing.

## What it does

- Fuzzy-searches both formulae and casks
- Handles typos, aliases, display names, and partial queries
- Shows descriptions, versions, taps, aliases, homepages, and dependencies
- Installs the best match directly with `brewfind install <query>`
- Caches Homebrew metadata locally to keep later searches fast

## Examples

```bash
brewfind ripgrap
brewfind vscode --cask
brewfind info docker desktop
brewfind install rg
brewfind install google chrome --cask
brewfind refresh
```

## Commands

```text
brewfind [OPTIONS] <query...>
brewfind search [OPTIONS] <query...>
brewfind info [OPTIONS] <query...>
brewfind install [OPTIONS] <query...>
brewfind refresh
```

Useful flags:

- `--formula` searches only formulae
- `--cask` searches only casks
- `--refresh` rebuilds the local metadata cache first
- `-n, --limit <count>` changes the number of search results
- `-y, --yes` skips the install confirmation prompt
- `--dry-run` prints the `brew install` command without running it

## How it works

On the first run, or whenever the cache is stale, `brewfind` refreshes its index
with:

```bash
brew info --json=v2 --eval-all
```

That metadata is stored locally under the normal cache directory:

- macOS: `~/Library/Caches/brewfind/catalog-v1.json`
- Linux: `$XDG_CACHE_HOME/brewfind/catalog-v1.json` or `~/.cache/brewfind/catalog-v1.json`

The first refresh can take a little while because it indexes the full Homebrew
catalog. Later searches read from the cache.

## Local development

```bash
cargo fmt
cargo test
cargo run -- ripgrap
cargo run -- install rg --dry-run
```

## Homebrew tap

This repository includes a tap formula at `Formula/brewfind.rb`, so the repo can
act as its own tap once it is published to GitHub.

Assuming the repo lives at `https://github.com/pancake/brewfind`, users can install it with:

```bash
brew tap pancake/brewfind https://github.com/pancake/brewfind
brew install brewfind
```

Right now the included formula tracks the `main` branch so the tap works before
you cut a tagged release. Once you publish stable release tags, update the
formula to point at a versioned tag or archive URL for better reproducibility.

If you publish under a different GitHub owner or repo slug, update the tap
command plus the `homepage`, `url`, and `head` entries in `Formula/brewfind.rb`.
