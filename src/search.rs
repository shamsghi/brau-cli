use crate::catalog::{Catalog, Package, PackageKind};
use crate::cli::QueryScope;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MatchStrength {
    Exact,
    Strong,
    Good,
    Fuzzy,
}

#[derive(Debug)]
pub struct SearchMatch<'a> {
    pub package: &'a Package,
    pub score: i32,
    pub strength: MatchStrength,
    pub reason: &'static str,
}

#[derive(Debug, Clone, Copy)]
pub struct SearchOptions {
    pub scope: QueryScope,
    pub limit: usize,
}

pub fn search_catalog<'a>(
    catalog: &'a Catalog,
    query: &str,
    options: SearchOptions,
) -> Vec<SearchMatch<'a>> {
    let prepared_query = PreparedQuery::new(query);
    if prepared_query.normalized.is_empty() {
        return Vec::new();
    }

    let mut matches = catalog
        .items
        .iter()
        .filter(|package| options.scope.includes(package.kind))
        .filter_map(|package| score_package(package, &prepared_query))
        .collect::<Vec<_>>();

    matches.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| left.package.token.len().cmp(&right.package.token.len()))
            .then_with(|| compare_kind(left.package.kind, right.package.kind))
            .then_with(|| left.package.token.cmp(&right.package.token))
    });
    matches.truncate(options.limit);
    matches
}

fn score_package<'a>(package: &'a Package, query: &PreparedQuery) -> Option<SearchMatch<'a>> {
    let best = build_fields(package)
        .into_iter()
        .filter_map(|field| score_field(query, &field))
        .fold(None, pick_best)?;

    Some(SearchMatch {
        package,
        score: apply_package_biases(best.score, package),
        strength: best.strength,
        reason: best.reason,
    })
}

fn build_fields(package: &Package) -> Vec<SearchField> {
    let mut fields = Vec::with_capacity(
        3 + package.aliases.len() + package.display_names.len() + package.old_names.len(),
    );

    push_name_field(
        &mut fields,
        &package.token,
        220,
        "exact package name",
        "package name prefix",
        "package name",
    );

    if !package.full_token.is_empty() && package.full_token != package.token {
        push_name_field(
            &mut fields,
            &package.full_token,
            200,
            "exact full package name",
            "full package name prefix",
            "full package name",
        );
    }

    extend_name_fields(
        &mut fields,
        &package.aliases,
        190,
        "alias",
        "alias prefix",
        "alias",
    );
    extend_name_fields(
        &mut fields,
        &package.display_names,
        185,
        "display name",
        "display name prefix",
        "display name",
    );
    extend_name_fields(
        &mut fields,
        &package.old_names,
        140,
        "legacy name",
        "legacy name prefix",
        "legacy name",
    );

    fields.push(SearchField::description(&package.desc, 70));

    if let Some(homepage) = package.homepage.as_deref() {
        push_name_field(
            &mut fields,
            homepage,
            40,
            "homepage",
            "homepage prefix",
            "homepage",
        );
    }

    fields
}

fn push_name_field(
    fields: &mut Vec<SearchField>,
    raw: &str,
    weight: i32,
    exact_reason: &'static str,
    prefix_reason: &'static str,
    contains_reason: &'static str,
) {
    fields.push(SearchField::name(
        raw,
        weight,
        exact_reason,
        prefix_reason,
        contains_reason,
    ));
}

fn extend_name_fields(
    fields: &mut Vec<SearchField>,
    values: &[String],
    weight: i32,
    exact_reason: &'static str,
    prefix_reason: &'static str,
    contains_reason: &'static str,
) {
    fields.extend(values.iter().map(|value| {
        SearchField::name(value, weight, exact_reason, prefix_reason, contains_reason)
    }));
}

