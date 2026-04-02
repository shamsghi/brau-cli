mod app;
mod brew;
mod catalog;
mod cli;
mod motion;
mod prompt;
mod render;
mod search;

use std::io::{self, IsTerminal};
use std::process::ExitCode;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

use app::BroAliasStatus;
use brew::{brew_command_display, refresh_brew_command_cache, run_brew_command};
use catalog::{CacheStatus, Catalog, CatalogLoad, CatalogLoadSource, Package, PackageKind};
use cli::{Cli, CommandKind, QueryScope};
use motion::{run_with_motion, should_run_motion, MotionSettings};
use prompt::{
    prompt_batch_retry_selection, prompt_batch_review_choice, prompt_confirmed_match_choice,
    prompt_match_selection, prompt_match_selection_choice, prompt_yes_no, BatchReviewChoice,
    ConfirmedMatchChoice, MatchSelection,
};
use render::CatalogWarmupKind;
use search::{search_catalog, MatchStrength, SearchMatch, SearchOptions};

const ACTION_MATCH_LIMIT: usize = 5;
const BATCH_MATCH_LIMIT: usize = 6;
const BATCH_FUZZY_MATCH_LIMIT: usize = 8;

#[derive(Clone, Copy, PartialEq, Eq)]
enum BrewAction {
    Install,
    Uninstall,
}

#[derive(Clone, Copy)]
enum CatalogLoadReason {
    FirstRun,
    StaleRefresh,
    ManualRefresh,
}

#[derive(Debug)]
struct BatchResolvedPackage<'a> {
    query: String,
    package: &'a Package,
}

