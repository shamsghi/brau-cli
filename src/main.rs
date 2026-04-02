mod catalog;
mod cli;
mod render;
mod search;

use std::collections::HashSet;
use std::env;
use std::fs;
use std::io::{self, IsTerminal, Read, Write};
use std::path::PathBuf;
use std::process::{Command, ExitCode, ExitStatus, Stdio};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

use catalog::{CacheStatus, Catalog, CatalogLoad, CatalogLoadSource, Package, PackageKind};
use cli::{Cli, CommandKind, QueryScope};
use render::CatalogWarmupKind;
use search::{search_catalog, MatchStrength, SearchMatch, SearchOptions};

const BREW_COMMAND_CACHE_FILE: &str = "brew-commands-v1.txt";
const ACTION_MATCH_LIMIT: usize = 5;
const BATCH_MATCH_LIMIT: usize = 6;
const BATCH_FUZZY_MATCH_LIMIT: usize = 8;
const KNOWN_BREW_COMMANDS: &[&str] = &[
    "--cache",
    "--caskroom",
    "--cellar",
    "--env",
    "--prefix",
    "--repository",
    "--taps",
    "--version",
    "alias",
    "analytics",
    "audit",
    "autoremove",
    "bottle",
    "bump",
    "bump-cask-pr",
    "bump-formula-pr",
    "bump-revision",
    "bump-unversioned-casks",
    "bundle",
    "casks",
    "cat",
    "cleanup",
    "command",
    "command-not-found-init",
    "commands",
    "completions",
    "config",
    "contributions",
    "create",
    "debugger",
    "deps",
    "desc",
    "determine-test-runners",
    "developer",
    "dispatch-build-bottle",
    "docs",
    "doctor",
    "edit",
    "extract",
    "fetch",
    "formula",
    "formula-analytics",
    "formulae",
    "generate-analytics-api",
    "generate-cask-api",
    "generate-cask-ci-matrix",
    "generate-formula-api",
    "generate-man-completions",
    "generate-zap",
    "gist-logs",
    "help",
    "home",
    "info",
    "install",
    "install-bundler-gems",
    "irb",
    "leaves",
    "lgtm",
    "link",
    "linkage",
    "list",
    "livecheck",
    "log",
    "mcp-server",
    "migrate",
    "missing",
    "nodenv-sync",
    "options",
    "outdated",
    "pin",
    "postinstall",
    "pr-automerge",
    "pr-publish",
    "pr-pull",
    "pr-upload",
    "prof",
    "pyenv-sync",
    "rbenv-sync",
    "readall",
    "reinstall",
    "release",
    "rubocop",
    "ruby",
    "rubydoc",
    "search",
    "services",
    "setup-ruby",
    "sh",
    "shellenv",
    "source",
    "style",
    "tab",
    "tap",
    "tap-info",
    "tap-new",
    "test",
    "test-bot",
    "tests",
    "typecheck",
    "unalias",
    "unbottled",
    "uninstall",
    "unlink",
    "unpack",
    "unpin",
    "untap",
    "update",
    "update-if-needed",
    "update-license-data",
    "update-maintainers",
    "update-perl-resources",
    "update-python-resources",
    "update-report",
    "update-reset",
    "update-sponsors",
    "update-test",
    "upgrade",
    "uses",
    "vendor-gems",
    "vendor-install",
    "verify",
    "version-install",
    "which-formula",
    "which-update",
];

#[derive(Clone, Copy)]
struct MotionSettings {
    animations_enabled: bool,
    finale_enabled: bool,
}

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

#[derive(Clone, Copy)]
enum OutputStream {
    Stdout,
    Stderr,
}

struct OutputChunk {
    stream: OutputStream,
    bytes: Vec<u8>,
}

#[derive(Debug)]
struct BatchResolvedPackage<'a> {
    query: String,
    package: &'a Package,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConfirmedMatchChoice {
    Accept,
    SearchAgain,
    Cancel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BatchReviewChoice {
    Proceed,
    RetryAll,
    RetryOne,
    Cancel,
}