fn score_field(query: &PreparedQuery, field: &SearchField) -> Option<ScoredField> {
    if field.normalized.is_empty() {
        return None;
    }

    if field.normalized == query.normalized {
        return Some(ScoredField {
            score: 1_650 + field.weight,
            strength: MatchStrength::Exact,
            reason: field.exact_reason,
        });
    }

    if !field.acronym.is_empty() && field.acronym == query.normalized {
        return Some(ScoredField {
            score: 1_520 + field.weight,
            strength: MatchStrength::Strong,
            reason: "initialism",
        });
    }

    let mut best = None;

    if field.normalized.starts_with(&query.normalized) {
        best = Some(ScoredField {
            score: 1_430 + field.weight - length_penalty(&field.normalized, &query.normalized),
            strength: MatchStrength::Strong,
            reason: field.prefix_reason,
        });
    }

    if let Some(distance) = field.edit_distance(query) {
        let candidate = ScoredField {
            score: 1_330 + field.weight - (distance as i32 * 90),
            strength: MatchStrength::Strong,
            reason: "typo-tolerant match",
        };
        best = pick_best(best, candidate);
    }

    if field.normalized.contains(&query.normalized) {
        let candidate = ScoredField {
            score: 1_190 + field.weight - length_penalty(&field.normalized, &query.normalized),
            strength: MatchStrength::Good,
            reason: field.contains_reason,
        };
        best = pick_best(best, candidate);
    }

    if let Some(word_score) = score_word_overlap(query, field) {
        let candidate = ScoredField {
            score: 930 + field.weight + word_score,
            strength: MatchStrength::Good,
            reason: field.contains_reason,
        };
        best = pick_best(best, candidate);
    }

    if let Some(subsequence_score) = subsequence_score(&query.normalized, &field.normalized) {
        let candidate = ScoredField {
            score: 720 + field.weight + subsequence_score,
            strength: MatchStrength::Fuzzy,
            reason: "fuzzy match",
        };
        best = pick_best(best, candidate);
    }

    best.filter(|candidate| candidate.score >= 420)
}

fn score_word_overlap(query: &PreparedQuery, field: &SearchField) -> Option<i32> {
    if query.words.is_empty() || field.words.is_empty() {
        return None;
    }

    let mut matched = 0usize;
    let mut score = 0i32;

    for query_word in &query.words {
        if field.words.iter().any(|word| word == query_word) {
            matched += 1;
            score += 90;
        } else if field.words.iter().any(|word| word.starts_with(query_word)) {
            matched += 1;
            score += 65;
        } else if field.words.iter().any(|word| word.contains(query_word)) {
            matched += 1;
            score += 35;
        }
    }

    if matched == 0 {
        return None;
    }

    if matched == query.words.len() {
        score += 120;
    } else {
        score += matched as i32 * 20;
    }

    Some(score - field.words.len() as i32)
}

fn subsequence_score(query: &str, candidate: &str) -> Option<i32> {
    if query.len() > candidate.len() {
        return None;
    }

    let mut last_match = None;
    let mut total_gap = 0usize;
    let mut matched = 0usize;

    let mut query_chars = query.chars();
    let mut current = query_chars.next()?;

    for (index, candidate_char) in candidate.chars().enumerate() {
        if candidate_char == current {
            matched += 1;
            if let Some(previous) = last_match {
                total_gap += index.saturating_sub(previous + 1);
            } else {
                total_gap += index;
            }
            last_match = Some(index);

            if let Some(next) = query_chars.next() {
                current = next;
            } else {
                let trailing = candidate.len().saturating_sub(index + 1);
                let compactness = candidate.len().saturating_sub(query.len()) as i32;
                return Some(260 - (total_gap as i32 * 10) - (trailing as i32 * 2) - compactness);
            }
        }
    }

    if matched == query.len() {
        Some(180 - (total_gap as i32 * 10))
    } else {
        None
    }
}

fn bounded_levenshtein(left: &str, right: &str, max_distance: usize) -> Option<usize> {
    if left == right {
        return Some(0);
    }

    let left_len = left.chars().count();
    let right_len = right.chars().count();

    if left_len.abs_diff(right_len) > max_distance {
        return None;
    }

    let right_chars = right.chars().collect::<Vec<_>>();
    let mut previous = (0..=right_len).collect::<Vec<_>>();
    let mut current = vec![0usize; right_len + 1];

    for (left_index, left_char) in left.chars().enumerate() {
        current[0] = left_index + 1;
        let mut row_min = current[0];

        for (right_index, right_char) in right_chars.iter().enumerate() {
            let substitution_cost = usize::from(left_char != *right_char);
            let deletion = previous[right_index + 1] + 1;
            let insertion = current[right_index] + 1;
            let substitution = previous[right_index] + substitution_cost;
            let value = deletion.min(insertion).min(substitution);
            current[right_index + 1] = value;
            row_min = row_min.min(value);
        }

        if row_min > max_distance {
            return None;
        }

        std::mem::swap(&mut previous, &mut current);
    }

    let distance = previous[right_len];
    (distance <= max_distance).then_some(distance)
}