impl BrewAction {
    fn command(self) -> &'static str {
        match self {
            Self::Install => "install",
            Self::Uninstall => "uninstall",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Install => "install",
            Self::Uninstall => "uninstall",
        }
    }

    fn present_participle(self) -> &'static str {
        match self {
            Self::Install => "installing",
            Self::Uninstall => "uninstalling",
        }
    }

    fn past_participle(self) -> &'static str {
        match self {
            Self::Install => "installed",
            Self::Uninstall => "uninstalled",
        }
    }

    fn preview_title(self) -> &'static str {
        match self {
            Self::Install => "Ready to install",
            Self::Uninstall => "Ready to uninstall",
        }
    }

    fn prompt(self, package: &Package) -> String {
        match self {
            Self::Install => format!("Install {} ({}) now?", package.token, package.kind.label()),
            Self::Uninstall => format!(
                "Uninstall {} ({}) now?",
                package.token,
                package.kind.label()
            ),
        }
    }

    fn preview_footer(self) -> &'static str {
        match self {
            Self::Install => "press y to install, or n to cancel",
            Self::Uninstall => "press y to uninstall, or n to cancel",
        }
    }

    fn batch_preview_footer(self) -> &'static str {
        "press y to keep this match, n to search again, or q to cancel"
    }

    fn candidates_footer(self) -> &'static str {
        match self {
            Self::Install => "choose a number to install, or q to cancel",
            Self::Uninstall => "choose a number to uninstall, or q to cancel",
        }
    }

    fn should_celebrate(self) -> bool {
        matches!(self, Self::Install)
    }
}

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
    let raw_args = std::env::args().skip(1).collect::<Vec<_>>();
    if app::is_hidden_easter_egg_command(&raw_args) {
        return unlock_bro_alias();
    }

    let cli = Cli::parse(raw_args)?;
    let motion = MotionSettings {
        animations_enabled: !cli.no_anim,
        finale_enabled: !cli.no_anim && !cli.no_finale,
    };

    match &cli.command {
        CommandKind::Help => {
            render::print_help_screen();
            return Ok(());
        }
        CommandKind::Refresh => {
            let (load, _) = load_catalog_with_feedback(
                catalog::LoadOptions {
                    force_refresh: true,
                    allow_stale_fallback: false,
                },
                motion,
            )?;
            if let Err(error) = refresh_brew_command_cache() {
                eprintln!("Warning: could not refresh brew command cache: {error}");
            }
            println!(
                "Cached {} packages ({} formulae, {} casks).",
                load.catalog.total_count(),
                load.catalog.formula_count(),
                load.catalog.cask_count()
            );
            return Ok(());
        }
        CommandKind::Brew { args } => {
            return run_brew_passthrough(args, motion);
        }
        CommandKind::Default { parts, limit } => {
            if should_passthrough_default(parts, cli.scope, cli.force_refresh, *limit)? {
                return run_brew_passthrough(parts, motion);
            }
        }
        CommandKind::Search { .. }
        | CommandKind::Info { .. }
        | CommandKind::Install { .. }
        | CommandKind::Uninstall { .. } => {}
    }

    let (load, load_reason) = load_catalog_with_feedback(
        catalog::LoadOptions {
            force_refresh: cli.force_refresh,
            allow_stale_fallback: true,
        },
        motion,
    )?;

    match load.source {
        CatalogLoadSource::Refreshed => match load_reason {
            Some(CatalogLoadReason::FirstRun) => eprintln!(
                "Built local Homebrew catalog ({} packages).",
                load.catalog.total_count()
            ),
            _ => eprintln!(
                "Refreshed Homebrew catalog ({} packages).",
                load.catalog.total_count()
            ),
        },
        CatalogLoadSource::StaleFallback => {
            if let Some(warning) = load.warning.as_deref() {
                eprintln!("Using a stale cache because refresh failed: {warning}");
            }
        }
        CatalogLoadSource::Cache => {}
    }

    match cli.command {
        CommandKind::Default { parts, limit } => {
            run_search(&load.catalog, &parts.join(" "), cli.scope, limit, motion)
        }
        CommandKind::Search { query, limit } => {
            run_search(&load.catalog, &query, cli.scope, limit, motion)
        }
        CommandKind::Info { query } => run_info(&load.catalog, &query, cli.scope, motion),
        CommandKind::Install {
            queries,
            yes,
            dry_run,
        } => {
            if queries.len() == 1 {
                run_action(
                    &load.catalog,
                    &queries[0],
                    cli.scope,
                    yes,
                    dry_run,
                    motion,
                    BrewAction::Install,
                )
            } else {
                run_batch_action(
                    &load.catalog,
                    &queries,
                    cli.scope,
                    yes,
                    dry_run,
                    motion,
                    BrewAction::Install,
                )
            }
        }
        CommandKind::Uninstall {
            queries,
            yes,
            dry_run,
        } => {
            if queries.len() == 1 {
                run_action(
                    &load.catalog,
                    &queries[0],
                    cli.scope,
                    yes,
                    dry_run,
                    motion,
                    BrewAction::Uninstall,
                )
            } else {
                run_batch_action(
                    &load.catalog,
                    &queries,
                    cli.scope,
                    yes,
                    dry_run,
                    motion,
                    BrewAction::Uninstall,
                )
            }
        }
        CommandKind::Brew { .. } | CommandKind::Refresh | CommandKind::Help => Ok(()),
    }
}

fn load_catalog_with_feedback(
    options: catalog::LoadOptions,
    motion: MotionSettings,
) -> Result<(CatalogLoad, Option<CatalogLoadReason>), String> {
    let reason = expected_catalog_load_reason(options.force_refresh)?;
    let Some(reason) = reason else {
        return Ok((catalog::load_catalog(options)?, None));
    };

    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let result = catalog::load_catalog(options);
        let _ = sender.send(result);
    });

    let start = Instant::now();
    let mut tick = 0usize;

    loop {
        match receiver.recv_timeout(Duration::from_millis(140)) {
            Ok(result) => {
                render::finish_catalog_warmup(motion.animations_enabled);
                return result.map(|load| (load, Some(reason)));
            }
            Err(RecvTimeoutError::Timeout) => {
                render::draw_catalog_warmup_tick(
                    reason.into(),
                    tick,
                    start.elapsed(),
                    motion.animations_enabled,
                );
                tick += 1;
            }
            Err(RecvTimeoutError::Disconnected) => {
                return Err("The catalog loader stopped unexpectedly.".to_string());
            }
        }
    }
}

