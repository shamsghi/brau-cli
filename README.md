# 🍻 brau — A Smarter Homebrew Experience with fuzzy search

<table>
<tr>
<td width="52%">

### The Homebrew wrapper that actually understands you.

`brau` is a drop-in replacement for `brew` that adds fuzzy search, typo correction, acronym matching, batch installs, and beautiful terminal animations — all without changing how you already work.

**Stop memorizing exact package names.** Just type what you mean and `brau` figures out the rest.

```bash
brau install pg          # Installs postgresql ✓
brau install vsc --cask  # Installs visual-studio-code ✓
brau install ripgrep, bat, fd  # All three, at once ✓
```

Every `brew` command you already know works with `brau` — just with a much better experience on top.

</td>
<td width="48%">

<img src="https://github.com/user-attachments/assets/87f958cb-6a2b-411d-a658-9a086cf6751c" alt="brau demo" />

</td>
</tr>
</table>

---

## ✨ Why brau?

Homebrew is great. But its search can be unforgiving — a small typo returns an error, acronyms go unrecognized, and installing multiple packages means running the same command over and over.

`brau` solves all of that:

- 🔍 **Fuzzy search** — typos, partial names, and acronyms all resolve to the right package
- ⚡ **Instant results** — searches a local cache instead of hitting the network every time
- 📦 **Search formulas and casks together** — no need to remember which flag to use
- 🍹 **Batch installs** — install (or uninstall) multiple packages in one command (with fuzzy search)
- 🎉 **Delightful animations** — your terminal deserves a little personality

---

## 🚀 Installation

`brau` installs through Homebrew itself using a custom tap. Two commands and you're done.

**1. Add the tap**
```bash
brew tap shamsghi/brau-cli https://github.com/shamsghi/brau-cli
```

**2. Install brau**
```bash
brew install brau --HEAD
```

> **Don't have Homebrew yet?** Get it first:
> ```bash
> /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
> ```

---

## 🪄 Usage

`brau` works exactly like `brew` — same commands, same flags — just smarter.

**Search for packages:**
```bash
brau postgres              # Finds postgresql automatically
brau vscode --cask         # Search casks specifically
brau node                  # Searches both formulas and casks at once
```

**Install & uninstall:**
```bash
brau install pg            # Fuzzy-matches and installs postgresql
brau install node -y       # Skip the confirmation prompt
brau uninstall postgresql  # Clean removal
```

**Batch operations:**
```bash
brau install ripgrep, bat, fd           # Install multiple packages at once
brau install chrome, firefox --cask  # Batch cask install
brau uninstall bat, fd --yes            # Batch uninstall, no prompts
```

**Standard Homebrew commands — fully supported:**
```bash
brau update
brau upgrade
brau cleanup --prune=all
brau doctor
```

---

## ⚔️ brau vs. brew

| Feature | `brew` | `brau` |
|---|---|---|
| Typo in package name | ❌ Error | ✅ Corrected automatically |
| Search speed | 🐢 Network request | ⚡ Local cache |
| Formula + cask search | Requires separate flags | ✅ Unified by default |
| Acronym search (`pg`, `vsc`) | ❌ No results | ✅ Resolves to full package names |
| Renamed / aliased packages | ❌ Not found | ✅ Matched via aliases |
| Batch install | One package at a time | ✅ Comma-separated list |
| All standard brew commands | ✅ | ✅ Passed through natively |
| Terminal animations | 😐 | 🎉 |

---

## 🎛️ Flags Reference

| Flag | Description |
|---|---|
| `--formula` | Limit search/install to formulas only |
| `--cask` | Limit search/install to casks only |
| `-y`, `--yes` | Skip confirmation prompts |
| `--no-anim` | Disable animations |
| `--no-finale` | Disable the end-of-install celebration |
| `--refresh` | Force a rebuild of the local package cache |

---

## 🛠️ How It Works

`brau` is a thin, fast layer on top of Homebrew — built in Rust for snappy performance.

1. **Builds a local catalog on first run** — indexes all Homebrew formulae and casks into a local cache.
2. **Stays fresh automatically** — checks your tap repos for updates and only rebuilds the cache when something has changed.
3. **Understands what you meant** — uses fuzzy matching across names, aliases, acronyms, and partial strings to find the best match.
4. **Ranks results intelligently** — scores candidates so the most relevant package surfaces first.
5. **Hands off to brew** — once a match is confirmed, `brau` runs the actual `brew` command under the hood.

---

## 👩‍💻 Contributing

Contributions are welcome! To get started locally:

```bash
cargo build
cargo test
cargo run -- postgres
```

### Before You Open a PR

Found a bug or have a feature idea? Open an issue — feedback of all kinds is appreciated. Low-effort or AI-generated slop PRs will not be reviewed or merged. If you're contributing code, make sure it is intentional, well-reasoned, and clearly explained.

A good PR or issue should answer:
- **What** is the problem or change?
- **Why** does it need to exist?
- **How** does your solution address it?

Vague titles like *"fix bug"* or *"improve performance"* with no description will be closed without review.

---

### Using an AI Agent to Contribute?

That's totally fine — **but you're still responsible** for the quality of what gets submitted. If you're using an agent to help write code or open a PR, give it this prompt to make sure the output meets the bar:

```
You are contributing to `brau`, a Rust CLI tool that wraps Homebrew with fuzzy search,
typo correction, batch installs, and terminal animations.

Repository: https://github.com/shamsghi/brau-cli

Your task: [DESCRIBE THE BUG / FEATURE / CHANGE HERE]

Requirements:
- Read the existing code carefully before making any changes.
- Do not submit a 500+ lines change, if the prompt above will generate something huge -> stop and suggest opening an issue instead first to the user
- Keep changes minimal optimized and focused — do not refactor unrelated code.
- Follow the existing code style and conventions used in the project.
- Write a clear PR title and description that explains what changed, why, and how.
- Do not include unnecessary comments, dead code, or placeholder logic.
- If you're fixing a bug, explain the root cause. If adding a feature, explain the design decision.
- The PR should be ready for human review, not a draft for the maintainer to finish.
```
