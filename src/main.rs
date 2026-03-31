mod catalog;
mod cli;
mod render;
mod search;

use std::io::{self, IsTerminal, Write};
use std::process::{Command, ExitCode, Stdio};

use catalog::{Catalog, CatalogLoadSource, Package, PackageKind};
use cli::{Cli, CommandKind, QueryScope};
use search::{search_catalog, MatchStrength, SearchMatch, SearchOptions};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let cli = Cli::parse(std::env::args().skip(1))?;

    match &cli.command {
        CommandKind::Help => {
            print!("{}", Cli::help_text());
            return Ok(());
        }
        CommandKind::Refresh => {
            eprintln!("Refreshing Homebrew catalog cache...");
            let load = catalog::load_catalog(catalog::LoadOptions {
                force_refresh: true,
                allow_stale_fallback: false,
            })?;
            println!(
                "Cached {} packages ({} formulae, {} casks).",
                load.catalog.total_count(),
                load.catalog.formula_count(),
                load.catalog.cask_count()
            );
            return Ok(());
        }
        CommandKind::Search { .. } | CommandKind::Info { .. } | CommandKind::Install { .. } => {}
    }

    let load = catalog::load_catalog(catalog::LoadOptions {
        force_refresh: cli.force_refresh,
        allow_stale_fallback: true,
    })?;

    match load.source {
        CatalogLoadSource::Refreshed => eprintln!(
            "Refreshed Homebrew catalog ({} packages).",
            load.catalog.total_count()
        ),
        CatalogLoadSource::StaleFallback => {
            if let Some(warning) = load.warning.as_deref() {
                eprintln!("Using a stale cache because refresh failed: {warning}");
            }
        }
        CatalogLoadSource::Cache => {}
    }

    match cli.command {
        CommandKind::Search { query, limit } => run_search(&load.catalog, &query, cli.scope, limit),
        CommandKind::Info { query } => run_info(&load.catalog, &query, cli.scope),
        CommandKind::Install {
            query,
            yes,
            dry_run,
        } => run_install(&load.catalog, &query, cli.scope, yes, dry_run),
        CommandKind::Refresh | CommandKind::Help => Ok(()),
    }
}

fn run_search(
    catalog: &Catalog,
    query: &str,
    scope: QueryScope,
    limit: usize,
) -> Result<(), String> {
    let matches = search_catalog(catalog, query, SearchOptions { scope, limit });

    render::play_search_charm(query);
    render::print_search_results(query, &matches);

    if matches.is_empty() {
        return Err(format!(
            "No Homebrew packages matched \"{query}\". Try `brewfind refresh` or a broader search."
        ));
    }

    Ok(())
}

fn run_info(catalog: &Catalog, query: &str, scope: QueryScope) -> Result<(), String> {
    let matches = search_catalog(catalog, query, SearchOptions { scope, limit: 5 });

    let best = matches
        .first()
        .ok_or_else(|| format!("No Homebrew packages matched \"{query}\"."))?;

    render::play_search_charm(query);
    render::print_package_detail(best.package);

    if matches.len() > 1 {
        let alternatives = &matches[1..];
        if !alternatives.is_empty() {
            println!();
            render::print_subsection("🧭", "Close alternatives");
            render::print_match_list(alternatives, 2);
        }
    }

    println!();
    render::print_footer("end of package details");

    Ok(())
}

fn run_install(
    catalog: &Catalog,
    query: &str,
    scope: QueryScope,
    yes: bool,
    dry_run: bool,
) -> Result<(), String> {
    let matches = search_catalog(catalog, query, SearchOptions { scope, limit: 5 });

    let best = matches
        .first()
        .ok_or_else(|| format!("No Homebrew packages matched \"{query}\"."))?;

    render::play_search_charm(query);
    let confident = is_confident(best, matches.get(1));

    if yes {
        if !confident {
            render::print_install_candidates(query, &matches);
            return Err(
                "The query is ambiguous. Rerun without `--yes` to choose a match, or be more specific."
                    .to_string(),
            );
        }

        return install_package(best.package, dry_run);
    }

    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Err(
            "Interactive installs require a terminal. Use `--yes` with a specific query instead."
                .to_string(),
        );
    }

    if confident {
        render::print_install_preview(query, best.package, best);

        if prompt_yes_no(&format!(
            "Install {} ({}) now?",
            best.package.token,
            best.package.kind.label()
        ))? {
            install_package(best.package, dry_run)?;
        } else {
            println!("Install cancelled.");
        }

        return Ok(());
    }

    render::print_install_candidates(query, &matches);
    let selected = prompt_match_selection(&matches)?;
    install_package(selected.package, dry_run)
}

fn install_package(package: &Package, dry_run: bool) -> Result<(), String> {
    let mut args = vec!["install".to_string()];
    if package.kind == PackageKind::Cask {
        args.push("--cask".to_string());
    }
    args.push(package.install_target().to_string());

    if dry_run {
        println!("Dry run: brew {}", args.join(" "));
        return Ok(());
    }

    eprintln!("Running: brew {}", args.join(" "));

    let status = Command::new("brew")
        .args(&args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|error| format!("Failed to launch Homebrew: {error}"))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!("Homebrew exited with status {status}."))
    }
}

fn is_confident(best: &SearchMatch<'_>, next: Option<&SearchMatch<'_>>) -> bool {
    match best.strength {
        MatchStrength::Exact => true,
        MatchStrength::Strong => next.map_or(true, |candidate| best.score - candidate.score >= 90),
        MatchStrength::Good => next.map_or(false, |candidate| {
            best.score >= 1_150 && best.score - candidate.score >= 180
        }),
        MatchStrength::Fuzzy => false,
    }
}

fn prompt_yes_no(prompt: &str) -> Result<bool, String> {
    loop {
        print!("{prompt} [Y/n] ");
        io::stdout()
            .flush()
            .map_err(|error| format!("Failed to flush stdout: {error}"))?;

        let mut answer = String::new();
        io::stdin()
            .read_line(&mut answer)
            .map_err(|error| format!("Failed to read your answer: {error}"))?;

        match answer.trim().to_ascii_lowercase().as_str() {
            "" | "y" | "yes" => return Ok(true),
            "n" | "no" => return Ok(false),
            _ => println!("Please answer with `y` or `n`."),
        }
    }
}

fn prompt_match_selection<'a>(
    matches: &'a [SearchMatch<'a>],
) -> Result<&'a SearchMatch<'a>, String> {
    loop {
        print!("Choose a package to install [1-{} or q]: ", matches.len());
        io::stdout()
            .flush()
            .map_err(|error| format!("Failed to flush stdout: {error}"))?;

        let mut answer = String::new();
        io::stdin()
            .read_line(&mut answer)
            .map_err(|error| format!("Failed to read your answer: {error}"))?;

        let trimmed = answer.trim();
        if trimmed.eq_ignore_ascii_case("q") {
            return Err("Install cancelled.".to_string());
        }

        if let Ok(index) = trimmed.parse::<usize>() {
            if let Some(selected) = matches.get(index.saturating_sub(1)) {
                return Ok(selected);
            }
        }

        println!(
            "Please enter a number between 1 and {} or `q`.",
            matches.len()
        );
    }
}
