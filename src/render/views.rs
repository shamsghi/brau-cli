use crate::app;
use crate::catalog::Package;
use crate::search::SearchMatch;

use super::{
    brew_command_mood, brew_invocation, candidate_action_label, style, BRO_ALIAS_ART,
    BRO_ALIAS_PALETTE, BRO_SHADES_ART, BRO_SHADES_PALETTE,
};

pub(super) fn print_batch_query_progress(action: &str, query: &str, index: usize, total: usize) {
    let app_name = app::display_name();

    println!(
        "{}",
        style().frame_title_for(action, "🧭", &format!("Resolve match {index} of {total}"))
    );
    println!("{} {}", style().meta_label("query"), style().token(query));
    println!(
        "{}",
        style().dim(&format!(
            "Lock this package in before {app_name} moves on to the next {action} query."
        ))
    );
    println!();
}

pub(super) fn print_batch_review(action: &str, packages: &[(&str, &Package)]) {
    let s = style();

    println!(
        "{}",
        s.frame_title_for(
            action,
            "🧺",
            &format!(
                "Ready to {} {} package{}",
                action,
                packages.len(),
                if packages.len() == 1 { "" } else { "s" }
            )
        )
    );

    println!("{}", s.winner_rule());

    for (i, (query, package)) in packages.iter().enumerate() {
        let mut title = format!("{} {}", s.list_index(i + 1), format_title(package, false));
        if let Some(version) = package.version.as_deref() {
            title.push_str("  ");
            title.push_str(&s.version(version));
        }
        if package.installed {
            title.push_str("  ");
            title.push_str(&s.status_chip("installed"));
        }

        println!("{} {}", s.winner_pipe(), title);
        println!(
            "{}    {} {}",
            s.winner_pipe(),
            s.meta_label("query"),
            s.token(query)
        );
        println!("{}    {}", s.winner_pipe(), s.body(&package.desc));

        if i + 1 < packages.len() {
            println!("{}", s.winner_pipe());
        }
    }

    println!("{}", s.winner_rule());
    println!();
    println!(
        "{}",
        s.frame_footer_for(
            action,
            &format!(
                "1 {} all  ·  2 search all again  ·  3 search one again  ·  4 cancel",
                action
            )
        )
    );
    println!();
}

pub(super) fn print_help_screen() {
    let app_name = app::display_name();

    println!("{}", style().frame_title_for("help", "🍺", &app_name));
    println!(
        "{}",
        style().body(
            "Fuzzy Homebrew search, richer package details, and a playful wrapper around everyday brew commands. When you ask for brew-only help or flags, it forwards the real command instead of faking it."
        )
    );
    println!();

    println!(
        "{}",
        style().frame_section_for("search", "🔎", "Quick start")
    );
    println!(
        "  {} {}",
        style().token(&format!("{app_name} ripgrap")),
        style().dim("search by default")
    );
    println!(
        "  {} {}",
        style().token(&format!("{app_name} install rg")),
        style().dim("fuzzy install from inside the CLI")
    );
    println!(
        "  {} {}",
        style().token(&format!("{app_name} uninstall ripgrep")),
        style().dim("fuzzy uninstall with the same shortlist flow")
    );
    println!(
        "  {} {}",
        style().token(&format!("{app_name} update")),
        style().dim("pass a bare Homebrew command through with extra formatting")
    );
    println!(
        "  {} {}",
        style().token(&format!("{app_name} brew doctor")),
        style().dim("explicit passthrough for any brew subcommand or flag")
    );
    println!(
        "  {} {}",
        style().token(&format!("{app_name} help search")),
        style().dim("show Homebrew's own docs for a command")
    );
    println!();

    println!(
        "{}",
        style().frame_section_for("brew-info", "🧭", "Commands")
    );
    println!(
        "  {}",
        style().token(&format!("{app_name} [OPTIONS] <query...>"))
    );
    println!(
        "  {}",
        style().token(&format!("{app_name} search [OPTIONS] <query...>"))
    );
    println!(
        "  {}",
        style().token(&format!("{app_name} info [OPTIONS] <query...>"))
    );
    println!(
        "  {}",
        style().token(&format!("{app_name} install [OPTIONS] <query...>"))
    );
    println!(
        "  {}",
        style().token(&format!("{app_name} uninstall [OPTIONS] <query...>"))
    );
    println!(
        "  {}",
        style().token(&format!("{app_name} brew <brew-command...>"))
    );
    println!("  {}", style().token(&format!("{app_name} refresh")));
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
        style().token("-l, --limit <count>"),
        style().dim("change the number of search matches")
    );
    println!(
        "  {} {}",
        style().token("-y, --yes"),
        style().dim("skip confirmation for install and uninstall")
    );
    println!(
        "  {} {}",
        style().token("-n, --dry-run"),
        style().dim("print install or uninstall commands without running them")
    );
    println!();

    println!(
        "{}",
        style().frame_footer_for("help", &format!("{app_name} help"))
    );
    println!();
}