enum MatchSelection {
    Selected(usize),
    Cancelled,
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
    let cli = Cli::parse(std::env::args().skip(1))?;
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
    if is_known_brew_command(first) {
        return Ok(true);
    }

    if first.starts_with('-') {
        return Err(format!("Unknown flag: `{first}`.\n\n{}", Cli::help_text()));
    }

    Ok(false)
}

fn should_run_motion(motion: MotionSettings) -> bool {
    if !motion.animations_enabled {
        return false;
    }

    let is_terminal = io::stdout().is_terminal();
    let no_color = env::var_os("NO_COLOR").is_some();
    let clicolor_disabled = matches!(env::var("CLICOLOR"), Ok(value) if value == "0");
    let dumb_term = matches!(env::var("TERM"), Ok(value) if value == "dumb");

    is_terminal
        && !no_color
        && !clicolor_disabled
        && !dumb_term
        && env::var_os("BRAU_NO_ANIM").is_none()
        && env::var_os("CI").is_none()
}

fn run_with_motion<T, W, A>(enabled: bool, work: W, animate: A) -> Result<T, String>
where
    T: Send,
    W: FnOnce() -> T + Send,
    A: FnOnce(),
{
    if !enabled {
        return Ok(work());
    }

    thread::scope(|scope| {
        let work_handle = scope.spawn(work);
        animate();
        work_handle
            .join()
            .map_err(|_| "A background task stopped unexpectedly.".to_string())
    })
}

fn brew_command_display(args: &[String]) -> String {
    if args.is_empty() {
        "brew".to_string()
    } else {
        format!("brew {}", args.join(" "))
    }
}

fn spawn_output_reader<R>(
    mut reader: R,
    stream: OutputStream,
    sender: mpsc::Sender<OutputChunk>,
) -> thread::JoinHandle<io::Result<()>>
where
    R: Read + Send + 'static,
{
    thread::spawn(move || {
        let mut buffer = [0u8; 8192];

        loop {
            let read = reader.read(&mut buffer)?;
            if read == 0 {
                return Ok(());
            }

            if sender
                .send(OutputChunk {
                    stream,
                    bytes: buffer[..read].to_vec(),
                })
                .is_err()
            {
                return Ok(());
            }
        }
    })
}

fn relay_output(receiver: mpsc::Receiver<OutputChunk>) -> Result<(), String> {
    while let Ok(chunk) = receiver.recv() {
        match chunk.stream {
            OutputStream::Stdout => {
                let mut stdout = io::stdout();
                stdout
                    .write_all(&chunk.bytes)
                    .map_err(|error| format!("Failed to forward Homebrew stdout: {error}"))?;
                stdout
                    .flush()
                    .map_err(|error| format!("Failed to flush Homebrew stdout: {error}"))?;
            }
            OutputStream::Stderr => {
                let mut stderr = io::stderr();
                stderr
                    .write_all(&chunk.bytes)
                    .map_err(|error| format!("Failed to forward Homebrew stderr: {error}"))?;
                stderr
                    .flush()
                    .map_err(|error| format!("Failed to flush Homebrew stderr: {error}"))?;
            }
        }
    }

    Ok(())
}

fn join_output_reader(
    handle: thread::JoinHandle<io::Result<()>>,
    label: &str,
) -> Result<(), String> {
    handle
        .join()
        .map_err(|_| format!("The Homebrew {label} reader stopped unexpectedly."))?
        .map_err(|error| format!("Failed to read Homebrew {label}: {error}"))
}