fn expected_catalog_load_reason(force_refresh: bool) -> Result<Option<CatalogLoadReason>, String> {
    let status = catalog::cache_status()?;

    Ok(match (force_refresh, status) {
        (true, _) => Some(CatalogLoadReason::ManualRefresh),
        (false, CacheStatus::Missing) => Some(CatalogLoadReason::FirstRun),
        (false, CacheStatus::Stale) => Some(CatalogLoadReason::StaleRefresh),
        (false, CacheStatus::Fresh) => None,
    })
}

fn unlock_bro_alias() -> Result<(), String> {
    match app::unlock_bro_alias()? {
        BroAliasStatus::Created(path) => {
            let path = path.display().to_string();
            render::print_bro_alias_unlock(&path, false);
        }
        BroAliasStatus::AlreadyAvailable(path) => {
            let path = path.display().to_string();
            render::print_bro_alias_unlock(&path, true);
        }
    }

    Ok(())
}

impl From<CatalogLoadReason> for CatalogWarmupKind {
    fn from(value: CatalogLoadReason) -> Self {
        match value {
            CatalogLoadReason::FirstRun => Self::FirstRun,
            CatalogLoadReason::StaleRefresh => Self::StaleRefresh,
            CatalogLoadReason::ManualRefresh => Self::ManualRefresh,
        }
    }
}

fn should_passthrough_default(
    parts: &[String],
    scope: QueryScope,
    force_refresh: bool,
    limit: usize,
) -> Result<bool, String> {
    if parts.is_empty() {
        return Ok(false);
    }

    if scope != QueryScope::All || force_refresh || limit != 6 {
        return Ok(false);
    }

    let first = parts[0].as_str();
    if brew::is_known_brew_command(first) {
        return Ok(true);
    }

    if first.starts_with('-') {
        return Err(format!("Unknown flag: `{first}`.\n\n{}", Cli::help_text()));
    }

    Ok(false)
}

fn run_search(
    catalog: &Catalog,
    query: &str,
    scope: QueryScope,
    limit: usize,
    motion: MotionSettings,
) -> Result<(), String> {
    let matches = search_matches(catalog, query, scope, limit, motion)?;
    render::print_search_results(query, &matches);

    if matches.is_empty() {
        return Err(format!(
            "No Homebrew packages matched \"{query}\". Try `{} refresh` or a broader search.",
            app::display_name()
        ));
    }

    Ok(())
}

fn run_info(
    catalog: &Catalog,
    query: &str,
    scope: QueryScope,
    motion: MotionSettings,
) -> Result<(), String> {
    let matches = search_matches(catalog, query, scope, ACTION_MATCH_LIMIT, motion)?;
    let best = matches
        .first()
        .ok_or_else(|| format!("No Homebrew packages matched \"{query}\"."))?;

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

fn run_action(
    catalog: &Catalog,
    query: &str,
    scope: QueryScope,
    yes: bool,
    dry_run: bool,
    motion: MotionSettings,
    action: BrewAction,
) -> Result<(), String> {
    let matches = search_matches(catalog, query, scope, ACTION_MATCH_LIMIT, motion)?;
    let best = matches
        .first()
        .ok_or_else(|| format!("No Homebrew packages matched \"{query}\"."))?;
    let confident = is_confident(best, matches.get(1));

    if yes {
        if !confident {
            render::print_action_candidates(
                query,
                action.label(),
                &matches,
                action.candidates_footer(),
            );
            return Err(
                "The query is ambiguous. Rerun without `--yes` to choose a match, or be more specific."
                    .to_string(),
            );
        }

        return execute_brew_action(best.package, action, dry_run, motion);
    }

    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Err(format!(
            "Interactive {} requires a terminal. Use `--yes` with a specific query instead.",
            action.present_participle()
        )
        .to_string());
    }

    if confident {
        render::print_action_preview(
            query,
            action.preview_title(),
            best.package,
            best,
            action.preview_footer(),
        );

        if prompt_yes_no(&action.prompt(best.package))? {
            execute_brew_action(best.package, action, dry_run, motion)?;
        } else {
            println!("{} cancelled.", capitalize(action.label()));
        }

        return Ok(());
    }

    render::print_action_candidates(query, action.label(), &matches, action.candidates_footer());
    let selected = prompt_match_selection(&matches)?;
    execute_brew_action(selected.package, action, dry_run, motion)
}

