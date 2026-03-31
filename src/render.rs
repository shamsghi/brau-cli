use std::env;
use std::io::{self, IsTerminal, Write};
use std::thread;
use std::time::Duration;

use crate::catalog::{Package, PackageKind};
use crate::search::SearchMatch;

const SEARCH_STEPS: [&str; 3] = ["sifting names", "tasting aliases", "pouring shortlist"];
const SEARCH_PRELUDE_STEPS: [&str; 3] = ["sifting names", "tasting aliases", "pouring shortlist"];
const INSTALL_STEPS_FORMULA: [&str; 3] = [
    "warming the cellar",
    "lining up bottles",
    "handing off to brew",
];
const INSTALL_STEPS_CASK: [&str; 3] = [
    "warming the cellar",
    "folding the app bundle",
    "handing off to brew",
];
const INSTALL_STEPS_DRY_RUN: [&str; 3] = [
    "warming the cellar",
    "sketching the install plan",
    "drafting the brew command",
];
const UNINSTALL_STEPS_FORMULA: [&str; 3] = [
    "checking linked files",
    "loosening the cellar grip",
    "handing off to brew",
];
const UNINSTALL_STEPS_CASK: [&str; 3] = [
    "checking app traces",
    "folding the bundle away",
    "handing off to brew",
];
const UNINSTALL_STEPS_DRY_RUN: [&str; 3] = [
    "checking linked files",
    "sketching the removal plan",
    "drafting the brew command",
];
const BREW_GENERIC_STEPS: [&str; 3] = [
    "straightening the counter",
    "lining up the next move",
    "handing off to brew",
];
const BREW_MAINTENANCE_STEPS: [&str; 3] = [
    "polishing the taproom",
    "tidying the cellar shelves",
    "handing off to brew",
];
const BREW_INSPECT_STEPS: [&str; 3] = [
    "reading the bottle labels",
    "sorting the package notes",
    "handing off to brew",
];
const BREW_SERVICE_STEPS: [&str; 3] = [
    "waking the service cart",
    "arranging launch labels",
    "handing off to brew",
];
const BREW_TAP_STEPS: [&str; 3] = [
    "checking the tap handles",
    "arranging the cask room",
    "handing off to brew",
];
const BREW_DEVELOPER_STEPS: [&str; 3] = [
    "clearing the workbench",
    "laying out the tool roll",
    "handing off to brew",
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

#[derive(Clone, Copy)]
struct FinaleFrame {
    top_pattern: &'static str,
    middle_wrap: (&'static str, &'static str),
    bottom_pattern: &'static str,
    whisper: &'static str,
}

const FINALE_FRAMES: [FinaleFrame; 4] = [
    FinaleFrame {
        top_pattern: "~ - ~ - ~ -",
        middle_wrap: ("<<", ">>"),
        bottom_pattern: ". = . = . =",
        whisper: "[freshly brewed]",
    },
    FinaleFrame {
        top_pattern: "= o = o = o",
        middle_wrap: ("[[", "]]"),
        bottom_pattern: "- : - : - :",
        whisper: "(cellar glow)",
    },
    FinaleFrame {
        top_pattern: "~ ^ ~ ^ ~ ^",
        middle_wrap: ("{{", "}}"),
        bottom_pattern: "= + = + = +",
        whisper: "",
    },
    FinaleFrame {
        top_pattern: ". : . : . :",
        middle_wrap: ("((", "))"),
        bottom_pattern: "~ . ~ . ~ .",
        whisper: "",
    },
];

#[derive(Clone, Copy)]
pub enum CatalogWarmupKind {
    FirstRun,
    StaleRefresh,
    ManualRefresh,
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

    let spec = catalog_warmup_spec(kind);
    let step_index = tick % spec.steps.len();
    let footer = format!("{} elapsed · {}", format_elapsed(elapsed), spec.footer_hint);

    if tick > 0 {
        print!("\x1b[4A");
    }

    let lines = [
        style().frame_title_for(spec.label, spec.icon, spec.title),
        style().body(spec.subtitle),
        style().catalog_pour_line(spec.label, spec.steps[step_index], tick),
        style().frame_footer_for(spec.label, &footer),
    ];

    let mut stdout = io::stdout();
    for (index, line) in lines.iter().enumerate() {
        print!("\r\x1b[2K{line}");
        if index + 1 < lines.len() {
            print!("\n");
        }
    }

    let _ = stdout.flush();
}

pub fn finish_catalog_warmup(enabled: bool) {
    if enabled && style().should_animate() {
        println!();
        println!();
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
    if !enabled || !style().should_animate() {
        return;
    }

    let mut stdout = io::stdout();

    for (index, frame) in FINALE_FRAMES.iter().enumerate() {
        if index > 0 {
            print!("\x1b[4A");
        }

        for (line_index, line) in finale_lines(*frame, package).iter().enumerate() {
            print!("\r\x1b[2K{line}");
            if line_index + 1 < 4 {
                print!("\n");
            }
        }

        let _ = stdout.flush();
        thread::sleep(Duration::from_millis(118));
    }

    println!();
    println!();
}

pub fn print_help_screen() {
    println!("{}", style().frame_title_for("help", "🍺", "brau"));
    println!(
        "{}",
        style().body(
            "Fuzzy Homebrew search, richer package details, and a playful wrapper around everyday brew commands."
        )
    );
    println!();

    println!(
        "{}",
        style().frame_section_for("search", "🔎", "Quick start")
    );
    println!(
        "  {} {}",
        style().token("brau ripgrap"),
        style().dim("search by default")
    );
    println!(
        "  {} {}",
        style().token("brau install rg"),
        style().dim("fuzzy install from inside the CLI")
    );
    println!(
        "  {} {}",
        style().token("brau uninstall ripgrep"),
        style().dim("fuzzy uninstall with the same shortlist flow")
    );
    println!(
        "  {} {}",
        style().token("brau update"),
        style().dim("pass a bare Homebrew command through with extra formatting")
    );
    println!(
        "  {} {}",
        style().token("brau brew doctor"),
        style().dim("explicit passthrough for any brew subcommand or flag")
    );
    println!();

    println!(
        "{}",
        style().frame_section_for("brew-info", "🧭", "Commands")
    );
    println!("  {}", style().token("brau [OPTIONS] <query...>"));
    println!("  {}", style().token("brau search [OPTIONS] <query...>"));
    println!("  {}", style().token("brau info [OPTIONS] <query...>"));
    println!("  {}", style().token("brau install [OPTIONS] <query...>"));
    println!("  {}", style().token("brau uninstall [OPTIONS] <query...>"));
    println!("  {}", style().token("brau brew <brew-command...>"));
    println!("  {}", style().token("brau refresh"));
    println!();

    println!(
        "{}",
        style().frame_section_for("brew-generic", "🎛", "Helpful flags")
    );
    println!(
        "  {} {}",
        style().token("--formula"),
        style().dim("search only formulae")
    );
    println!(
        "  {} {}",
        style().token("--cask"),
        style().dim("search only casks")
    );
    println!(
        "  {} {}",
        style().token("--refresh"),
        style().dim("rebuild the local catalog before fuzzy search")
    );
    println!(
        "  {} {}",
        style().token("--no-anim"),
        style().dim("turn off the motion touches")
    );
    println!(
        "  {} {}",
        style().token("--no-finale"),
        style().dim("skip the post-install ASCII finale")
    );
    println!(
        "  {} {}",
        style().token("-n, --limit <count>"),
        style().dim("change the number of search matches")
    );
    println!(
        "  {} {}",
        style().token("-y, --yes"),
        style().dim("skip confirmation for install and uninstall")
    );
    println!(
        "  {} {}",
        style().token("--dry-run"),
        style().dim("print install or uninstall commands without running them")
    );
    println!();

    println!("{}", style().frame_footer_for("help", "brau help"));
    println!();
}

pub fn print_brew_command_banner(command: &str, args: &[String]) {
    let mood = brew_command_mood(command);
    let invocation = brew_invocation(command, args);
    let title = if invocation == "brew" {
        "Homebrew passthrough".to_string()
    } else {
        invocation.clone()
    };

    println!("{}", style().frame_title_for(mood.label, mood.icon, &title));
    println!("{}", style().body(mood.subtitle));
    println!(
        "{} {}",
        style().meta_label("command"),
        style().token(&invocation)
    );
    println!(
        "{} {}",
        style().meta_label("mood"),
        style().accent_text(mood.label, mood.mood_line)
    );
    println!();
    println!(
        "{}",
        style().frame_footer_for(mood.label, "handing off to Homebrew")
    );
    println!();
}

pub fn print_brew_command_footer(command: &str, success: bool) {
    let mood = brew_command_mood(command);
    let message = if success {
        format!("brew {command} finished")
    } else {
        format!("brew {command} ended with a hiccup")
    };

    println!();
    println!(
        "{}",
        style().status_frame(
            if success { "success" } else { "error" },
            if success { "✓" } else { "!" },
            &message
        )
    );
    println!("{}", style().frame_footer_for(mood.label, "back to brau"));
    println!();
}

pub fn print_search_results(query: &str, matches: &[SearchMatch<'_>]) {
    if matches.is_empty() {
        println!(
            "{}",
            style().frame_title_for("search", "🫥", &format!("No matches for \"{query}\""))
        );
        println!(
            "{}",
            style().dim("Try a broader query or run `brau refresh`.")
        );
        println!();
        println!(
            "{}",
            style().frame_footer_for("search", "end of brau results")
        );
        println!();
        return;
    }

    println!(
        "{}",
        style().frame_title_for("search", "🔎", &format!("Best guess for \"{query}\""))
    );
    print_best_guess_card(matches[0].package, &matches[0]);

    if matches.len() > 1 {
        println!();
        println!(
            "{}",
            style().frame_section_for("search", "📋", "Other matches")
        );
        print_match_list(&matches[1..], 2);
    }

    println!();
    println!(
        "{}",
        style().frame_footer_for(
            "search",
            &format!(
                "{} match{} shown",
                matches.len(),
                if matches.len() == 1 { "" } else { "es" }
            )
        )
    );
    println!();
}

pub fn print_action_candidates(
    query: &str,
    action: &str,
    matches: &[SearchMatch<'_>],
    footer: &str,
) {
    println!(
        "{}",
        style().frame_title_for(action, "⚠️", &format!("Multiple matches for \"{query}\""))
    );
    println!(
        "{}",
        style().dim(&format!(
            "Pick the package you want to {action} from the shortlist below."
        ))
    );
    println!();
    print_match_list(matches, 1);
    println!();
    println!("{}", style().frame_footer_for(action, footer));
    println!();
}

pub fn print_action_preview(
    query: &str,
    title: &str,
    package: &Package,
    candidate: &SearchMatch<'_>,
    footer: &str,
) {
    println!(
        "{}",
        style().frame_title_for(
            candidate_action_label(title),
            "✨",
            &format!("{title} for \"{query}\"")
        )
    );
    print_best_guess_card(package, candidate);
    println!();
    println!(
        "{}",
        style().frame_footer_for(candidate_action_label(title), footer)
    );
    println!();
}

pub fn print_subsection(icon: &str, title: &str) {
    println!("{}", style().frame_section_for("brew-info", icon, title));
}

pub fn print_footer(message: &str) {
    println!("{}", style().frame_footer_for("brew-info", message));
    println!();
}

pub fn print_match_list(matches: &[SearchMatch<'_>], start_index: usize) {
    for (offset, candidate) in matches.iter().enumerate() {
        let index = start_index + offset;
        print_package_line(index, candidate);
    }
}

pub fn print_package_detail(package: &Package) {
    println!(
        "{}",
        style().frame_title_for("brew-info", "📦", "Package details")
    );
    println!("{}", format_title(package, true));

    let status = package.short_status();
    if !status.is_empty() {
        print_meta("status", &style_statuses(&status));
    }

    print_meta("description", &package.desc);

    if !package.display_names.is_empty() {
        print_meta("names", &package.display_names.join(", "));
    }
    if !package.aliases.is_empty() {
        print_meta("aliases", &package.aliases.join(", "));
    }
    if !package.old_names.is_empty() {
        print_meta("old names", &package.old_names.join(", "));
    }
    if let Some(version) = package.version.as_deref() {
        print_meta("version", &style().version(version));
    }
    if let Some(homepage) = package.homepage.as_deref() {
        print_meta("homepage", homepage);
    }
    if let Some(tap) = package.tap.as_deref() {
        print_meta("tap", tap);
    }
    if let Some(license) = package.license.as_deref() {
        print_meta("license", license);
    }
    if !package.dependencies.is_empty() {
        print_meta("dependencies", &join_limited(&package.dependencies, 12));
    }
}

fn print_package_line(index: usize, candidate: &SearchMatch<'_>) {
    let package = candidate.package;
    let mut line = format!(
        "{} {}",
        style().list_index(index),
        format_title(package, false)
    );

    if let Some(version) = package.version.as_deref() {
        line.push_str("  ");
        line.push_str(&style().version(version));
    }
    if package.installed {
        line.push_str("  ");
        line.push_str(&style().status_chip("installed"));
    }
    if package.auto_updates {
        line.push_str("  ");
        line.push_str(&style().status_chip("auto-updates"));
    }
    println!("{line}");
    println!("   {}", style().body(&package.desc));

    if !package.display_names.is_empty() {
        println!(
            "   {} {}",
            style().meta_label("names"),
            join_limited(&package.display_names, 3)
        );
    } else if !package.aliases.is_empty() {
        println!(
            "   {} {}",
            style().meta_label("aliases"),
            join_limited(&package.aliases, 3)
        );
    }

    println!(
        "   {} {}",
        style().meta_label("match"),
        style().match_reason(candidate.reason)
    );
}

fn print_best_guess_card(package: &Package, candidate: &SearchMatch<'_>) {
    println!("{}", style().winner_rule());

    for line in build_package_card_lines(package, Some(candidate)) {
        println!("{} {}", style().winner_pipe(), line.trim_start());
    }

    println!("{}", style().winner_rule());
}

fn build_package_card_lines(package: &Package, candidate: Option<&SearchMatch<'_>>) -> Vec<String> {
    let mut lines = Vec::new();
    lines.push(format!("  {}", format_title(package, true)));
    lines.push(format!("    {}", style().body(&package.desc)));

    if !package.display_names.is_empty() {
        lines.push(format_meta("names", &package.display_names.join(", ")));
    }
    if !package.aliases.is_empty() {
        lines.push(format_meta("aliases", &package.aliases.join(", ")));
    }
    if !package.old_names.is_empty() {
        lines.push(format_meta(
            "old names",
            &join_limited(&package.old_names, 4),
        ));
    }
    if let Some(homepage) = package.homepage.as_deref() {
        lines.push(format_meta("homepage", homepage));
    }
    if let Some(tap) = package.tap.as_deref() {
        lines.push(format_meta("tap", tap));
    }
    if let Some(license) = package.license.as_deref() {
        lines.push(format_meta("license", license));
    }
    if !package.dependencies.is_empty() {
        lines.push(format_meta(
            "depends",
            &join_limited(&package.dependencies, 6),
        ));
    }
    if let Some(candidate) = candidate {
        lines.push(format_meta(
            "match",
            &style().match_reason(candidate.reason),
        ));
    }

    lines
}

fn join_limited(values: &[String], limit: usize) -> String {
    if values.len() <= limit {
        return values.join(", ");
    }

    let mut preview = values.iter().take(limit).cloned().collect::<Vec<_>>();
    preview.push(format!("+{} more", values.len() - limit));
    preview.join(", ")
}

fn print_meta(label: &str, value: &str) {
    println!("{}", format_meta(label, value));
}

fn format_meta(label: &str, value: &str) -> String {
    format!("{} {}", style().meta_label(label), value)
}

fn format_title(package: &Package, include_status: bool) -> String {
    let mut parts = Vec::new();
    let icon = style().package_icon(package.kind);
    if !icon.is_empty() {
        parts.push(icon.to_string());
    }
    parts.push(style().token(&package.token));
    parts.push(style().kind_badge(package.kind));

    if include_status {
        let status = package.short_status();
        if !status.is_empty() {
            parts.push(style_statuses(&status));
        }
    }

    parts.join(" ")
}

fn style_statuses(statuses: &[String]) -> String {
    statuses
        .iter()
        .map(|status| {
            if let Some(version) = status.strip_prefix('v') {
                style().version(version)
            } else {
                style().status_chip(status)
            }
        })
        .collect::<Vec<_>>()
        .join(&style().separator())
}

fn style() -> Style {
    Style::detect()
}

fn play_motion_sequence(label: &str, target: &str, steps: &[&str], frame_ms: u64, enabled: bool) {
    if !enabled || !style().should_animate() {
        return;
    }

    let mut stdout = io::stdout();

    for (index, step) in steps.iter().enumerate() {
        let line = style().motion_line(label, target, step, index + 1, steps.len());
        print!("\r\x1b[2K{line}");
        let _ = stdout.flush();
        thread::sleep(Duration::from_millis(frame_ms));
    }

    print!("\r\x1b[2K");
    let _ = stdout.flush();
}

fn finale_lines(frame: FinaleFrame, package: &Package) -> [String; 4] {
    let title = format!("{} freshly brewed {}", frame.top_pattern, frame.top_pattern);
    let middle = format!(
        "{} {} is ready {}",
        frame.middle_wrap.0, package.token, frame.middle_wrap.1
    );
    let bottom = format!(
        "{} pour complete {}",
        frame.bottom_pattern, frame.bottom_pattern
    );
    let whisper = frame.whisper.to_string();

    [
        style().finale_top(&title),
        style().finale_middle(&middle),
        style().finale_bottom(&bottom),
        style().finale_whisper(&whisper),
    ]
}

fn print_catalog_warmup_note(kind: CatalogWarmupKind) {
    let spec = catalog_warmup_spec(kind);
    println!(
        "{}",
        style().frame_title_for(spec.label, spec.icon, spec.title)
    );
    println!("{}", style().body(spec.subtitle));
    println!("{}", style().frame_footer_for(spec.label, spec.footer_hint));
    println!();
}

fn catalog_warmup_spec(kind: CatalogWarmupKind) -> CatalogWarmupSpec {
    match kind {
        CatalogWarmupKind::FirstRun => CatalogWarmupSpec {
            label: "catalog-build",
            icon: "🫙",
            title: "Building local Homebrew catalog",
            subtitle:
                "First run is slower because brau asks Homebrew for every formula and cask once, then keeps a local cache for later.",
            footer_hint: "later runs read the local cache instead",
            steps: &CATALOG_BUILD_STEPS,
        },
        CatalogWarmupKind::StaleRefresh => CatalogWarmupSpec {
            label: "catalog-build",
            icon: "🧭",
            title: "Refreshing local Homebrew catalog",
            subtitle:
                "The saved catalog is stale, so brau is refreshing Homebrew metadata before the next search.",
            footer_hint: "once this settles, searches snap back",
            steps: &CATALOG_REFRESH_STEPS,
        },
        CatalogWarmupKind::ManualRefresh => CatalogWarmupSpec {
            label: "catalog-build",
            icon: "🧹",
            title: "Rebuilding local Homebrew catalog",
            subtitle:
                "You asked brau to rebuild its local Homebrew catalog from scratch, so this run takes the scenic route.",
            footer_hint: "fresh cache, then back to quick searches",
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

#[derive(Clone, Copy)]
struct CatalogWarmupSpec {
    label: &'static str,
    icon: &'static str,
    title: &'static str,
    subtitle: &'static str,
    footer_hint: &'static str,
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

    match token.as_str() {
        "" => BrewCommandMood {
            label: "brew-generic",
            icon: "🍺",
            subtitle: "Sending the next move straight to Homebrew with a little extra ceremony.",
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
            subtitle: "Housekeeping commands get a warmer runway before Homebrew takes over.",
            mood_line: "cellar tidy",
            steps: &BREW_MAINTENANCE_STEPS,
        },
        "info" | "list" | "search" | "desc" | "deps" | "cat" | "config" | "doctor" | "commands"
        | "command" | "formula" | "formulae" | "casks" | "leaves" | "log" | "options"
        | "outdated" | "uses" | "tap-info" | "home" | "--version" | "--prefix" | "--cellar"
        | "--cache" | "--repository" | "--env" | "--caskroom" | "--taps" => BrewCommandMood {
            label: "brew-info",
            icon: "🧭",
            subtitle: "A softer prelude for the commands that inspect, list, and explain.",
            mood_line: "notes and labels",
            steps: &BREW_INSPECT_STEPS,
        },
        "services" => BrewCommandMood {
            label: "brew-services",
            icon: "🪄",
            subtitle:
                "Service commands get a little stage setup before the launch machinery kicks in.",
            mood_line: "service cart rolling",
            steps: &BREW_SERVICE_STEPS,
        },
        "tap" | "untap" | "tap-new" | "extract" => BrewCommandMood {
            label: "brew-tap",
            icon: "🚰",
            subtitle: "Tap commands get a roomier handoff with a splash of extra color.",
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
            subtitle: "Developer-facing brew commands get a slightly more workshop-y entrance.",
            mood_line: "tools laid out",
            steps: &BREW_DEVELOPER_STEPS,
        },
        _ => BrewCommandMood {
            label: "brew-generic",
            icon: "🍺",
            subtitle: "Sending the next move straight to Homebrew with a little extra ceremony.",
            mood_line: "counter reset",
            steps: &BREW_GENERIC_STEPS,
        },
    }
}

#[derive(Clone, Copy)]
struct MotionTheme {
    left: &'static str,
    right: &'static str,
    fill: char,
    empty: char,
    icon: &'static str,
    prefix: &'static str,
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

    fn motion_line(
        &self,
        label: &str,
        target: &str,
        step: &str,
        index: usize,
        total: usize,
    ) -> String {
        let theme = self.motion_theme(label);
        let filled = theme.fill.to_string().repeat(index);
        let empty = theme.empty.to_string().repeat(total.saturating_sub(index));
        let bar = format!("{}{}{}{}", theme.left, filled, empty, theme.right);
        let content = if target.is_empty() {
            format!("{bar} {} {}: {step}", theme.icon, theme.prefix)
        } else {
            format!(
                "{bar} {} {}: {step} for \"{target}\"",
                theme.icon, theme.prefix
            )
        };
        self.motion_color(label, index, &content)
    }

    fn catalog_pour_line(&self, label: &str, step: &str, tick: usize) -> String {
        let palette = self.palette(label);
        let fill = (tick % 8) + 1;
        let stream = match tick % 4 {
            0 => ".",
            1 => ".:",
            2 => ".::",
            _ => ".:::",
        };
        let bubble = match tick % 4 {
            0 => "°",
            1 => "o",
            2 => "°",
            _ => ".",
        };
        let mug = format!(
            "[{}{}]",
            "▓".repeat(fill),
            "░".repeat(8usize.saturating_sub(fill))
        );

        format!(
            "{}{}{}{}{}",
            self.paint(palette.tertiary, &format!("  tap ))> {stream} {bubble}  ")),
            self.paint(palette.secondary, &format!("{mug} ")),
            self.paint(palette.primary, "🍺 "),
            self.paint(palette.secondary, "· "),
            self.paint(palette.primary, step)
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

    fn motion_theme(&self, label: &str) -> MotionTheme {
        match label {
            "catalog-build" => MotionTheme {
                left: "{",
                right: "}",
                fill: '=',
                empty: '.',
                icon: "◍",
                prefix: "catalog",
            },
            "search" => MotionTheme {
                left: "[",
                right: "]",
                fill: '=',
                empty: '.',
                icon: "◌",
                prefix: "search",
            },
            "install" => MotionTheme {
                left: "{",
                right: "}",
                fill: '#',
                empty: '.',
                icon: "+",
                prefix: "install",
            },
            "uninstall" => MotionTheme {
                left: "<",
                right: ">",
                fill: '~',
                empty: '.',
                icon: "-",
                prefix: "uninstall",
            },
            "brew-maintenance" => MotionTheme {
                left: "(",
                right: ")",
                fill: '=',
                empty: ':',
                icon: ">",
                prefix: "brew",
            },
            "brew-info" => MotionTheme {
                left: "[",
                right: "]",
                fill: '-',
                empty: '.',
                icon: ":",
                prefix: "brew",
            },
            "brew-services" => MotionTheme {
                left: "{",
                right: "}",
                fill: '/',
                empty: '.',
                icon: "~",
                prefix: "brew",
            },
            "brew-tap" => MotionTheme {
                left: "<",
                right: ">",
                fill: 'o',
                empty: '.',
                icon: "=",
                prefix: "brew",
            },
            "brew-dev" => MotionTheme {
                left: "[",
                right: "]",
                fill: '^',
                empty: '.',
                icon: "*",
                prefix: "brew",
            },
            _ => MotionTheme {
                left: "[",
                right: "]",
                fill: '*',
                empty: '.',
                icon: ">",
                prefix: "brew",
            },
        }
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

    fn finale_top(&self, value: &str) -> String {
        self.bold_cyan(value)
    }

    fn finale_middle(&self, value: &str) -> String {
        self.paint("1;33", value)
    }

    fn finale_bottom(&self, value: &str) -> String {
        self.magenta(value)
    }

    fn finale_whisper(&self, value: &str) -> String {
        self.bold_green(value)
    }

    fn paint(&self, code: &str, value: &str) -> String {
        if self.enabled {
            format!("\x1b[{code}m{value}\x1b[0m")
        } else {
            value.to_string()
        }
    }
}