pub(super) fn print_bro_alias_unlock(path: &str, already_available: bool) {
    let s = style();
    let (title, subtitle, stamp, status, footer) = if already_available {
        (
            "Born With The Bone Structure. Never Needed The Arc.",
            "`bro` didn't mew its way here. It was born with a positive canthal tilt, forward skull growth, and a name that already slapped. It mogged `brau` out of existence without breaking a sweat.",
            "HARDMAXXED FROM BIRTH. ARC SKIPPED.",
            "Nothing to do. `bro` was already mogging the entire filesystem.",
            "hardmaxxed from birth, no arc required",
        )
    } else {
        (
            "Looksmaxxed Into A New Jawline.",
            "`brau` mewed for three months, fixed its canthal tilt, and legally changed its name to `bro`. The jawline did the rest. You wouldn't get it.",
            "GLOW-UP: VERIFIED. MEWING: VINDICATED.",
            "Alias forged. `bro` has hunter eyes and zero interest in your opinion.",
            "same binary, different bone structure",
        )
    };

    println!("{}", s.frame_title_for("bro-unlock", "😎", title));
    println!("{}", s.body(subtitle));
    println!();
    println!("{}", s.catalog_frame_line(stamp, "1;38;5;221", '=', 9));
    println!();

    for (index, line) in BRO_ALIAS_ART.iter().enumerate() {
        println!(
            "{}",
            s.paint_finale_gradient(line, &BRO_ALIAS_PALETTE, index)
        );
    }

    println!();
    for (index, line) in BRO_SHADES_ART.iter().enumerate() {
        println!(
            "{}",
            s.paint_finale_gradient(line, &BRO_SHADES_PALETTE, index + 2)
        );
    }
    println!();
    println!(
        "{}",
        s.catalog_frame_line(
            "CANTHAL TILT: POSITIVE. JAWLINE: IMMACULATE. SYMLINK STATUS: IRRELEVANT.",
            "38;5;183",
            '-',
            13
        )
    );
    println!();
    println!(
        "{}",
        s.frame_section_for("bro-unlock", "🪪", "Witness Card")
    );
    println!("{} {}", s.meta_label("name"), s.token("bro"));
    println!("{} {}", s.meta_label("formerly"), s.body("brau"));
    println!(
        "{} {}",
        s.meta_label("cover story"),
        s.accent_text(
            "bro-unlock",
            "genetically distinct executable, your honor, look at the bone structure"
        )
    );
    println!("{} {}", s.meta_label("where"), s.body(path));
    println!(
        "{} {}",
        s.meta_label("swagger"),
        s.accent_text(
            "bro-unlock",
            "identical internals, hunter eyes, mogging other CLIs without trying"
        )
    );
    println!(
        "{} {}",
        s.meta_label("covers"),
        s.body("search, install, uninstall, refresh — and any brew command it doesn't recognise, forwarded without judgment")
    );
    println!("{} {}", s.meta_label("first mew"), s.token("bro update"));
    println!(
        "{} {}",
        s.meta_label("show-off"),
        s.token("bro install chrome -y")
    );
    println!(
        "{} {}",
        s.meta_label("looksmax tier"),
        s.token("bro info ripgrep")
    );
    println!();
    println!("{}", s.status_frame("bro-unlock", "✓", status));
    println!("{}", s.frame_footer_for("bro-unlock", footer));
    println!();
}

pub(super) fn print_brew_command_banner(command: &str, args: &[String]) {
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
        style().frame_footer_for(mood.label, "keeping pace with Homebrew")
    );
    println!();
}

pub(super) fn print_brew_command_footer(command: &str, success: bool) {
    let app_name = app::display_name();
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
    println!(
        "{}",
        style().frame_footer_for(mood.label, &format!("back to {}", app_name))
    );
    println!();
}

pub(super) fn print_search_results(query: &str, matches: &[SearchMatch<'_>]) {
    let app_name = app::display_name();

    if matches.is_empty() {
        println!(
            "{}",
            style().frame_title_for("search", "🫥", &format!("No matches for \"{query}\""))
        );
        println!(
            "{}",
            style().dim(&format!("Try a broader query or run `{app_name} refresh`."))
        );
        println!();
        println!(
            "{}",
            style().frame_footer_for("search", &format!("end of {app_name} results"))
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

pub(super) fn print_action_candidates(
    query: &str,
    action: &str,
    matches: &[SearchMatch<'_>],
    footer: &str,
) {
    print_candidate_panel(
        action,
        "⚠️",
        &format!("Multiple matches for \"{query}\""),
        &format!("Pick the package you want to {action} from the shortlist below."),
        matches,
        footer,
    );
}

pub(super) fn print_retry_candidates(
    query: &str,
    action: &str,
    matches: &[SearchMatch<'_>],
    footer: &str,
) {
    print_candidate_panel(
        action,
        "🪄",
        &format!("More likely matches for \"{query}\""),
        "The first guess was off, so here are the next most probable fuzzy matches.",
        matches,
        footer,
    );
}

pub(super) fn print_action_preview(
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

pub(super) fn print_subsection(icon: &str, title: &str) {
    println!("{}", style().frame_section_for("brew-info", icon, title));
}

pub(super) fn print_footer(message: &str) {
    println!("{}", style().frame_footer_for("brew-info", message));
    println!();
}

pub(super) fn print_match_list(matches: &[SearchMatch<'_>], start_index: usize) {
    for (offset, candidate) in matches.iter().enumerate() {
        let index = start_index + offset;
        print_package_line(index, candidate);
    }
}

pub(super) fn print_package_detail(package: &Package) {
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

fn print_candidate_panel(
    action: &str,
    icon: &str,
    title: &str,
    subtitle: &str,
    matches: &[SearchMatch<'_>],
    footer: &str,
) {
    println!("{}", style().frame_title_for(action, icon, title));
    println!("{}", style().dim(subtitle));
    println!();
    print_match_list(matches, 1);
    println!();
    println!("{}", style().frame_footer_for(action, footer));
    println!();
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