fn run_batch_action(
    catalog: &Catalog,
    queries: &[String],
    scope: QueryScope,
    yes: bool,
    dry_run: bool,
    motion: MotionSettings,
    action: BrewAction,
) -> Result<(), String> {
    if yes {
        let resolved = resolve_batch_queries_without_prompt(catalog, queries, scope, action)?;
        let packages = resolved.iter().map(|item| item.package).collect::<Vec<_>>();
        return execute_batch_action(&packages, action, dry_run, motion);
    }

    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Err(format!(
            "Interactive batch {} requires a terminal. Use `--yes` with specific queries instead.",
            action.present_participle()
        ));
    }

    let mut resolved =
        match resolve_batch_queries_interactively(catalog, queries, scope, motion, action)? {
            Some(resolved) => resolved,
            None => {
                println!("{} cancelled.", capitalize(action.label()));
                return Ok(());
            }
        };

    loop {
        let preview = resolved
            .iter()
            .map(|item| (item.query.as_str(), item.package))
            .collect::<Vec<_>>();
        render::print_batch_review(action.label(), &preview);

        match prompt_batch_review_choice()? {
            BatchReviewChoice::Proceed => break,
            BatchReviewChoice::RetryAll => {
                resolved = match resolve_batch_queries_interactively(
                    catalog, queries, scope, motion, action,
                )? {
                    Some(resolved) => resolved,
                    None => {
                        println!("{} cancelled.", capitalize(action.label()));
                        return Ok(());
                    }
                };
            }
            BatchReviewChoice::RetryOne => {
                let Some(index) = prompt_batch_retry_selection(resolved.len())? else {
                    continue;
                };
                let query = resolved[index].query.clone();
                let package = match resolve_batch_query_interactively(
                    catalog,
                    &query,
                    scope,
                    motion,
                    action,
                    index + 1,
                    queries.len(),
                )? {
                    Some(package) => package,
                    None => {
                        println!("{} cancelled.", capitalize(action.label()));
                        return Ok(());
                    }
                };
                resolved[index].package = package;
            }
            BatchReviewChoice::Cancel => {
                println!("{} cancelled.", capitalize(action.label()));
                return Ok(());
            }
        }
    }

    let packages = resolved.iter().map(|item| item.package).collect::<Vec<_>>();
    execute_batch_action(&packages, action, dry_run, motion)
}

fn execute_batch_action(
    resolved: &[&Package],
    action: BrewAction,
    dry_run: bool,
    motion: MotionSettings,
) -> Result<(), String> {
    if resolved.is_empty() {
        return Ok(());
    }

    // Suppress individual finales so the combined batch finale lands once at the end.
    let batch_motion = MotionSettings {
        animations_enabled: motion.animations_enabled,
        finale_enabled: false,
    };

    let mut succeeded: Vec<&Package> = Vec::new();
    let mut failed: Vec<(&Package, String)> = Vec::new();

    for package in resolved {
        match execute_brew_action(package, action, dry_run, batch_motion) {
            Ok(()) => succeeded.push(package),
            Err(error) => {
                eprintln!("Failed to {} {}: {}", action.label(), package.token, error);
                failed.push((package, error));
            }
        }
    }

    if action.should_celebrate() && motion.finale_enabled && !dry_run && !succeeded.is_empty() {
        let tokens: Vec<&str> = succeeded.iter().map(|p| p.token.as_str()).collect();
        render::play_batch_install_finale(&tokens, true);
    }

    if !failed.is_empty() {
        let names: Vec<&str> = failed.iter().map(|(p, _)| p.token.as_str()).collect();
        let count = if failed.len() == 1 {
            "1 package".to_string()
        } else {
            format!("{} packages", failed.len())
        };
        return Err(format!(
            "Failed to {} {}: {}",
            action.label(),
            count,
            names.join(", ")
        ));
    }

    Ok(())
}