fn run_brew_command<A>(
    args: &[String],
    motion: MotionSettings,
    animate: A,
) -> Result<ExitStatus, String>
where
    A: FnOnce(),
{
    if !should_run_motion(motion) {
        return Command::new("brew")
            .args(args)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .map_err(|error| format!("Failed to launch Homebrew: {error}"));
    }

    let mut child = Command::new("brew")
        .args(args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("Failed to launch Homebrew: {error}"))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Failed to capture Homebrew stdout.".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "Failed to capture Homebrew stderr.".to_string())?;

    let (sender, receiver) = mpsc::channel();
    let stdout_reader = spawn_output_reader(stdout, OutputStream::Stdout, sender.clone());
    let stderr_reader = spawn_output_reader(stderr, OutputStream::Stderr, sender);

    animate();
    relay_output(receiver)?;
    let status = child
        .wait()
        .map_err(|error| format!("Failed to wait for Homebrew: {error}"))?;

    join_output_reader(stdout_reader, "stdout")?;
    join_output_reader(stderr_reader, "stderr")?;

    Ok(status)
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
            "No Homebrew packages matched \"{query}\". Try `brau refresh` or a broader search."
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

                match prompt_match_selection_choice(&fuzzy_matches)? {
                    MatchSelection::Selected(index) => Ok(Some(fuzzy_matches[index].package)),
                    MatchSelection::Cancelled => Ok(None),
                }
            }
            ConfirmedMatchChoice::Cancel => Ok(None),
        };
    }

    render::print_action_candidates(query, action.label(), &matches, action.candidates_footer());

    match prompt_match_selection_choice(&matches)? {
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

fn refresh_brew_command_cache() -> Result<(), String> {
    let output = Command::new("brew")
        .args(["commands", "--quiet"])
        .output()
        .map_err(|error| format!("Failed to ask Homebrew for its command list: {error}"))?;

    if !output.status.success() {
        return Err(format!(
            "Homebrew failed while listing commands with status {}.",
            output.status
        ));
    }

    let commands = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n");

    let cache_path = brew_command_cache_path()?;
    if let Some(parent) = cache_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "Failed to create command cache directory {}: {error}",
                parent.display()
            )
        })?;
    }

    fs::write(&cache_path, format!("{commands}\n")).map_err(|error| {
        format!(
            "Failed to write brew command cache {}: {error}",
            cache_path.display()
        )
    })?;

    Ok(())
}

fn is_known_brew_command(command: &str) -> bool {
    KNOWN_BREW_COMMANDS.contains(&command) || cached_brew_commands().contains(command)
}

