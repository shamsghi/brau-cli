use std::env;
use std::io::{self, IsTerminal, Write};
use std::thread;
use std::time::Duration;

use crate::catalog::{Package, PackageKind};
use crate::search::SearchMatch;

const CHARM_FRAMES: [&str; 6] = [
    "｡･ﾟﾟ･ brewing guesses",
    "°｡⋆ brewing guesses",
    "｡°✧ brewing guesses",
    "⋆｡° brewing guesses",
    "･ﾟ｡ brewing guesses",
    "°･ﾟ brewing guesses",
];

pub fn play_search_charm(query: &str) {
    if !style().should_animate() {
        return;
    }

    let mut stdout = io::stdout();

    for frame in CHARM_FRAMES {
        let line = style().charm_line(frame, query);
        print!("\r\x1b[2K{line}");
        let _ = stdout.flush();
        thread::sleep(Duration::from_millis(42));
    }

    let settle = style().charm_line("☁︎ guess steeped", query);
    print!("\r\x1b[2K{settle}");
    let _ = stdout.flush();
    thread::sleep(Duration::from_millis(64));
    print!("\r\x1b[2K");
    let _ = stdout.flush();
}

pub fn print_search_results(query: &str, matches: &[SearchMatch<'_>]) {
    if matches.is_empty() {
        println!(
            "{}",
            style().frame_title("🫥", &format!("No matches for \"{query}\""))
        );
        println!(
            "{}",
            style().dim("Try a broader query or run `brewfind refresh`.")
        );
        println!();
        println!("{}", style().frame_footer("end of brewfind results"));
        println!();
        return;
    }

    println!(
        "{}",
        style().frame_title("🔎", &format!("Best guess for \"{query}\""))
    );
    print_best_guess_card(matches[0].package, &matches[0]);

    if matches.len() > 1 {
        println!();
        println!("{}", style().frame_section("📋", "Other matches"));
        print_match_list(&matches[1..], 2);
    }

    println!();
    println!(
        "{}",
        style().frame_footer(&format!(
            "{} match{} shown",
            matches.len(),
            if matches.len() == 1 { "" } else { "es" }
        ))
    );
    println!();
}

pub fn print_install_candidates(query: &str, matches: &[SearchMatch<'_>]) {
    println!(
        "{}",
        style().frame_title("⚠️", &format!("Multiple matches for \"{query}\""))
    );
    println!(
        "{}",
        style().dim("Pick the package you want to install from the shortlist below.")
    );
    println!();
    print_match_list(matches, 1);
    println!();
    println!(
        "{}",
        style().frame_footer("choose a number to install, or q to cancel")
    );
    println!();
}

pub fn print_install_preview(query: &str, package: &Package, candidate: &SearchMatch<'_>) {
    println!(
        "{}",
        style().frame_title("✨", &format!("Ready to install for \"{query}\""))
    );
    print_best_guess_card(package, candidate);
    println!();
    println!(
        "{}",
        style().frame_footer("press y to install, or n to cancel")
    );
    println!();
}

pub fn print_subsection(icon: &str, title: &str) {
    println!("{}", style().frame_section(icon, title));
}

pub fn print_footer(message: &str) {
    println!("{}", style().frame_footer(message));
    println!();
}

pub fn print_match_list(matches: &[SearchMatch<'_>], start_index: usize) {
    for (offset, candidate) in matches.iter().enumerate() {
        let index = start_index + offset;
        print_package_line(index, candidate);
    }
}

pub fn print_package_detail(package: &Package) {
    println!("{}", style().frame_title("📦", "Package details"));
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
    println!("{}", style().winner_badge("best match"));
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

#[derive(Clone, Copy)]
struct Style {
    enabled: bool,
    fancy: bool,
}

impl Style {
    fn should_animate(&self) -> bool {
        self.enabled
            && self.fancy
            && env::var_os("BREWFIND_NO_ANIM").is_none()
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

    fn frame_title(&self, icon: &str, title: &str) -> String {
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
        self.bold_cyan(&format!("+{bar} {prefix}{title} {bar}+"))
    }

    fn frame_section(&self, icon: &str, title: &str) -> String {
        let prefix = if self.fancy {
            format!("{icon} ")
        } else {
            String::new()
        };
        let bar = "-".repeat(8);
        self.magenta(&format!("+{bar} {prefix}{title} {bar}+"))
    }

    fn frame_footer(&self, message: &str) -> String {
        let bar = "-".repeat(10);
        self.dim(&format!("+{bar} {message} {bar}+"))
    }

    fn winner_badge(&self, label: &str) -> String {
        if self.fancy {
            self.bold_yellow(&format!(">>> {label} <<<"))
        } else {
            self.bold_yellow(label)
        }
    }

    fn winner_rule(&self) -> String {
        self.bold_green("+======================================+")
    }

    fn winner_pipe(&self) -> String {
        self.bold_green("┃")
    }

    fn charm_line(&self, mood: &str, query: &str) -> String {
        let content = if self.fancy {
            format!("{mood} for \"{query}\" ☁︎")
        } else {
            format!("{mood} for \"{query}\"")
        };
        self.magenta(&content)
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
            PackageKind::Formula => "🍵",
            PackageKind::Cask => "🧰",
        }
    }

    fn version(&self, version: &str) -> String {
        self.cyan(&format!("v{version}"))
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

    fn cyan(&self, value: &str) -> String {
        self.paint("36", value)
    }

    fn bold_cyan(&self, value: &str) -> String {
        self.paint("1;36", value)
    }

    fn blue(&self, value: &str) -> String {
        self.paint("34", value)
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

    fn bold_yellow(&self, value: &str) -> String {
        self.paint("1;33", value)
    }

    fn paint(&self, code: &str, value: &str) -> String {
        if self.enabled {
            format!("\x1b[{code}m{value}\x1b[0m")
        } else {
            value.to_string()
        }
    }
}