fn resolve_batch_queries_without_prompt<'a>(
    catalog: &'a Catalog,
    queries: &[String],
    scope: QueryScope,
    action: BrewAction,
) -> Result<Vec<BatchResolvedPackage<'a>>, String> {
    let mut resolved = Vec::with_capacity(queries.len());

    for query in queries {
        let matches = search_catalog(
            catalog,
            query,
            SearchOptions {
                scope,
                limit: ACTION_MATCH_LIMIT,
            },
        );

        let best = matches
            .first()
            .ok_or_else(|| format!("No Homebrew packages matched \"{query}\"."))?;
        if !is_confident(best, matches.get(1)) {
            return Err(format!(
                "The query \"{query}\" is ambiguous. Be more specific or {} each package individually.",
                action.label()
            ));
        }
        let package = best.package;

        resolved.push(BatchResolvedPackage {
            query: query.clone(),
            package,
        });
    }

    Ok(resolved)
}

fn resolve_batch_queries_interactively<'a>(
    catalog: &'a Catalog,
    queries: &[String],
    scope: QueryScope,
    motion: MotionSettings,
    action: BrewAction,
) -> Result<Option<Vec<BatchResolvedPackage<'a>>>, String> {
    let mut resolved = Vec::with_capacity(queries.len());

    for (index, query) in queries.iter().enumerate() {
        let package = match resolve_batch_query_interactively(
            catalog,
            query,
            scope,
            motion,
            action,
            index + 1,
            queries.len(),
        )? {
            Some(package) => package,
            None => return Ok(None),
        };

        resolved.push(BatchResolvedPackage {
            query: query.clone(),
            package,
        });
    }

    Ok(Some(resolved))
}

fn resolve_batch_query_interactively<'a>(
    catalog: &'a Catalog,
    query: &str,
    scope: QueryScope,
    motion: MotionSettings,
    action: BrewAction,
    index: usize,
    total: usize,
) -> Result<Option<&'a Package>, String> {
    render::print_batch_query_progress(action.label(), query, index, total);

    let matches = search_matches(catalog, query, scope, BATCH_MATCH_LIMIT, motion)?;
    let best = matches
        .first()
        .ok_or_else(|| format!("No Homebrew packages matched \"{query}\"."))?;

    if is_confident(best, matches.get(1)) {
        let package = best.package;
        render::print_action_preview(
            query,
            action.preview_title(),
            package,
            best,
            action.batch_preview_footer(),
        );

        return match prompt_confirmed_match_choice()? {
            ConfirmedMatchChoice::Accept => Ok(Some(package)),
            ConfirmedMatchChoice::SearchAgain => {
                let fuzzy_matches =
                    search_matches(catalog, query, scope, BATCH_FUZZY_MATCH_LIMIT, motion)?;
                render::print_retry_candidates(
                    query,
                    action.label(),
                    &fuzzy_matches,
                    action.candidates_footer(),
                );

                match prompt_match_selection_choice(fuzzy_matches.len())? {
                    MatchSelection::Selected(index) => Ok(Some(fuzzy_matches[index].package)),
                    MatchSelection::Cancelled => Ok(None),
                }
            }
            ConfirmedMatchChoice::Cancel => Ok(None),
        };
    }

    render::print_action_candidates(query, action.label(), &matches, action.candidates_footer());

    match prompt_match_selection_choice(matches.len())? {
        MatchSelection::Selected(index) => Ok(Some(matches[index].package)),
        MatchSelection::Cancelled => Ok(None),
    }
}

fn execute_brew_action(
    package: &Package,
    action: BrewAction,
    dry_run: bool,
    motion: MotionSettings,
) -> Result<(), String> {
    let mut args = vec![action.command().to_string()];
    if package.kind == PackageKind::Cask {
        args.push("--cask".to_string());
    }
    args.push(package.install_target().to_string());

    if dry_run {
        render::play_brew_action_charm(package, action.label(), dry_run, motion.animations_enabled);
        println!("Dry run: brew {}", args.join(" "));
        return Ok(());
    }

    eprintln!("Running: {}", brew_command_display(&args));

    let status = run_brew_command(&args, motion, move || {
        render::play_brew_action_charm(package, action.label(), dry_run, motion.animations_enabled);
    })?;

    if status.success() {
        if action.should_celebrate() && motion.finale_enabled {
            render::play_install_finale(package, true);
        }
        println!(
            "{} {}.",
            capitalize(package.token.as_str()),
            action.past_participle()
        );
        Ok(())
    } else {
        Err(format!("Homebrew exited with status {status}."))
    }
}

