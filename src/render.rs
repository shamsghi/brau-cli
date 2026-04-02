use std::env;
use std::io::{self, IsTerminal, Write};
use std::process;
use std::thread;
use std::time::{Duration, SystemTime};

#[cfg(unix)]
use std::os::unix::io::AsRawFd;

use crate::app;
use crate::catalog::{Package, PackageKind};
use crate::search::SearchMatch;

mod finale;
mod views;

const SEARCH_STEPS: [&str; 3] = ["sifting names", "tasting aliases", "pouring shortlist"];
const SEARCH_PRELUDE_STEPS: [&str; 3] = ["sifting names", "tasting aliases", "pouring shortlist"];
const INSTALL_STEPS_FORMULA: [&str; 3] = [
    "warming the cellar",
    "lining up bottles",
    "keeping pace with brew",
];
const INSTALL_STEPS_CASK: [&str; 3] = [
    "warming the cellar",
    "folding the app bundle",
    "keeping pace with brew",
];
const INSTALL_STEPS_DRY_RUN: [&str; 3] = [
    "warming the cellar",
    "sketching the install plan",
    "drafting the brew command",
];
const UNINSTALL_STEPS_FORMULA: [&str; 3] = [
    "checking linked files",
    "loosening the cellar grip",
    "keeping pace with brew",
];
const UNINSTALL_STEPS_CASK: [&str; 3] = [
    "checking app traces",
    "folding the bundle away",
    "keeping pace with brew",
];
const UNINSTALL_STEPS_DRY_RUN: [&str; 3] = [
    "checking linked files",
    "sketching the removal plan",
    "drafting the brew command",
];
const BREW_GENERIC_STEPS: [&str; 3] = [
    "straightening the counter",
    "lining up the next move",
    "keeping pace with brew",
];
const BREW_MAINTENANCE_STEPS: [&str; 3] = [
    "polishing the taproom",
    "tidying the cellar shelves",
    "keeping pace with brew",
];
const BREW_INSPECT_STEPS: [&str; 3] = [
    "reading the bottle labels",
    "sorting the package notes",
    "keeping pace with brew",
];
const BREW_SERVICE_STEPS: [&str; 3] = [
    "waking the service cart",
    "arranging launch labels",
    "keeping pace with brew",
];
const BREW_TAP_STEPS: [&str; 3] = [
    "checking the tap handles",
    "arranging the cask room",
    "keeping pace with brew",
];
const BREW_DEVELOPER_STEPS: [&str; 3] = [
    "clearing the workbench",
    "laying out the tool roll",
    "keeping pace with brew",
];
const BRO_ALIAS_ART: [&str; 5] = [
    "           ____   ____    ___",
    "          | __ ) |  _ \\  / _ \\",
    "          |  _ \\ | |_) || | | |",
    "          | |_) ||  _ < | |_| |",
    "          |____/ |_| \\_\\ \\___/",
];
const BRO_ALIAS_PALETTE: [&str; 6] = [
    "1;38;5;221",
    "1;38;5;215",
    "1;38;5;183",
    "1;38;5;147",
    "1;38;5;109",
    "1;38;5;150",
];
const BRO_SHADES_ART: [&str; 5] = [
    "        ▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄",
    "       ▐███████████████████████████████▌",
    "        ▀███████████▀   ▀███████████▀",
    "          ▀███████▀       ▀███████▀",
    "            ▀▀▀▀             ▀▀▀▀",
];
const BRO_SHADES_PALETTE: [&str; 6] = [
    "38;5;220", "38;5;214", "38;5;226", "38;5;178", "38;5;136", "38;5;130",
];
const CATALOG_BUILD_STEPS: [&str; 4] = [
    "uncorking the formula shelf",
    "walking the cask room",
    "gathering aliases and notes",
    "pouring a local catalog",
];
const CATALOG_REFRESH_STEPS: [&str; 4] = [
    "dusting off the shelf map",
    "refreshing formula notes",
    "refreshing cask notes",
    "rewriting the local catalog",
];
const CATALOG_WARMUP_MAX_WIDTH: usize = 68;
const CATALOG_WARMUP_MIN_WIDTH: usize = 52;
const HONEY_HUES: [&str; 4] = ["38;5;214", "38;5;220", "38;5;221", "38;5;228"];
const TEAL_HUES: [&str; 2] = ["38;5;73", "38;5;109"];

// ── Broom sweep animation frames ─────────────────────────────────────
const BROOM_FRAMES: [&str; 4] = ["/|\\.", "/|\\.:", "/|\\.:·", "/|\\.:·˚"];
const DUST_TRAIL: [&str; 8] = ["·", "˚", "∘", "✦", "·", "˚", "∘", "."];
const SWEEP_COLORS: [&str; 6] = [
    "38;5;223", "38;5;222", "38;5;221", "38;5;220", "38;5;214", "38;5;228",
];

// ── Charm animation sprites (themed per action) ─────────────────────
const CHARM_SEARCH: [&str; 6] = ["◇", "◈", "◆", "◈", "◇", "◈"];
const CHARM_INSTALL: [&str; 5] = ["◦", "○", "◎", "●", "◉"];
const CHARM_UNINSTALL: [&str; 5] = ["◉", "●", "◎", "○", "◦"];
const CHARM_MAINT: [&str; 4] = ["╲", "│", "╱", "─"];
const CHARM_INFO: [&str; 4] = ["◇", "◈", "◆", "◈"];
const CHARM_SERVICE: [&str; 4] = ["✧", "✦", "✧", "✦"];
const CHARM_TAP: [&str; 4] = ["◁", "▷", "◁", "▷"];
const CHARM_DEV: [&str; 4] = ["⊕", "⊗", "⊕", "⊗"];
const CHARM_GENERIC: [&str; 4] = ["·", ":", "·", ":"];

