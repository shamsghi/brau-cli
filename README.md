# 🍻 brau

**The Homebrew companion that actually installs what you meant.**

Tired of memorizing exact package names or getting errors for simple typos?

`brau` is a cozy wrapper around Homebrew that guesses what you *actually* meant. It searches, it spell-checks, it installs, and it makes your terminal look good doing it. ✨

*(And yes, it is written in Rust. Because for obvious reasons, all new CLI tools must be blazingly fast™ and written in Rust.)*

## 📋 Prerequisites

Before installing `brau`, make sure you have:

- **macOS** (Homebrew is macOS-only)
- **Homebrew** — if you don't have it yet, install it with:

```bash
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
```

## 🚀 Quick Install

You can install `brau` directly via Homebrew using our custom tap! Just run:

```bash
# Tap the repository (tells Homebrew where to find it)
brew tap shamsghi/brau-cli https://github.com/shamsghi/brau-cli

# Install the magic
brew install brau
```

## 🎉 Why `brau`?

- **🧠 It reads your mind:** Fuzzy-searches both formulae and casks. Looking for `postgress` but the package is actually `postgresql@14`? No problem!
- **⚡ Blazing fast:** Builds a local cache of Homebrew so you never have to wait for a slow `brew search` again.
- **🎬 Dramatic flair:** Adds fun animations and ASCII finales to your everyday installs.
- **🛠️ One CLI to rule them all:** Pass your regular Homebrew commands (`brau update`, `brau cleanup`) straight through!

## 🪄 Magic Tricks (Usage)

You can use `brau` exactly like you'd use `brew`. It just works better.

**Find things (even if you can't spell them):**
```bash
brau postgress           # Wait, did you mean postgresql? Yes, we did.
brau vscode --cask       # Search specifically for casks
```

**Install & Uninstall with ease:**
```bash
brau install pg          # Finds the best match and installs it!
brau uninstall postgresql # Say goodbye
```

**Do regular Homebrew stuff:**
```bash
brau update              # Passes straight to brew
brau cleanup --prune=all
```

## ⚙️ How It Works

When you run `brau`, here's what happens under the hood:

1. **Local cache** — On first run, `brau` calls `brew info --json=v2 --all` to snapshot every formula and cask into a local cache (`~/.cache/brau/`). This takes a few seconds once, then never again.
2. **Smart invalidation** — Instead of using a dumb timer, `brau` fingerprints the `HEAD` commit of each of your Homebrew tap repos. If nothing changed in your taps, the cache is reused instantly.
3. **Multi-factor fuzzy search** — Your query is scored against every package using a combination of:
   - Exact / prefix / contains matching on names, aliases, and old names
   - Word-overlap scoring
   - Subsequence scoring
   - Bounded Levenshtein distance (typo correction)
   - Acronym matching (e.g. `pg` → `postgresql`)
4. **Ranked results** — Matches are ranked by score, with installed packages getting a small boost and a length penalty discouraging overly generic matches.
5. **You confirm, brew executes** — `brau` picks the best match, asks you to confirm (unless `-y` is passed), then hands off the final command straight to `brew`.

## ⚔️ `brew` vs `brau`

| Scenario | `brew` | `brau` |
|---|---|---|
| Typo in package name | ❌ Error | ✅ Figures it out |
| Searching packages | 🐢 Queries network | ⚡ Hits local cache |
| Finding casks | Separate `--cask` flag required | Searches both automatically |
| Acronym search (`pg`, `vsc`) | ❌ No results | ✅ Matches `postgresql`, `visual-studio-code` |
| Old / renamed packages | ❌ Not found | ✅ Matched via old names & aliases |
| Regular brew commands | ✅ Native | ✅ Passed straight through |
| Fun animations | 😐 | 🎉 |

## 🎛️ Cool Flags

Need to tweak things? Try these out:

- `--formula` or `--cask` — Narrow down your searches.
- `-y, --yes` — Skip the install/uninstall confirmation prompts.
- `--no-anim` & `--no-finale` — Turn off the fun animations 😢.
- `--refresh` — Rebuild your local cache to get the absolute freshest packages.

## 💻 For Developers

Want to tinker with the code under the hood and fight the borrow checker? 🦀

```bash
cargo build
cargo test
cargo run -- postgress
```