fn run_brew_passthrough(args: &[String], motion: MotionSettings) -> Result<(), String> {
    let command = args.first().map(String::as_str).unwrap_or_default();
    let trailing = if args.len() > 1 { &args[1..] } else { &[][..] };

    render::print_brew_command_banner(command, trailing);
    eprintln!("Running: {}", brew_command_display(args));

    let command_owned = command.to_string();
    let trailing_owned = trailing.to_vec();
    let status = run_brew_command(args, motion, move || {
        render::play_brew_command_charm(&command_owned, &trailing_owned, motion.animations_enabled);
    })?;

    render::print_brew_command_footer(
        if command.is_empty() { "help" } else { command },
        status.success(),
    );

    if status.success() {
        Ok(())
    } else {
        Err(format!("Homebrew exited with status {status}."))
    }
}

fn search_matches<'a>(
    catalog: &'a Catalog,
    query: &str,
    scope: QueryScope,
    limit: usize,
    motion: MotionSettings,
) -> Result<Vec<SearchMatch<'a>>, String> {
    run_with_motion(
        should_run_motion(motion),
        || search_catalog(catalog, query, SearchOptions { scope, limit }),
        || render::play_search_charm(query, motion.animations_enabled),
    )
}

fn is_confident(best: &SearchMatch<'_>, next: Option<&SearchMatch<'_>>) -> bool {
    match best.strength {
        MatchStrength::Exact => true,
        MatchStrength::Strong => next.map_or(true, |candidate| best.score - candidate.score >= 90),
        MatchStrength::Good => {
            next.is_some_and(|candidate| best.score >= 1_150 && best.score - candidate.score >= 180)
        }
        MatchStrength::Fuzzy => false,
    }
}

fn capitalize(value: &str) -> String {
    let mut characters = value.chars();
    match characters.next() {
        Some(first) => {
            let mut result = first.to_uppercase().collect::<String>();
            result.push_str(characters.as_str());
            result
        }
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{Catalog, Package, PackageKind};
    use crate::cli::QueryScope;

    fn package(kind: PackageKind, token: &str, aliases: &[&str], desc: &str) -> Package {
        Package {
            kind,
            token: token.to_string(),
            full_token: token.to_string(),
            display_names: Vec::new(),
            aliases: aliases.iter().map(|value| value.to_string()).collect(),
            old_names: Vec::new(),
            desc: desc.to_string(),
            homepage: None,
            version: Some("1.0.0".to_string()),
            tap: None,
            license: None,
            dependencies: Vec::new(),
            installed: false,
            outdated: false,
            deprecated: false,
            disabled: false,
            auto_updates: false,
        }
    }

    #[test]
    fn resolve_batch_queries_without_prompt_accepts_clear_matches() {
        let catalog = Catalog {
            generated_at: 0,
            brew_state: None,
            items: vec![
                package(
                    PackageKind::Formula,
                    "ripgrep",
                    &["rg"],
                    "Search tool like grep",
                ),
                package(PackageKind::Formula, "bat", &[], "Cat clone with wings"),
            ],
        };

        let queries = vec!["rg".to_string(), "bat".to_string()];
        let resolved = resolve_batch_queries_without_prompt(
            &catalog,
            &queries,
            QueryScope::All,
            BrewAction::Install,
        )
        .expect("clear matches should resolve");

        assert_eq!(resolved.len(), 2);
        assert_eq!(resolved[0].package.token, "ripgrep");
        assert_eq!(resolved[1].package.token, "bat");
    }

    #[test]
    fn resolve_batch_queries_without_prompt_rejects_ambiguous_matches() {
        let catalog = Catalog {
            generated_at: 0,
            brew_state: None,
            items: vec![
                package(PackageKind::Formula, "foo-tool", &[], "First foo package"),
                package(PackageKind::Formula, "foo-bar", &[], "Second foo package"),
            ],
        };

        let queries = vec!["foo".to_string()];
        let error = resolve_batch_queries_without_prompt(
            &catalog,
            &queries,
            QueryScope::All,
            BrewAction::Install,
        )
        .expect_err("ambiguous matches should be rejected");

        assert!(error.contains("ambiguous"));
    }
}