fn pick_best(current: Option<ScoredField>, candidate: ScoredField) -> Option<ScoredField> {
    match current {
        Some(existing) if existing.score >= candidate.score => Some(existing),
        _ => Some(candidate),
    }
}

fn apply_package_biases(mut score: i32, package: &Package) -> i32 {
    if package.installed {
        score += 25;
    }
    if package.outdated {
        score += 10;
    }
    if package.deprecated {
        score -= 120;
    }
    if package.disabled {
        score -= 180;
    }

    score
}

fn compare_kind(left: PackageKind, right: PackageKind) -> std::cmp::Ordering {
    match (left, right) {
        (PackageKind::Formula, PackageKind::Cask) => std::cmp::Ordering::Less,
        (PackageKind::Cask, PackageKind::Formula) => std::cmp::Ordering::Greater,
        _ => std::cmp::Ordering::Equal,
    }
}

fn length_penalty(candidate: &str, query: &str) -> i32 {
    ((candidate.len() as i32 - query.len() as i32).max(0)) / 3
}

#[derive(Debug)]
struct SearchField {
    normalized: String,
    words: Vec<String>,
    acronym: String,
    weight: i32,
    exact_reason: &'static str,
    prefix_reason: &'static str,
    contains_reason: &'static str,
    allow_edit_distance: bool,
}

impl SearchField {
    fn new(
        raw: &str,
        weight: i32,
        exact_reason: &'static str,
        prefix_reason: &'static str,
        contains_reason: &'static str,
        allow_edit_distance: bool,
    ) -> Self {
        let words = split_words(raw);
        let normalized = normalize(raw);
        let acronym = build_acronym(&words);

        Self {
            normalized,
            words,
            acronym,
            weight,
            exact_reason,
            prefix_reason,
            contains_reason,
            allow_edit_distance,
        }
    }

    fn name(
        raw: &str,
        weight: i32,
        exact_reason: &'static str,
        prefix_reason: &'static str,
        contains_reason: &'static str,
    ) -> Self {
        Self::new(
            raw,
            weight,
            exact_reason,
            prefix_reason,
            contains_reason,
            true,
        )
    }

    fn description(raw: &str, weight: i32) -> Self {
        Self::new(
            raw,
            weight,
            "exact description match",
            "description prefix",
            "description",
            false,
        )
    }

    fn edit_distance(&self, query: &PreparedQuery) -> Option<usize> {
        if !self.allow_edit_distance || self.normalized.len() > 40 {
            return None;
        }

        let max_distance = match query.normalized.len() {
            0..=4 => 1,
            5..=8 => 2,
            _ => 3,
        };

        bounded_levenshtein(&query.normalized, &self.normalized, max_distance)
    }
}

#[derive(Debug, Clone, Copy)]
struct ScoredField {
    score: i32,
    strength: MatchStrength,
    reason: &'static str,
}

#[derive(Debug)]
struct PreparedQuery {
    normalized: String,
    words: Vec<String>,
}

impl PreparedQuery {
    fn new(raw: &str) -> Self {
        Self {
            normalized: normalize(raw),
            words: split_words(raw),
        }
    }
}

fn normalize(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .map(|character| character.to_ascii_lowercase())
        .collect()
}

fn split_words(value: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();

    for character in value.chars() {
        if character.is_ascii_alphanumeric() {
            current.push(character.to_ascii_lowercase());
        } else if !current.is_empty() {
            words.push(std::mem::take(&mut current));
        }
    }

    if !current.is_empty() {
        words.push(current);
    }

    words
}

