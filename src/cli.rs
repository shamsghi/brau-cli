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
    Search {
        query: String,
        limit: usize,
    },
    Info {
        query: String,
    },
    Install {
        query: String,
        yes: bool,
        dry_run: bool,
    },
    Refresh,
    Help,
}

#[derive(Debug)]
pub struct Cli {
    pub scope: QueryScope,
    pub force_refresh: bool,
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
                "-h" | "--help" | "help" => {
                    return Ok(Self {
                        scope,
                        force_refresh,
                        command: CommandKind::Help,
                    })
                }
                "--refresh" => force_refresh = true,
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
                "-y" | "--yes" => yes = true,
                "--dry-run" => dry_run = true,
                "search" | "info" | "install" | "refresh" if command_name.is_none() => {
                    command_name = Some(arg);
                }
                flag if flag.starts_with('-') => {
                    return Err(format!("Unknown flag: `{flag}`.\n\n{}", Self::help_text()));
                }
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
                query: join_query(positionals)?,
                yes,
                dry_run,
            },
            Some("search") => CommandKind::Search {
                query: join_query(positionals)?,
                limit,
            },
            None => {
                if positionals.is_empty() {
                    CommandKind::Help
                } else {
                    CommandKind::Search {
                        query: join_query(positionals)?,
                        limit,
                    }
                }
            }
            Some(other) => return Err(format!("Unknown command: `{other}`.")),
        };

        Ok(Self {
            scope,
            force_refresh,
            command,
        })
    }

    pub fn help_text() -> &'static str {
        "brewfind 0.1.0

Fuzzy search Homebrew formulae and casks, show richer package details,
and install matches directly from inside the CLI.

`search` is the default command, so `brewfind rg` works as shorthand for
`brewfind search rg`.

USAGE:
    brewfind [OPTIONS] <query...>
    brewfind search [OPTIONS] <query...>
    brewfind info [OPTIONS] <query...>
    brewfind install [OPTIONS] <query...>
    brewfind refresh

OPTIONS:
    --formula, --formulae    Search only formulae
    --cask, --casks          Search only casks
    --refresh                Rebuild the local package cache before running
    -n, --limit <count>      Number of matches to show in search mode (default: 6)
    -y, --yes                Skip confirmation in install mode
    --dry-run                Print the brew command instead of running it
    -h, --help               Show this help text

EXAMPLES:
    brewfind ripgrap
    brewfind vscode --cask
    brewfind info docker desktop
    brewfind install rg
    brewfind install google chrome --cask
    brewfind refresh
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

#[cfg(test)]
mod tests {
    use super::{Cli, CommandKind};

    #[test]
    fn bare_query_defaults_to_search() {
        let cli = Cli::parse(vec!["ripgrap".to_string()]).expect("cli should parse");

        match cli.command {
            CommandKind::Search { query, limit } => {
                assert_eq!(query, "ripgrap");
                assert_eq!(limit, 6);
            }
            other => panic!("expected search command, got {other:?}"),
        }
    }
}