#[derive(Clone, Copy)]
pub enum CatalogWarmupKind {
    FirstRun,
    StaleRefresh,
    ManualRefresh,
}

// ── Finale color themes ──────────────────────────────────────────────
// Each theme is a cohesive palette. One is chosen at random per run.
const FINALE_CONFETTI: [char; 8] = ['✦', '✧', '·', '˚', '*', '⊹', '✶', '∘'];

#[derive(Clone, Copy)]
struct FinaleTheme {
    /// Liquid fill color (4 shades)
    amber: [&'static str; 4],
    /// Header / cheer accent (3 bold shades)
    gold: [&'static str; 3],
    /// Foam / bubble highlights (3 shades)
    foam: [&'static str; 3],
    /// Mug walls + structural lines (3 shades)
    teal: [&'static str; 3],
    /// Banner / success accent (3 bold shades)
    green: [&'static str; 3],
    /// Sparkle / confetti particles (6 bold shades)
    sparkle: [&'static str; 6],
}

const FINALE_THEMES: [FinaleTheme; 8] = [
    // 0 · Amber Ale  — the classic warm golden brew
    FinaleTheme {
        amber: ["38;5;214", "38;5;220", "38;5;221", "38;5;178"],
        gold: ["1;38;5;220", "1;38;5;226", "1;38;5;228"],
        foam: ["38;5;255", "38;5;230", "38;5;223"],
        teal: ["38;5;73", "38;5;109", "38;5;116"],
        green: ["1;38;5;114", "1;38;5;150", "1;38;5;156"],
        sparkle: [
            "1;38;5;214",
            "1;38;5;177",
            "1;38;5;114",
            "1;38;5;220",
            "1;38;5;204",
            "1;38;5;159",
        ],
    },
    // 1 · Tropical IPA  — mango, lime, papaya
    FinaleTheme {
        amber: ["38;5;215", "38;5;221", "38;5;222", "38;5;179"],
        gold: ["1;38;5;228", "1;38;5;222", "1;38;5;186"],
        foam: ["38;5;229", "38;5;223", "38;5;187"],
        teal: ["38;5;114", "38;5;150", "38;5;79"],
        green: ["1;38;5;156", "1;38;5;192", "1;38;5;228"],
        sparkle: [
            "1;38;5;215",
            "1;38;5;222",
            "1;38;5;156",
            "1;38;5;228",
            "1;38;5;114",
            "1;38;5;186",
        ],
    },
    // 2 · Berry Sour  — raspberry, plum, violet
    FinaleTheme {
        amber: ["38;5;168", "38;5;204", "38;5;211", "38;5;175"],
        gold: ["1;38;5;212", "1;38;5;218", "1;38;5;225"],
        foam: ["38;5;225", "38;5;218", "38;5;182"],
        teal: ["38;5;133", "38;5;140", "38;5;177"],
        green: ["1;38;5;183", "1;38;5;219", "1;38;5;225"],
        sparkle: [
            "1;38;5;204",
            "1;38;5;177",
            "1;38;5;219",
            "1;38;5;212",
            "1;38;5;183",
            "1;38;5;225",
        ],
    },
    // 3 · Ocean Stout  — deep teal, seafoam, moonlight
    FinaleTheme {
        amber: ["38;5;24", "38;5;30", "38;5;31", "38;5;37"],
        gold: ["1;38;5;117", "1;38;5;123", "1;38;5;159"],
        foam: ["38;5;159", "38;5;153", "38;5;195"],
        teal: ["38;5;30", "38;5;37", "38;5;73"],
        green: ["1;38;5;79", "1;38;5;115", "1;38;5;158"],
        sparkle: [
            "1;38;5;117",
            "1;38;5;159",
            "1;38;5;79",
            "1;38;5;123",
            "1;38;5;153",
            "1;38;5;195",
        ],
    },
    // 4 · Aurora Lager  — arctic green, violet, electric blue
    FinaleTheme {
        amber: ["38;5;48", "38;5;84", "38;5;85", "38;5;49"],
        gold: ["1;38;5;121", "1;38;5;158", "1;38;5;194"],
        foam: ["38;5;194", "38;5;158", "38;5;195"],
        teal: ["38;5;63", "38;5;99", "38;5;141"],
        green: ["1;38;5;84", "1;38;5;121", "1;38;5;157"],
        sparkle: [
            "1;38;5;48",
            "1;38;5;141",
            "1;38;5;84",
            "1;38;5;177",
            "1;38;5;121",
            "1;38;5;159",
        ],
    },
    // 5 · Cherry Blossom  — sakura pink, peach, soft rose
    FinaleTheme {
        amber: ["38;5;217", "38;5;218", "38;5;224", "38;5;181"],
        gold: ["1;38;5;218", "1;38;5;224", "1;38;5;230"],
        foam: ["38;5;230", "38;5;224", "38;5;188"],
        teal: ["38;5;138", "38;5;175", "38;5;181"],
        green: ["1;38;5;218", "1;38;5;224", "1;38;5;223"],
        sparkle: [
            "1;38;5;217",
            "1;38;5;224",
            "1;38;5;218",
            "1;38;5;181",
            "1;38;5;175",
            "1;38;5;230",
        ],
    },
    // 6 · Neon Haze  — electric cyan, hot magenta, lime
    FinaleTheme {
        amber: ["38;5;45", "38;5;51", "38;5;87", "38;5;123"],
        gold: ["1;38;5;201", "1;38;5;207", "1;38;5;213"],
        foam: ["38;5;195", "38;5;189", "38;5;159"],
        teal: ["38;5;201", "38;5;165", "38;5;129"],
        green: ["1;38;5;118", "1;38;5;154", "1;38;5;190"],
        sparkle: [
            "1;38;5;51",
            "1;38;5;201",
            "1;38;5;118",
            "1;38;5;213",
            "1;38;5;87",
            "1;38;5;190",
        ],
    },
    // 7 · Copper Porter  — burnished copper, mahogany, warm cream
    FinaleTheme {
        amber: ["38;5;130", "38;5;166", "38;5;172", "38;5;136"],
        gold: ["1;38;5;173", "1;38;5;179", "1;38;5;223"],
        foam: ["38;5;223", "38;5;217", "38;5;181"],
        teal: ["38;5;94", "38;5;130", "38;5;137"],
        green: ["1;38;5;179", "1;38;5;186", "1;38;5;223"],
        sparkle: [
            "1;38;5;172",
            "1;38;5;179",
            "1;38;5;186",
            "1;38;5;173",
            "1;38;5;223",
            "1;38;5;130",
        ],
    },
];

/// Pick a finale theme using runtime entropy so each install feels fresh.
fn pick_finale_theme() -> &'static FinaleTheme {
    let pid = process::id() as usize;
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as usize)
        .unwrap_or(0);
    // Mix PID and nanoseconds for good spread even in rapid succession
    let seed = pid.wrapping_mul(2654435761).wrapping_add(nanos);
    &FINALE_THEMES[seed % FINALE_THEMES.len()]
}

pub fn play_search_charm(query: &str, enabled: bool) {
    play_motion_sequence("search", query, &SEARCH_PRELUDE_STEPS, 24, enabled);
}

pub fn draw_catalog_warmup_tick(
    kind: CatalogWarmupKind,
    tick: usize,
    elapsed: Duration,
    enabled: bool,
) {
    if !enabled || !style().should_animate() {
        if tick == 0 {
            print_catalog_warmup_note(kind);
        }
        return;
    }

    let s = style();
    let spec = catalog_warmup_spec(kind);
    let step_index = tick % spec.steps.len();
    let step = spec.steps[step_index];
    let time = format_elapsed(elapsed);

    // Build the broom sweep animation on a single line
    let line = s.broom_sweep_line(spec.icon, step, &time, tick);

    let mut stdout = io::stdout();
    print!("\r\x1b[2K{line}");
    let _ = stdout.flush();
}

pub fn finish_catalog_warmup(enabled: bool) {
    if enabled && style().should_animate() {
        // Clear the animation line and move to next line
        print!("\r\x1b[2K");
        let _ = io::stdout().flush();
    }
}

pub fn play_brew_action_charm(package: &Package, action: &str, dry_run: bool, enabled: bool) {
    let steps = match (action, dry_run, package.kind) {
        ("install", true, _) => &INSTALL_STEPS_DRY_RUN[..],
        ("install", false, PackageKind::Cask) => &INSTALL_STEPS_CASK[..],
        ("install", false, PackageKind::Formula) => &INSTALL_STEPS_FORMULA[..],
        ("uninstall", true, _) => &UNINSTALL_STEPS_DRY_RUN[..],
        ("uninstall", false, PackageKind::Cask) => &UNINSTALL_STEPS_CASK[..],
        ("uninstall", false, PackageKind::Formula) => &UNINSTALL_STEPS_FORMULA[..],
        _ => &SEARCH_STEPS[..],
    };

    play_motion_sequence(action, package.install_target(), steps, 42, enabled);
}

pub fn play_brew_command_charm(command: &str, args: &[String], enabled: bool) {
    let mood = brew_command_mood(command);
    let target = if args.is_empty() {
        command.to_string()
    } else {
        format!("{command} {}", args.join(" "))
    };

    let label = if target.trim().is_empty() {
        "brew-generic"
    } else {
        mood.label
    };

    play_motion_sequence(label, target.trim(), mood.steps, 44, enabled);
}

pub fn play_install_finale(package: &Package, enabled: bool) {
    finale::play_install_finale(package, enabled);
}

pub fn print_batch_query_progress(action: &str, query: &str, index: usize, total: usize) {
    views::print_batch_query_progress(action, query, index, total);
}

pub fn print_batch_review(action: &str, packages: &[(&str, &Package)]) {
    views::print_batch_review(action, packages);
}

pub fn play_batch_install_finale(tokens: &[&str], enabled: bool) {
    finale::play_batch_install_finale(tokens, enabled);
}

pub fn print_help_screen() {
    views::print_help_screen();
}

pub fn print_bro_alias_unlock(path: &str, already_available: bool) {
    views::print_bro_alias_unlock(path, already_available);
}

pub fn print_brew_command_banner(command: &str, args: &[String]) {
    views::print_brew_command_banner(command, args);
}

pub fn print_brew_command_footer(command: &str, success: bool) {
    views::print_brew_command_footer(command, success);
}

pub fn print_search_results(query: &str, matches: &[SearchMatch<'_>]) {
    views::print_search_results(query, matches);
}

pub fn print_action_candidates(
    query: &str,
    action: &str,
    matches: &[SearchMatch<'_>],
    footer: &str,
) {
    views::print_action_candidates(query, action, matches, footer);
}

pub fn print_retry_candidates(
    query: &str,
    action: &str,
    matches: &[SearchMatch<'_>],
    footer: &str,
) {
    views::print_retry_candidates(query, action, matches, footer);
}

pub fn print_action_preview(
    query: &str,
    title: &str,
    package: &Package,
    candidate: &SearchMatch<'_>,
    footer: &str,
) {
    views::print_action_preview(query, title, package, candidate, footer);
}

pub fn print_subsection(icon: &str, title: &str) {
    views::print_subsection(icon, title);
}

pub fn print_footer(message: &str) {
    views::print_footer(message);
}

pub fn print_match_list(matches: &[SearchMatch<'_>], start_index: usize) {
    views::print_match_list(matches, start_index);
}

pub fn print_package_detail(package: &Package) {
    views::print_package_detail(package);
}

fn style() -> Style {
    Style::detect()
}

fn play_motion_sequence(label: &str, target: &str, steps: &[&str], frame_ms: u64, enabled: bool) {
    if !enabled || !style().should_animate() {
        return;
    }

    let s = style();
    let mut stdout = io::stdout();
    let sub_frames = 5usize;
    let total_frames = steps.len() * sub_frames;
    // Keep motion snappy, but line stays readable
    let ms = frame_ms.max(36);

    for frame in 0..total_frames {
        let step_idx = (frame / sub_frames).min(steps.len() - 1);
        let step = steps[step_idx];
        let line = s.charm_line(label, target, step, frame, total_frames);
        print!("\r\x1b[2K{line}");
        let _ = stdout.flush();
        thread::sleep(Duration::from_millis(ms));
    }

    print!("\r\x1b[2K");
    let _ = stdout.flush();
}

fn charm_sprites(label: &str) -> &'static [&'static str] {
    match label {
        "search" => &CHARM_SEARCH,
        "install" => &CHARM_INSTALL,
        "uninstall" => &CHARM_UNINSTALL,
        "brew-maintenance" => &CHARM_MAINT,
        "brew-info" => &CHARM_INFO,
        "brew-services" => &CHARM_SERVICE,
        "brew-tap" => &CHARM_TAP,
        "brew-dev" => &CHARM_DEV,
        _ => &CHARM_GENERIC,
    }
}

fn print_catalog_warmup_note(kind: CatalogWarmupKind) {
    let spec = catalog_warmup_spec(kind);
    println!(
        "{}",
        style().frame_title_for(spec.label, spec.icon, spec.title)
    );
    println!("{}", style().body(&spec.subtitle));
    println!(
        "{}",
        style().frame_footer_for(spec.label, &spec.footer_hint)
    );
    println!();
}

fn catalog_warmup_spec(kind: CatalogWarmupKind) -> CatalogWarmupSpec {
    let app_name = app::display_name();

    match kind {
        CatalogWarmupKind::FirstRun => CatalogWarmupSpec {
            label: "catalog-build",
            icon: "🫙",
            title: "Building local Homebrew catalog",
            subtitle: format!(
                "First run is slower because {app_name} asks Homebrew for every formula and cask once, then keeps a local cache for later."
            ),
            footer_hint: "later runs use the local cache".to_string(),
            steps: &CATALOG_BUILD_STEPS,
        },
        CatalogWarmupKind::StaleRefresh => CatalogWarmupSpec {
            label: "catalog-build",
            icon: "🧭",
            title: "Refreshing local Homebrew catalog",
            subtitle: format!(
                "The saved catalog is stale, so {app_name} is refreshing Homebrew metadata before the next search."
            ),
            footer_hint: "once this settles, searches snap back".to_string(),
            steps: &CATALOG_REFRESH_STEPS,
        },
        CatalogWarmupKind::ManualRefresh => CatalogWarmupSpec {
            label: "catalog-build",
            icon: "🧹",
            title: "Rebuilding local Homebrew catalog",
            subtitle: format!(
                "You asked {app_name} to rebuild its local Homebrew catalog from scratch, so this run takes the scenic route."
            ),
            footer_hint: "fresh cache, then back to quick searches".to_string(),
            steps: &CATALOG_REFRESH_STEPS,
        },
    }
}

fn format_elapsed(elapsed: Duration) -> String {
    let seconds = elapsed.as_secs();
    if seconds < 60 {
        format!("{seconds}s")
    } else {
        format!("{}m {}s", seconds / 60, seconds % 60)
    }
}

struct CatalogWarmupSpec {
    label: &'static str,
    icon: &'static str,
    title: &'static str,
    subtitle: String,
    footer_hint: String,
    steps: &'static [&'static str],
}

fn brew_invocation(command: &str, args: &[String]) -> String {
    if command.trim().is_empty() {
        return "brew".to_string();
    }

    if args.is_empty() {
        format!("brew {command}")
    } else {
        format!("brew {command} {}", args.join(" "))
    }
}

fn candidate_action_label(title: &str) -> &'static str {
    if title.to_ascii_lowercase().contains("uninstall") {
        "uninstall"
    } else {
        "install"
    }
}

#[derive(Clone, Copy)]
struct BrewCommandMood {
    label: &'static str,
    icon: &'static str,
    subtitle: &'static str,
    mood_line: &'static str,
    steps: &'static [&'static str],
}

fn brew_command_mood(command: &str) -> BrewCommandMood {
    let token = command.trim().to_ascii_lowercase();
    let canonical = match token.as_str() {
        "--config" => "config",
        "--repo" => "--repository",
        "-s" => "search",
        "-v" => "--version",
        "abv" => "info",
        "dr" => "doctor",
        "environment" => "--env",
        "homepage" => "home",
        "instal" => "install",
        "lc" => "livecheck",
        "ln" => "link",
        "ls" => "list",
        "post_install" => "postinstall",
        "remove" | "rm" | "uninstal" => "uninstall",
        "tc" => "typecheck",
        "up" => "update",
        _ => token.as_str(),
    };

    match canonical {
        "" => BrewCommandMood {
            label: "brew-generic",
            icon: "🍺",
            subtitle: "Homebrew gets moving right away, with a little extra ceremony around it.",
            mood_line: "counter reset",
            steps: &BREW_GENERIC_STEPS,
        },
        "update"
        | "upgrade"
        | "cleanup"
        | "autoremove"
        | "fetch"
        | "reinstall"
        | "update-if-needed"
        | "update-reset"
        | "update-test"
        | "update-report"
        | "update-sponsors"
        | "update-maintainers"
        | "update-license-data"
        | "update-perl-resources"
        | "update-python-resources" => BrewCommandMood {
            label: "brew-maintenance",
            icon: "🧹",
            subtitle: "Housekeeping commands get a warmer live status line while Homebrew works.",
            mood_line: "cellar tidy",
            steps: &BREW_MAINTENANCE_STEPS,
        },
        "info" | "list" | "search" | "desc" | "deps" | "cat" | "config" | "doctor" | "commands"
        | "command" | "formula" | "formulae" | "casks" | "help" | "leaves" | "log"
        | "options" | "outdated" | "uses" | "tap-info" | "home" | "--version"
        | "--prefix" | "--cellar" | "--cache" | "--repository" | "--env"
        | "--caskroom" | "--taps" => BrewCommandMood {
            label: "brew-info",
            icon: "🧭",
            subtitle: "A softer live status line while Homebrew inspects, lists, and explains.",
            mood_line: "notes and labels",
            steps: &BREW_INSPECT_STEPS,
        },
        "services" => BrewCommandMood {
            label: "brew-services",
            icon: "🪄",
            subtitle:
                "Service commands get a little stage setup while the launch machinery is already moving.",
            mood_line: "service cart rolling",
            steps: &BREW_SERVICE_STEPS,
        },
        "tap" | "untap" | "tap-new" | "extract" => BrewCommandMood {
            label: "brew-tap",
            icon: "🚰",
            subtitle: "Tap commands get a roomier live status line with a splash of extra color.",
            mood_line: "tap handles aligned",
            steps: &BREW_TAP_STEPS,
        },
        "audit"
        | "bottle"
        | "bump"
        | "bump-cask-pr"
        | "bump-formula-pr"
        | "bump-revision"
        | "bump-unversioned-casks"
        | "create"
        | "edit"
        | "readall"
        | "style"
        | "test"
        | "tests"
        | "typecheck"
        | "rubocop"
        | "developer"
        | "debugger"
        | "ruby"
        | "irb"
        | "sh"
        | "docs" => BrewCommandMood {
            label: "brew-dev",
            icon: "🧰",
            subtitle: "Developer-facing brew commands get a slightly more workshop-y live status line.",
            mood_line: "tools laid out",
            steps: &BREW_DEVELOPER_STEPS,
        },
        _ => BrewCommandMood {
            label: "brew-generic",
            icon: "🍺",
            subtitle: "Homebrew gets moving right away, with a little extra ceremony around it.",
            mood_line: "counter reset",
            steps: &BREW_GENERIC_STEPS,
        },
    }
}

#[derive(Clone, Copy)]
struct AccentPalette {
    primary: &'static str,
    secondary: &'static str,
    tertiary: &'static str,
}

#[derive(Clone, Copy)]
struct Style {
    enabled: bool,
    fancy: bool,
}

impl Style {
    fn should_animate(&self) -> bool {
        self.enabled
            && self.fancy
            && env::var_os("BRAU_NO_ANIM").is_none()
            && env::var_os("CI").is_none()
    }

    fn detect() -> Self {
        let is_terminal = io::stdout().is_terminal();
        let no_color = env::var_os("NO_COLOR").is_some();
        let clicolor_disabled = matches!(env::var("CLICOLOR"), Ok(value) if value == "0");
        let dumb_term = matches!(env::var("TERM"), Ok(value) if value == "dumb");
        let enabled = is_terminal && !no_color && !clicolor_disabled && !dumb_term;

        Self {
            enabled,
            fancy: is_terminal,
        }
    }

    fn frame_title_for(&self, label: &str, icon: &str, title: &str) -> String {
        if label == "catalog-build" {
            let content = if self.fancy {
                format!("{icon} {title}")
            } else {
                title.to_string()
            };
            return self.catalog_frame_line(&content, "1;38;5;223", '=', 3);
        }

        let prefix = if self.fancy {
            format!("{icon} ")
        } else {
            String::new()
        };
        let bar = if self.fancy {
            "=".repeat(12)
        } else {
            "-".repeat(12)
        };
        let palette = self.palette(label);
        self.compose_colored_line(
            (palette.secondary, &format!("+{bar} ")),
            (palette.primary, &format!("{prefix}{title}")),
            (palette.secondary, &format!(" {bar}+")),
        )
    }

    fn frame_section_for(&self, label: &str, icon: &str, title: &str) -> String {
        let prefix = if self.fancy {
            format!("{icon} ")
        } else {
            String::new()
        };
        let bar = "-".repeat(8);
        let palette = self.palette(label);
        self.compose_colored_line(
            (palette.tertiary, &format!("+{bar} ")),
            (palette.secondary, &format!("{prefix}{title}")),
            (palette.tertiary, &format!(" {bar}+")),
        )
    }

    fn frame_footer_for(&self, label: &str, message: &str) -> String {
        if label == "catalog-build" {
            return self.catalog_frame_line(message, "38;5;150", '-', 17);
        }

        let bar = "-".repeat(10);
        let palette = self.palette(label);
        self.compose_colored_line(
            (palette.tertiary, &format!("+{bar} ")),
            (palette.secondary, message),
            (palette.tertiary, &format!(" {bar}+")),
        )
    }

    fn winner_rule(&self) -> String {
        self.bold_green("+======================================+")
    }

    fn winner_pipe(&self) -> String {
        self.bold_green("┃")
    }

    fn charm_line(
        &self,
        label: &str,
        target: &str,
        step: &str,
        frame: usize,
        total: usize,
    ) -> String {
        if !self.enabled {
            return if target.is_empty() {
                format!("[{}/{}] {}", frame + 1, total, step)
            } else {
                format!("[{}/{}] {} · \"{}\"", frame + 1, total, step, target)
            };
        }

        let sprites = charm_sprites(label);
        let sprite = sprites[frame % sprites.len()];
        let palette = self.palette(label);

        // Animated spinning sprite with cycling color
        let sprite_colored = self.motion_color(label, frame, sprite);

        // Progress bar: [▰▰▰▱▱▱▱▱▱▱]
        let bar_w = 10usize;
        let filled = ((frame + 1) * bar_w) / total;
        let empty = bar_w.saturating_sub(filled);
        let bar = format!("[{}{}]", "▰".repeat(filled), "▱".repeat(empty));
        let bar_colored = self.paint(palette.secondary, &bar);

        // Step text
        let step_colored = self.paint(palette.primary, step);

        // Target (truncated for terminal fit)
        let target_part = if target.is_empty() {
            String::new()
        } else {
            let max_t = 24usize;
            let t = if target.chars().count() > max_t {
                let mut s: String = target.chars().take(max_t - 2).collect();
                s.push_str("..");
                s
            } else {
                target.to_string()
            };
            format!(" {}", self.paint(palette.tertiary, &format!("\"{}\"", t)))
        };

        // Shimmer trail: cycling colored particles
        let trail_len = 4 + (frame % 4);
        let mut trail = String::new();
        for i in 0..trail_len {
            let p = DUST_TRAIL[(frame + i) % DUST_TRAIL.len()];
            let c = SWEEP_COLORS[(frame + i) % SWEEP_COLORS.len()];
            trail.push_str(&format!("\x1b[{c}m{p}\x1b[0m"));
        }

        format!(
            "  {} {} · {}{}  {}",
            sprite_colored, bar_colored, step_colored, target_part, trail
        )
    }

    fn meta_label(&self, label: &str) -> String {
        let padded = format!("{label:>13}:");
        self.dim(&padded)
    }

    fn token(&self, token: &str) -> String {
        self.bold(token)
    }

    fn body(&self, value: &str) -> String {
        if self.enabled {
            self.paint("38;5;252", value)
        } else {
            value.to_string()
        }
    }

    fn kind_badge(&self, kind: PackageKind) -> String {
        match kind {
            PackageKind::Formula => self.blue("[formula]"),
            PackageKind::Cask => self.magenta("[cask]"),
        }
    }

    fn package_icon(&self, kind: PackageKind) -> &'static str {
        if !self.fancy {
            return "";
        }

        match kind {
            PackageKind::Formula => "🍹",
            PackageKind::Cask => "🍺",
        }
    }

    fn version(&self, version: &str) -> String {
        self.paint("38;5;181", &format!("v{version}"))
    }

    fn match_reason(&self, reason: &str) -> String {
        self.yellow(reason)
    }

    fn status_chip(&self, status: &str) -> String {
        match status {
            "installed" => self.green(status),
            "outdated" => self.yellow(status),
            "deprecated" => self.red(status),
            "disabled" => self.red(status),
            "auto-updates" => self.magenta(status),
            _ => self.bold(status),
        }
    }

    fn list_index(&self, index: usize) -> String {
        self.bold_cyan(&format!("{index:>2}."))
    }

    fn separator(&self) -> String {
        self.dim(" · ")
    }

    fn bold(&self, value: &str) -> String {
        if self.enabled {
            format!("\x1b[1m{value}\x1b[0m")
        } else {
            value.to_string()
        }
    }

    fn dim(&self, value: &str) -> String {
        if self.enabled {
            format!("\x1b[2m{value}\x1b[0m")
        } else {
            value.to_string()
        }
    }

    fn bold_cyan(&self, value: &str) -> String {
        self.paint("1;38;5;109", value)
    }

    fn blue(&self, value: &str) -> String {
        self.paint("38;5;73", value)
    }

    fn magenta(&self, value: &str) -> String {
        self.paint("35", value)
    }

    fn green(&self, value: &str) -> String {
        self.paint("32", value)
    }

    fn yellow(&self, value: &str) -> String {
        self.paint("33", value)
    }

    fn red(&self, value: &str) -> String {
        self.paint("31", value)
    }

    fn bold_green(&self, value: &str) -> String {
        self.paint("1;32", value)
    }

    fn accent_text(&self, label: &str, value: &str) -> String {
        let palette = self.palette(label);
        self.paint(palette.primary, value)
    }

    fn motion_color(&self, label: &str, index: usize, value: &str) -> String {
        let code = match (label, index % 3) {
            ("catalog-build", 0) => "38;5;109",
            ("catalog-build", 1) => "38;5;150",
            ("catalog-build", _) => "38;5;221",
            ("search", 0) => "38;5;73",
            ("search", 1) => "38;5;109",
            ("search", _) => "38;5;116",
            ("install", 0) => "38;5;150",
            ("install", 1) => "38;5;222",
            ("install", _) => "38;5;114",
            ("uninstall", 0) => "38;5;177",
            ("uninstall", 1) => "38;5;204",
            ("uninstall", _) => "38;5;216",
            ("brew-maintenance", 0) => "38;5;214",
            ("brew-maintenance", 1) => "38;5;221",
            ("brew-maintenance", _) => "38;5;228",
            ("brew-info", 0) => "38;5;73",
            ("brew-info", 1) => "38;5;109",
            ("brew-info", _) => "38;5;116",
            ("brew-services", 0) => "38;5;115",
            ("brew-services", 1) => "38;5;150",
            ("brew-services", _) => "38;5;109",
            ("brew-tap", 0) => "38;5;147",
            ("brew-tap", 1) => "38;5;183",
            ("brew-tap", _) => "38;5;111",
            ("bro-unlock", 0) => "38;5;221",
            ("bro-unlock", 1) => "38;5;215",
            ("bro-unlock", _) => "38;5;183",
            ("brew-generic", 0) => "38;5;215",
            ("brew-generic", 1) => "38;5;221",
            ("brew-generic", _) => "38;5;180",
            ("brew-dev", 0) => "38;5;215",
            ("brew-dev", 1) => "38;5;222",
            ("brew-dev", _) => "38;5;186",
            ("success", 0) => "1;38;5;114",
            ("error", 0) => "1;38;5;204",
            _ => "38;5;186",
        };

        self.paint(code, value)
    }

    fn status_frame(&self, label: &str, icon: &str, title: &str) -> String {
        let bar = if self.fancy {
            "-".repeat(10)
        } else {
            "-".repeat(8)
        };
        let palette = self.palette(label);
        self.compose_colored_line(
            (palette.secondary, &format!("+{bar} ")),
            (palette.primary, &format!("{icon} {title}")),
            (palette.secondary, &format!(" {bar}+")),
        )
    }

    fn palette(&self, label: &str) -> AccentPalette {
        match label {
            "catalog-build" => AccentPalette {
                primary: "1;38;5;221",
                secondary: "38;5;109",
                tertiary: "38;5;150",
            },
            "search" | "help" | "brew-info" => AccentPalette {
                primary: "1;38;5;109",
                secondary: "38;5;109",
                tertiary: "38;5;73",
            },
            "install" | "success" => AccentPalette {
                primary: "1;38;5;150",
                secondary: "38;5;150",
                tertiary: "38;5;114",
            },
            "brew-services" => AccentPalette {
                primary: "1;38;5;115",
                secondary: "38;5;115",
                tertiary: "38;5;79",
            },
            "uninstall" | "error" => AccentPalette {
                primary: "1;38;5;209",
                secondary: "38;5;209",
                tertiary: "38;5;177",
            },
            "brew-maintenance" => AccentPalette {
                primary: "1;38;5;221",
                secondary: "38;5;221",
                tertiary: "38;5;214",
            },
            "brew-generic" => AccentPalette {
                primary: "1;38;5;215",
                secondary: "38;5;215",
                tertiary: "38;5;180",
            },
            "brew-dev" => AccentPalette {
                primary: "1;38;5;216",
                secondary: "38;5;216",
                tertiary: "38;5;180",
            },
            "brew-tap" => AccentPalette {
                primary: "1;38;5;183",
                secondary: "38;5;183",
                tertiary: "38;5;147",
            },
            "bro-unlock" => AccentPalette {
                primary: "1;38;5;221",
                secondary: "38;5;215",
                tertiary: "38;5;183",
            },
            _ => AccentPalette {
                primary: "1;38;5;109",
                secondary: "38;5;109",
                tertiary: "38;5;73",
            },
        }
    }

    fn compose_colored_line(
        &self,
        left: (&str, &str),
        center: (&str, &str),
        right: (&str, &str),
    ) -> String {
        format!(
            "{}{}{}",
            self.paint(left.0, left.1),
            self.paint(center.0, center.1),
            self.paint(right.0, right.1)
        )
    }

    fn paint_finale_gradient(&self, value: &str, palette: &[&str], frame: usize) -> String {
        if !self.enabled {
            return value.to_string();
        }
        let mut out = String::new();
        let mut color_index = frame;
        for ch in value.chars() {
            if ch.is_whitespace() {
                out.push(ch);
            } else {
                let code = palette[color_index % palette.len()];
                out.push_str(&format!("\x1b[{code}m{ch}\x1b[0m"));
                color_index += 1;
            }
        }
        out
    }

    fn paint(&self, code: &str, value: &str) -> String {
        if self.enabled {
            format!("\x1b[{code}m{value}\x1b[0m")
        } else {
            value.to_string()
        }
    }

    fn catalog_frame_line(
        &self,
        content: &str,
        content_code: &str,
        fill_char: char,
        seed: usize,
    ) -> String {
        let width = self.catalog_card_width();
        let inner_width = width.saturating_sub(2);
        let content = self.truncate_plain(content, inner_width.saturating_sub(2));
        let content_width = self.visible_width(&content) + 2;
        let fill_total = inner_width.saturating_sub(content_width);
        let left_fill = fill_total / 2;
        let right_fill = fill_total.saturating_sub(left_fill);
        let left = format!("+{}", fill_char.to_string().repeat(left_fill));
        let right = format!("{}+", fill_char.to_string().repeat(right_fill));

        format!(
            "{}{}{}",
            self.catalog_sunwash(&left, seed),
            self.paint(content_code, &format!(" {content} ")),
            self.catalog_sunwash(&right, seed + 7)
        )
    }

    fn catalog_sunwash(&self, value: &str, seed: usize) -> String {
        if !self.enabled {
            return value.to_string();
        }

        let mut out = String::new();
        for (index, ch) in value.chars().enumerate() {
            if ch.is_whitespace() {
                out.push(ch);
                continue;
            }

            let palette = if (index + seed) % 6 == 0 {
                TEAL_HUES[(index + seed) % TEAL_HUES.len()]
            } else {
                HONEY_HUES[(index + seed) % HONEY_HUES.len()]
            };

            out.push_str(&format!("\x1b[{palette}m{ch}\x1b[0m"));
        }
        out
    }

    fn catalog_card_width(&self) -> usize {
        let columns = env::var("COLUMNS")
            .ok()
            .and_then(|value| value.parse::<usize>().ok());
        match columns {
            Some(columns) => {
                let available = columns.saturating_sub(4);
                if available < CATALOG_WARMUP_MIN_WIDTH {
                    available.max(36)
                } else {
                    available.min(CATALOG_WARMUP_MAX_WIDTH)
                }
            }
            None => CATALOG_WARMUP_MAX_WIDTH,
        }
    }

    fn truncate_plain(&self, value: &str, max_width: usize) -> String {
        if self.visible_width(value) <= max_width {
            return value.to_string();
        }

        if max_width <= 3 {
            return ".".repeat(max_width);
        }

        let mut clipped = String::new();
        for ch in value.chars() {
            if self.visible_width(&clipped) + 1 > max_width.saturating_sub(3) {
                break;
            }
            clipped.push(ch);
        }
        clipped.push_str("...");
        clipped
    }

    fn visible_width(&self, value: &str) -> usize {
        value.chars().count()
    }

    /// Build a single-line broom sweep animation for catalog warmup.
    ///
    /// Layout:  🧭 /|\.:·˚  dusting off the shelf map  ·˚∘·˚∘  3s
    ///
    /// The broom sweeps through dust particles that shimmer with color,
    /// and the whole thing fits on one terminal line.
    fn broom_sweep_line(&self, icon: &str, step: &str, time: &str, tick: usize) -> String {
        if !self.enabled {
            return format!("{icon} {step} ({time})");
        }

        let width = terminal_width();

        // Broom frame cycles through sweep positions
        let broom_frame = BROOM_FRAMES[tick % BROOM_FRAMES.len()];

        // Build dust trail: a shimmering sequence of particles
        let trail_len = 6 + (tick % 4);
        let mut dust = String::new();
        for i in 0..trail_len {
            let particle = DUST_TRAIL[(tick + i) % DUST_TRAIL.len()];
            let color = SWEEP_COLORS[(tick + i) % SWEEP_COLORS.len()];
            dust.push_str(&format!("\x1b[{color}m{particle}\x1b[0m"));
        }

        // Time display
        let time_display = self.paint("38;5;109", &format!(" {time}"));

        // Icon
        let icon_part = if self.fancy {
            format!("{icon} ")
        } else {
            String::new()
        };

        // Colorful broom
        let broom_colored = self.paint_broom(broom_frame, tick);

        // Step text with warm color
        let step_colored = self.paint("1;38;5;223", step);

        // Calculate visible widths
        // icon(2) + broom(~6) + space(1) + step + space(1) + dust(~8) + time(~5)
        let icon_w = if self.fancy { 3 } else { 0 }; // emoji + space
        let broom_w = broom_frame.chars().count();
        let step_w = step.chars().count();
        let time_w = time.chars().count() + 1; // space + time
        let separators_w = 4; // spaces between parts

        let used = icon_w + broom_w + step_w + time_w + separators_w + trail_len;

        if used > width {
            // Compact mode: just icon + step + time
            return format!("{}{}  {}", icon_part, step_colored, time_display);
        }

        format!(
            "{}{}  {}  {}{}",
            icon_part, broom_colored, step_colored, dust, time_display
        )
    }

    /// Paint the broom characters with cycling warm colors.
    fn paint_broom(&self, frame: &str, tick: usize) -> String {
        if !self.enabled {
            return frame.to_string();
        }
        let mut out = String::new();
        let broom_colors = ["1;38;5;180", "1;38;5;222", "1;38;5;214", "1;38;5;215"];
        for (i, ch) in frame.chars().enumerate() {
            let color = broom_colors[(tick + i) % broom_colors.len()];
            out.push_str(&format!("\x1b[{color}m{ch}\x1b[0m"));
        }
        out
    }
}

/// Get the actual terminal width. Uses ioctl on Unix, falls back to
/// COLUMNS env var, then to 80.
fn terminal_width() -> usize {
    #[cfg(unix)]
    {
        #[repr(C)]
        struct Winsize {
            ws_row: u16,
            ws_col: u16,
            ws_xpixel: u16,
            ws_ypixel: u16,
        }

        extern "C" {
            fn ioctl(fd: i32, request: u64, ...) -> i32;
        }

        // TIOCGWINSZ on macOS = 0x40087468
        const TIOCGWINSZ: u64 = 0x40087468;

        let mut ws = Winsize {
            ws_row: 0,
            ws_col: 0,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };

        let fd = io::stdout().as_raw_fd();
        let ret = unsafe { ioctl(fd, TIOCGWINSZ, &mut ws as *mut Winsize) };
        if ret == 0 && ws.ws_col > 0 {
            return ws.ws_col as usize;
        }
    }

    // Fallback: COLUMNS env var, then 80
    env::var("COLUMNS")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(80)
}
