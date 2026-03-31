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

### 1. Tap the repository (tells Homebrew where to find it)
```bash
brew tap shamsghi/brau-cli https://github.com/shamsghi/brau-cli
```
### 2. Install brau (HEAD-only formula, no release tags yet)
```bash
brew install brau --HEAD
```

## 🎉 Why `brau`?

- **🧠 It reads your mind:** Fuzzy-searches both formulae and casks. Looking for `postgress` but the package is actually `postgresql@14`? No problem!
- **⚡ Blazing fast:** Builds a local cache of Homebrew so you never have to wait for a slow `brew search` again.
- **🎬 Dramatic flair:** Adds fun animations and ASCII finales to your everyday installs.
- **🛠️ One CLI to rule them all:** Pass your regular Homebrew commands (`brau update`, `brau cleanup`) straight through!

## 🪄 Usage

You can use `brau` exactly like you'd use `brew`. It just works better.

**Find things:**
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

### How It Works

When you run `brau`, here's what happens under the hood:

1. **Builds a local catalog once** — On first run, `brau` asks Homebrew for all formulae and casks, then saves them in a local cache.
2. **Refreshes only when needed** — It checks your tap repos for changes and reuses the cache if nothing changed.
3. **Finds what you meant** — It uses fuzzy matching (typos, aliases, acronyms, partial names) to pick the best package.
4. **Ranks the best options** — Results are scored so the most likely match appears first.
5. **Runs brew for real** — After confirmation (or `-y`), `brau` executes the actual `brew` command.

Want to tinker with the code under the hood and fight the borrow checker? 🦀

```bash
cargo build
cargo test
cargo run -- postgress
```