fn build_acronym(words: &[String]) -> String {
    words
        .iter()
        .filter_map(|word| word.chars().next())
        .collect::<String>()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{Catalog, Package, PackageKind};
    use crate::cli::QueryScope;

    fn package(
        kind: PackageKind,
        token: &str,
        display_names: &[&str],
        aliases: &[&str],
        desc: &str,
    ) -> Package {
        Package {
            kind,
            token: token.to_string(),
            full_token: token.to_string(),
            display_names: display_names
                .iter()
                .map(|value| value.to_string())
                .collect(),
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
    fn exact_alias_beats_other_matches() {
        let catalog = Catalog {
            generated_at: 0,
            brew_state: None,
            items: vec![
                package(
                    PackageKind::Formula,
                    "ripgrep",
                    &[],
                    &["rg"],
                    "Search tool like grep",
                ),
                package(
                    PackageKind::Formula,
                    "ripgrep-all",
                    &[],
                    &["rga"],
                    "Search PDFs and archives",
                ),
            ],
        };

        let results = search_catalog(
            &catalog,
            "rg",
            SearchOptions {
                scope: QueryScope::All,
                limit: 3,
            },
        );

        assert_eq!(results[0].package.token, "ripgrep");
        assert_eq!(results[0].strength, MatchStrength::Exact);
    }

    #[test]
    fn typo_matching_finds_the_right_formula() {
        let catalog = Catalog {
            generated_at: 0,
            brew_state: None,
            items: vec![
                package(
                    PackageKind::Formula,
                    "ripgrep",
                    &[],
                    &["rg"],
                    "Search tool like grep",
                ),
                package(
                    PackageKind::Formula,
                    "ripme",
                    &[],
                    &[],
                    "Download albums from websites",
                ),
            ],
        };

        let results = search_catalog(
            &catalog,
            "ripgrap",
            SearchOptions {
                scope: QueryScope::All,
                limit: 3,
            },
        );

        assert_eq!(results[0].package.token, "ripgrep");
    }

    #[test]
    fn display_names_help_casks_win() {
        let catalog = Catalog {
            generated_at: 0,
            brew_state: None,
            items: vec![
                package(
                    PackageKind::Cask,
                    "visual-studio-code",
                    &["Microsoft Visual Studio Code", "VS Code"],
                    &[],
                    "Open-source code editor",
                ),
                package(
                    PackageKind::Cask,
                    "vscodium",
                    &["VSCodium"],
                    &[],
                    "Telemetry-free code editor",
                ),
            ],
        };

        let results = search_catalog(
            &catalog,
            "vs code",
            SearchOptions {
                scope: QueryScope::All,
                limit: 3,
            },
        );

        assert_eq!(results[0].package.token, "visual-studio-code");
    }

    #[test]
    fn scope_filter_hides_other_package_types() {
        let catalog = Catalog {
            generated_at: 0,
            brew_state: None,
            items: vec![
                package(
                    PackageKind::Formula,
                    "docker",
                    &[],
                    &[],
                    "Pack and ship software",
                ),
                package(
                    PackageKind::Cask,
                    "docker-desktop",
                    &["Docker Desktop"],
                    &[],
                    "Desktop app for Docker",
                ),
            ],
        };

        let results = search_catalog(
            &catalog,
            "docker",
            SearchOptions {
                scope: QueryScope::Cask,
                limit: 3,
            },
        );

        assert_eq!(results[0].package.kind, PackageKind::Cask);
        assert_eq!(results[0].package.token, "docker-desktop");
    }

    #[test]
    fn installed_packages_get_a_small_ranking_boost() {
        let mut omega = package(
            PackageKind::Formula,
            "omega",
            &[],
            &[],
            "Fast downloader for releases",
        );
        omega.installed = true;

        let catalog = Catalog {
            generated_at: 0,
            brew_state: None,
            items: vec![
                package(
                    PackageKind::Formula,
                    "alpha",
                    &[],
                    &[],
                    "Fast downloader for releases",
                ),
                omega,
            ],
        };

        let results = search_catalog(
            &catalog,
            "fast downloader",
            SearchOptions {
                scope: QueryScope::All,
                limit: 3,
            },
        );

        assert_eq!(results[0].package.token, "omega");
    }
}
