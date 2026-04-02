use crate::app;

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
        brew_flags: Vec<String>,
    },
    Uninstall {
        queries: Vec<String>,
        yes: bool,
        dry_run: bool,
        brew_flags: Vec<String>,
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
        let mut brew_flags = Vec::new();
        let mut no_anim = false;
        let mut no_finale = false;
        let mut command_name: Option<String> = None;
        let mut positionals = Vec::new();
        let mut passthrough = false;

        let mut iter = args.into_iter().peekable();
        while let Some(arg) = iter.next() {
            if passthrough {
                positionals.push(arg);
                continue;
            }

            match arg.as_str() {
                "--" => passthrough = true,
                "-h" | "--help"
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
                "help"
                    if positionals.is_empty()
                        && command_name.is_none()
                        && iter.peek().is_none() =>
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
                "-l" | "--limit"
                    if !matches!(command_name.as_deref(), Some("install" | "uninstall")) =>
                {
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
                "-n" if !matches!(command_name.as_deref(), Some("install" | "uninstall")) => {
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
                "-n" | "--dry-run"
                    if matches!(command_name.as_deref(), Some("install" | "uninstall")) =>
                {
                    dry_run = true
                }
                "--desc" | "--eval-all" if matches!(command_name.as_deref(), Some("search")) => {}
                "search" | "info" | "install" | "uninstall" | "refresh"
                    if command_name.is_none() && positionals.is_empty() =>
                {
                    command_name = Some(arg);
                }
                "brew" | "run" if command_name.is_none() && positionals.is_empty() => {
                    command_name = Some(arg);
                    passthrough = true;
                }
                flag if flag.starts_with('-')
                    && matches!(command_name.as_deref(), Some("install" | "uninstall")) =>
                {
                    brew_flags.push(flag.to_string())
                }
                flag if flag.starts_with('-') => positionals.push(flag.to_string()),
                _ => {
                    positionals.push(arg);
                }
            }
        }

        let command = match command_name.as_deref() {
            Some("refresh") => CommandKind::Refresh,
            Some("info") if should_passthrough_info_args(&positionals) => CommandKind::Brew {
                args: prepend_brew_command("info", scope, positionals),
            },
            Some("info") => CommandKind::Info {
                query: join_query(positionals)?,
            },
            Some("search") if should_passthrough_search_args(&positionals) => CommandKind::Brew {
                args: prepend_brew_command("search", scope, positionals),
            },
            Some("install") => CommandKind::Install {
                queries: split_queries(join_query(positionals)?),
                yes,
                dry_run,
                brew_flags,
            },
            Some("uninstall") => CommandKind::Uninstall {
                queries: split_queries(join_query(positionals)?),
                yes,
                dry_run,
                brew_flags,
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

    pub fn help_text() -> String {
        let binary_name = app::display_name();
        let version = app::version();

        format!(
            "{binary_name} v{version}

Fuzzy search Homebrew formulae and casks, show richer package details,
and install matches directly from inside the CLI.

`search` is the default command, so `{binary_name} rg` works as shorthand for
`{binary_name} search rg`. If you pass brew-only flags like `help search` or
`search /regex/`, `{binary_name}` steps aside and forwards them to Homebrew.
Separate multiple packages with commas:
`{binary_name} install foo, bar, baz`.

Bare Homebrew commands such as `{binary_name} update` or `{binary_name} cleanup --prune=all`
also pass through to `brew`.

USAGE:
    {binary_name} [OPTIONS] <query...>
    {binary_name} search [OPTIONS] <query...>
    {binary_name} info [OPTIONS] <query..>
    {binary_name} install [OPTIONS] <query[, query...]>
    {binary_name} uninstall [OPTIONS] <query[, query...]>
    {binary_name} brew <brew-command...>
    {binary_name} refresh

OPTIONS:
    --formula, --formulae    Search only formulae
    --cask, --casks          Search only casks
    --refresh                Rebuild the local package cache before running
    --no-anim                Disable search/install motion touches
    --no-finale              Disable the post-install ASCII finale
    -l, --limit <count>      Number of matches to show in search mode (default: 6)
    -y, --yes                Skip confirmation in install/uninstall mode
    -n, --dry-run            Print the brew command instead of running it
    -h, --help               Show this help text

EXAMPLES:
    {binary_name} ripgrap
    {binary_name} vscode --cask
    {binary_name} info docker desktop
    {binary_name} install rg
    {binary_name} install ripgrep, bat, fd
    {binary_name} install ripgrep -n
    {binary_name} uninstall ripgrep --yes
    {binary_name} uninstall bat, fd --yes
    {binary_name} update
    {binary_name} help search
    {binary_name} brew cleanup
    {binary_name} install google chrome --cask
    {binary_name} install ripgrep --no-finale
    {binary_name} refresh
"
        )
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

fn prepend_brew_command(command: &str, scope: QueryScope, args: Vec<String>) -> Vec<String> {
    let mut full_args = Vec::with_capacity(args.len() + 2);
    full_args.push(command.to_string());
    match scope {
        QueryScope::All => {}
        QueryScope::Formula => full_args.push("--formula".to_string()),
        QueryScope::Cask => full_args.push("--cask".to_string()),
    }
    full_args.extend(args);
    full_args
}

fn should_passthrough_info_args(args: &[String]) -> bool {
    args.is_empty() || args.iter().any(|arg| arg.starts_with('-'))
}

fn should_passthrough_search_args(args: &[String]) -> bool {
    args.is_empty()
        || args.iter().any(|arg| arg.starts_with('-'))
        || matches!(args, [query] if looks_like_brew_regex_query(query))
}

fn looks_like_brew_regex_query(query: &str) -> bool {
    query.len() >= 2 && query.starts_with('/') && query.ends_with('/')
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
            "-n".to_string(),
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
                brew_flags,
            } => {
                assert_eq!(queries, vec!["ripgrep"]);
                assert!(yes);
                assert!(dry_run);
                assert!(brew_flags.is_empty());
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

    #[test]
    fn help_with_a_command_passthroughs_to_brew() {
        let cli =
            Cli::parse(vec!["help".to_string(), "search".to_string()]).expect("cli should parse");

        match cli.command {
            CommandKind::Default { parts, limit } => {
                assert_eq!(parts, vec!["help", "search"]);
                assert_eq!(limit, 6);
            }
            other => panic!("expected default command, got {other:?}"),
        }
    }

    #[test]
    fn explicit_search_regex_passthroughs_to_brew() {
        let cli =
            Cli::parse(vec!["search".to_string(), "/^rip/".to_string()]).expect("cli should parse");

        match cli.command {
            CommandKind::Brew { args } => {
                assert_eq!(args, vec!["search", "/^rip/"]);
            }
            other => panic!("expected brew passthrough, got {other:?}"),
        }
    }

    #[test]
    fn explicit_info_with_brew_flags_passthroughs_to_brew() {
        let cli = Cli::parse(vec![
            "info".to_string(),
            "--json=v2".to_string(),
            "ripgrep".to_string(),
        ])
        .expect("cli should parse");

        match cli.command {
            CommandKind::Brew { args } => {
                assert_eq!(args, vec!["info", "--json=v2", "ripgrep"]);
            }
            other => panic!("expected brew passthrough, got {other:?}"),
        }
    }

    #[test]
    fn explicit_search_passthrough_keeps_scope_flags() {
        let cli = Cli::parse(vec![
            "--cask".to_string(),
            "search".to_string(),
            "/^fire/".to_string(),
        ])
        .expect("cli should parse");

        match cli.command {
            CommandKind::Brew { args } => {
                assert_eq!(args, vec!["search", "--cask", "/^fire/"]);
            }
            other => panic!("expected brew passthrough, got {other:?}"),
        }
    }

    #[test]
    fn install_collects_extra_brew_flags() {
        let cli = Cli::parse(vec![
            "install".to_string(),
            "--verbose".to_string(),
            "--HEAD".to_string(),
            "ripgrap".to_string(),
        ])
        .expect("cli should parse");

        match cli.command {
            CommandKind::Install {
                queries,
                dry_run,
                brew_flags,
                ..
            } => {
                assert_eq!(queries, vec!["ripgrap"]);
                assert!(!dry_run);
                assert_eq!(brew_flags, vec!["--verbose", "--HEAD"]);
            }
            other => panic!("expected install command, got {other:?}"),
        }
    }
}
