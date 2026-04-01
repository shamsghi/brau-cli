#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryScope {
    All,
    Formula,
    Cask,
}

impl QueryScope {
    pub fn includes(self, package_kind: crate::catalog::PackageKind) -> bool {
        matches!(
            (self, package_kind),
            (Self::All, _)
                | (Self::Formula, crate::catalog::PackageKind::Formula)
                | (Self::Cask, crate::catalog::PackageKind::Cask)
        )
    }
}

#[derive(Debug)]
pub enum CommandKind {
    Default {
        parts: Vec<String>,
        limit: usize,
    },
    Search {
        query: String,
        limit: usize,
    },
    Info {
        query: String,
    },
    Install {
        queries: Vec<String>,
        yes: bool,
        dry_run: bool,
    },
    Uninstall {
        queries: Vec<String>,
        yes: bool,
        dry_run: bool,
    },
    Brew {
        args: Vec<String>,
    },
    Refresh,
    Help,
}

#[derive(Debug)]
pub struct Cli {
    pub scope: QueryScope,
    pub force_refresh: bool,
    pub no_anim: bool,
    pub no_finale: bool,
    pub command: CommandKind,
}

impl Cli {
    pub fn parse<I>(args: I) -> Result<Self, String>
    where
        I: IntoIterator<Item = String>,
    {
        let mut scope = QueryScope::All;
        let mut force_refresh = false;
        let mut limit = 6usize;
        let mut yes = false;
        let mut dry_run = false;
        let mut no_anim = false;
        let mut no_finale = false;
        let mut command_name: Option<String> = None;
        let mut positionals = Vec::new();
        let mut passthrough = false;

        let mut iter = args.into_iter();
        while let Some(arg) = iter.next() {
            if passthrough {
                positionals.push(arg);
                continue;
            }

            match arg.as_str() {
                "--" => passthrough = true,
                "-h" | "--help" | "help"
                    if positionals.is_empty()
                        && matches!(
                            command_name.as_deref(),
                            None | Some("search" | "info" | "install" | "uninstall" | "refresh")
                        ) =>
                {
                    return Ok(Self {
                        scope,
                        force_refresh,
                        no_anim,
                        no_finale,
                        command: CommandKind::Help,
                    })
                }
                "--refresh" => force_refresh = true,
                "--no-anim" => no_anim = true,
                "--no-finale" => no_finale = true,
                "--formula" | "--formulae" => scope = QueryScope::Formula,
                "--cask" | "--casks" => scope = QueryScope::Cask,
                "-n" | "--limit" => {
                    let value = iter
                        .next()
                        .ok_or_else(|| "Expected a number after `--limit`.".to_string())?;
                    limit = value
                        .parse::<usize>()
                        .map_err(|_| format!("`{value}` is not a valid result limit."))?;
                    if limit == 0 {
                        return Err("`--limit` must be at least 1.".to_string());
                    }
                }
                "-y" | "--yes"
                    if matches!(command_name.as_deref(), Some("install" | "uninstall")) =>
                {
                    yes = true
                }
                "--dry-run" if matches!(command_name.as_deref(), Some("install" | "uninstall")) => {
                    dry_run = true
                }
                "search" | "info" | "install" | "uninstall" | "refresh"
                    if command_name.is_none() =>
                {
                    command_name = Some(arg);
                }
                "brew" | "run" if command_name.is_none() => {
                    command_name = Some(arg);
                    passthrough = true;
                }
                flag if flag.starts_with('-') => positionals.push(flag.to_string()),
                _ => {
                    positionals.push(arg);
                }
            }
        }

        let command = match command_name.as_deref() {
            Some("refresh") => CommandKind::Refresh,
            Some("info") => CommandKind::Info {
                query: join_query(positionals)?,
            },
            Some("install") => CommandKind::Install {
                queries: split_queries(join_query(positionals)?),
                yes,
                dry_run,
            },
            Some("uninstall") => CommandKind::Uninstall {
                queries: split_queries(join_query(positionals)?),
                yes,
                dry_run,
            },
            Some("brew") | Some("run") => CommandKind::Brew { args: positionals },
            Some("search") => CommandKind::Search {
                query: join_query(positionals)?,
                limit,
            },
            None => {
                if positionals.is_empty() {
                    CommandKind::Help
                } else {
                    CommandKind::Default {
                        parts: positionals,
                        limit,
                    }
                }
            }
            Some(other) => return Err(format!("Unknown command: `{other}`.")),
        };

        Ok(Self {
            scope,
            force_refresh,
            no_anim,
            no_finale,
            command,
        })
    }