fn cached_brew_commands() -> HashSet<String> {
    let cache_path = match brew_command_cache_path() {
        Ok(path) => path,
        Err(_) => return HashSet::new(),
    };

    let contents = match fs::read_to_string(&cache_path) {
        Ok(contents) => contents,
        Err(_) => return HashSet::new(),
    };

    contents
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn brew_command_cache_path() -> Result<PathBuf, String> {
    Ok(catalog::cache_dir()?.join(BREW_COMMAND_CACHE_FILE))
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

fn parse_confirmed_match_choice(input: &str) -> Option<ConfirmedMatchChoice> {
    match input.trim().to_ascii_lowercase().as_str() {
        "" | "y" | "yes" => Some(ConfirmedMatchChoice::Accept),
        "n" | "no" => Some(ConfirmedMatchChoice::SearchAgain),
        "q" | "quit" | "cancel" => Some(ConfirmedMatchChoice::Cancel),
        _ => None,
    }
}

fn prompt_confirmed_match_choice() -> Result<ConfirmedMatchChoice, String> {
    loop {
        print!("Keep this package? [Y/n/q] ");
        io::stdout()
            .flush()
            .map_err(|error| format!("Failed to flush stdout: {error}"))?;

        let mut answer = String::new();
        io::stdin()
            .read_line(&mut answer)
            .map_err(|error| format!("Failed to read your answer: {error}"))?;

        if let Some(choice) = parse_confirmed_match_choice(&answer) {
            return Ok(choice);
        }

        println!("Please answer with `y`, `n`, or `q`.");
    }
}

fn parse_batch_review_choice(input: &str) -> Option<BatchReviewChoice> {
    match input.trim().to_ascii_lowercase().as_str() {
        "" | "1" => Some(BatchReviewChoice::Proceed),
        "2" => Some(BatchReviewChoice::RetryAll),
        "3" => Some(BatchReviewChoice::RetryOne),
        "4" | "q" | "quit" | "cancel" => Some(BatchReviewChoice::Cancel),
        _ => None,
    }
}

fn prompt_batch_review_choice() -> Result<BatchReviewChoice, String> {
    loop {
        print!("Choose an option [1-4]: ");
        io::stdout()
            .flush()
            .map_err(|error| format!("Failed to flush stdout: {error}"))?;

        let mut answer = String::new();
        io::stdin()
            .read_line(&mut answer)
            .map_err(|error| format!("Failed to read your answer: {error}"))?;

        if let Some(choice) = parse_batch_review_choice(&answer) {
            return Ok(choice);
        }

        println!("Please enter `1`, `2`, `3`, or `4`.");
    }
}

fn prompt_batch_retry_selection(total: usize) -> Result<Option<usize>, String> {
    loop {
        print!("Pick a package to search again [1-{total} or q]: ");
        io::stdout()
            .flush()
            .map_err(|error| format!("Failed to flush stdout: {error}"))?;

        let mut answer = String::new();
        io::stdin()
            .read_line(&mut answer)
            .map_err(|error| format!("Failed to read your answer: {error}"))?;

        let trimmed = answer.trim();
        if trimmed.eq_ignore_ascii_case("q") {
            return Ok(None);
        }

        if let Ok(index) = trimmed.parse::<usize>() {
            if (1..=total).contains(&index) {
                return Ok(Some(index - 1));
            }
        }

        println!("Please enter a number between 1 and {total}, or `q`.");
    }
}

fn prompt_match_selection<'a>(
    matches: &'a [SearchMatch<'a>],
) -> Result<&'a SearchMatch<'a>, String> {
    match prompt_match_selection_choice(matches)? {
        MatchSelection::Selected(index) => Ok(&matches[index]),
        MatchSelection::Cancelled => Err("Action cancelled.".to_string()),
    }
}

fn prompt_match_selection_choice(matches: &[SearchMatch<'_>]) -> Result<MatchSelection, String> {
    loop {
        print!("Choose a package [1-{} or q]: ", matches.len());
        io::stdout()
            .flush()
            .map_err(|error| format!("Failed to flush stdout: {error}"))?;

        let mut answer = String::new();
        io::stdin()
            .read_line(&mut answer)
            .map_err(|error| format!("Failed to read your answer: {error}"))?;

        let trimmed = answer.trim();
        if trimmed.eq_ignore_ascii_case("q") {
            return Ok(MatchSelection::Cancelled);
        }

        if let Ok(index) = trimmed.parse::<usize>() {
            if let Some(zero_based) = index.checked_sub(1) {
                if matches.get(zero_based).is_some() {
                    return Ok(MatchSelection::Selected(zero_based));
                }
            }
        }

        println!(
            "Please enter a number between 1 and {} or `q`.",
            matches.len()
        );
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

    #[test]
    fn parse_confirmed_match_choice_supports_retry_and_cancel() {
        assert_eq!(
            parse_confirmed_match_choice(""),
            Some(ConfirmedMatchChoice::Accept)
        );
        assert_eq!(
            parse_confirmed_match_choice("n"),
            Some(ConfirmedMatchChoice::SearchAgain)
        );
        assert_eq!(
            parse_confirmed_match_choice("q"),
            Some(ConfirmedMatchChoice::Cancel)
        );
    }

    #[test]
    fn parse_batch_review_choice_supports_all_menu_options() {
        assert_eq!(
            parse_batch_review_choice("1"),
            Some(BatchReviewChoice::Proceed)
        );
        assert_eq!(
            parse_batch_review_choice("2"),
            Some(BatchReviewChoice::RetryAll)
        );
        assert_eq!(
            parse_batch_review_choice("3"),
            Some(BatchReviewChoice::RetryOne)
        );
        assert_eq!(
            parse_batch_review_choice("q"),
            Some(BatchReviewChoice::Cancel)
        );
    }
}