    pub fn help_text() -> &'static str {
        "brau 0.1.0

Fuzzy search Homebrew formulae and casks, show richer package details,
and install matches directly from inside the CLI.

`search` is the default command, so `brau rg` works as shorthand for
`brau search rg`. Separate multiple packages with commas:
`brau install foo, bar, baz`.

Bare Homebrew commands such as `brau update` or `brau cleanup --prune=all`
also pass through to `brew`.

USAGE:
    brau [OPTIONS] <query...>
    brau search [OPTIONS] <query...>
    brau info [OPTIONS] <query..>
    brau install [OPTIONS] <query[, query...]>
    brau uninstall [OPTIONS] <query[, query...]>
    brau brew <brew-command...>
    brau refresh

OPTIONS:
    --formula, --formulae    Search only formulae
    --cask, --casks          Search only casks
    --refresh                Rebuild the local package cache before running
    --no-anim                Disable search/install motion touches
    --no-finale              Disable the post-install ASCII finale
    -n, --limit <count>      Number of matches to show in search mode (default: 6)
    -y, --yes                Skip confirmation in install/uninstall mode
    --dry-run                Print the brew command instead of running it
    -h, --help               Show this help text

EXAMPLES:
    brau ripgrap
    brau vscode --cask
    brau info docker desktop
    brau install rg
    brau install ripgrep, bat, fd
    brau uninstall ripgrep --yes
    brau uninstall bat, fd --yes
    brau update
    brau brew cleanup
    brau install google chrome --cask
    brau install ripgrep --no-finale
    brau refresh
"
    }
}

fn join_query(parts: Vec<String>) -> Result<String, String> {
    let query = parts.join(" ").trim().to_string();
    if query.is_empty() {
        Err("Please provide a package query.".to_string())
    } else {
        Ok(query)
    }
}

fn split_queries(combined: String) -> Vec<String> {
    combined
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{Cli, CommandKind};

    #[test]
    fn bare_query_defaults_to_search() {
        let cli = Cli::parse(vec!["ripgrap".to_string()]).expect("cli should parse");

        match cli.command {
            CommandKind::Default { parts, limit } => {
                assert_eq!(parts, vec!["ripgrap"]);
                assert_eq!(limit, 6);
            }
            other => panic!("expected default command, got {other:?}"),
        }
    }

    #[test]
    fn uninstall_command_parses_flags() {
        let cli = Cli::parse(vec![
            "uninstall".to_string(),
            "--yes".to_string(),
            "--dry-run".to_string(),
            "--no-finale".to_string(),
            "ripgrep".to_string(),
        ])
        .expect("cli should parse");

        assert!(cli.no_finale);

        match cli.command {
            CommandKind::Uninstall {
                queries,
                yes,
                dry_run,
            } => {
                assert_eq!(queries, vec!["ripgrep"]);
                assert!(yes);
                assert!(dry_run);
            }
            other => panic!("expected uninstall command, got {other:?}"),
        }
    }

    #[test]
    fn brew_passthrough_collects_remaining_args() {
        let cli = Cli::parse(vec![
            "brew".to_string(),
            "cleanup".to_string(),
            "--prune=all".to_string(),
        ])
        .expect("cli should parse");

        match cli.command {
            CommandKind::Brew { args } => {
                assert_eq!(args, vec!["cleanup", "--prune=all"]);
            }
            other => panic!("expected brew passthrough, got {other:?}"),
        }
    }

    #[test]
    fn default_command_keeps_passthrough_flags() {
        let cli = Cli::parse(vec!["cleanup".to_string(), "--prune=all".to_string()])
            .expect("cli should parse");

        match cli.command {
            CommandKind::Default { parts, limit } => {
                assert_eq!(parts, vec!["cleanup", "--prune=all"]);
                assert_eq!(limit, 6);
            }
            other => panic!("expected default command, got {other:?}"),
        }
    }

    #[test]
    fn leading_brew_flag_is_collected_for_default_passthrough() {
        let cli = Cli::parse(vec!["--version".to_string()]).expect("cli should parse");

        match cli.command {
            CommandKind::Default { parts, .. } => {
                assert_eq!(parts, vec!["--version"]);
            }
            other => panic!("expected default command, got {other:?}"),
        }
    }

    #[test]
    fn passthrough_help_flag_after_command_is_not_intercepted() {
        let cli = Cli::parse(vec!["cleanup".to_string(), "--help".to_string()])
            .expect("cli should parse");

        match cli.command {
            CommandKind::Default { parts, .. } => {
                assert_eq!(parts, vec!["cleanup", "--help"]);
            }
            other => panic!("expected default command, got {other:?}"),
        }
    }

    #[test]
    fn search_help_still_returns_help() {
        let cli =
            Cli::parse(vec!["search".to_string(), "--help".to_string()]).expect("cli should parse");

        assert!(matches!(cli.command, CommandKind::Help));
    }
}
