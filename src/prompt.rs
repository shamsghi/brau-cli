use std::io::{self, Write};

use crate::search::SearchMatch;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmedMatchChoice {
    Accept,
    SearchAgain,
    Cancel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatchReviewChoice {
    Proceed,
    RetryAll,
    RetryOne,
    Cancel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchSelection {
    Selected(usize),
    Cancelled,
}

pub fn prompt_yes_no(prompt: &str) -> Result<bool, String> {
    let prompt = format!("{prompt} [Y/n] ");
    prompt_with_parser(&prompt, "Please answer with `y` or `n`.", parse_yes_no)
}

pub fn prompt_confirmed_match_choice() -> Result<ConfirmedMatchChoice, String> {
    prompt_with_parser(
        "Keep this package? [Y/n/q] ",
        "Please answer with `y`, `n`, or `q`.",
        parse_confirmed_match_choice,
    )
}

pub fn prompt_batch_review_choice() -> Result<BatchReviewChoice, String> {
    prompt_with_parser(
        "Choose an option [1-4]: ",
        "Please enter `1`, `2`, `3`, or `4`.",
        parse_batch_review_choice,
    )
}

pub fn prompt_batch_retry_selection(total: usize) -> Result<Option<usize>, String> {
    let prompt = format!("Pick a package to search again [1-{total} or q]: ");
    let invalid_input_message = format!("Please enter a number between 1 and {total}, or `q`.");
    prompt_with_parser(&prompt, &invalid_input_message, |input| {
        parse_batch_retry_selection_input(input, total)
    })
}

pub fn prompt_match_selection<'a>(
    matches: &'a [SearchMatch<'a>],
) -> Result<&'a SearchMatch<'a>, String> {
    match prompt_match_selection_choice(matches.len())? {
        MatchSelection::Selected(index) => Ok(&matches[index]),
        MatchSelection::Cancelled => Err("Action cancelled.".to_string()),
    }
}

pub fn prompt_match_selection_choice(total: usize) -> Result<MatchSelection, String> {
    let prompt = format!("Choose a package [1-{total} or q]: ");
    let invalid_input_message = format!("Please enter a number between 1 and {total} or `q`.");
    prompt_with_parser(&prompt, &invalid_input_message, |input| {
        parse_match_selection_input(input, total)
    })
}

fn prompt_line(prompt: &str) -> Result<String, String> {
    print!("{prompt}");
    io::stdout()
        .flush()
        .map_err(|error| format!("Failed to flush stdout: {error}"))?;

    let mut answer = String::new();
    io::stdin()
        .read_line(&mut answer)
        .map_err(|error| format!("Failed to read your answer: {error}"))?;

    Ok(answer)
}

fn prompt_with_parser<T>(
    prompt: &str,
    invalid_input_message: &str,
    mut parse: impl FnMut(&str) -> Option<T>,
) -> Result<T, String> {
    loop {
        let answer = prompt_line(prompt)?;
        if let Some(value) = parse(&answer) {
            return Ok(value);
        }

        println!("{invalid_input_message}");
    }
}

fn parse_yes_no(input: &str) -> Option<bool> {
    match input.trim().to_ascii_lowercase().as_str() {
        "" | "y" | "yes" => Some(true),
        "n" | "no" => Some(false),
        _ => None,
    }
}

fn parse_selection_index(input: &str, total: usize) -> Option<usize> {
    let index = input.trim().parse::<usize>().ok()?;
    if (1..=total).contains(&index) {
        Some(index - 1)
    } else {
        None
    }
}

fn parse_batch_retry_selection_input(input: &str, total: usize) -> Option<Option<usize>> {
    let trimmed = input.trim();
    if trimmed.eq_ignore_ascii_case("q") {
        return Some(None);
    }

    parse_selection_index(trimmed, total).map(Some)
}

fn parse_match_selection_input(input: &str, total: usize) -> Option<MatchSelection> {
    let trimmed = input.trim();
    if trimmed.eq_ignore_ascii_case("q") {
        return Some(MatchSelection::Cancelled);
    }

    parse_selection_index(trimmed, total).map(MatchSelection::Selected)
}

fn parse_confirmed_match_choice(input: &str) -> Option<ConfirmedMatchChoice> {
    match input.trim().to_ascii_lowercase().as_str() {
        "" | "y" | "yes" => Some(ConfirmedMatchChoice::Accept),
        "n" | "no" => Some(ConfirmedMatchChoice::SearchAgain),
        "q" | "quit" | "cancel" => Some(ConfirmedMatchChoice::Cancel),
        _ => None,
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

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn parse_yes_no_supports_default_affirmative_and_negative() {
        assert_eq!(parse_yes_no(""), Some(true));
        assert_eq!(parse_yes_no("yes"), Some(true));
        assert_eq!(parse_yes_no("n"), Some(false));
        assert_eq!(parse_yes_no("maybe"), None);
    }

    #[test]
    fn parse_batch_retry_selection_supports_number_and_cancel() {
        assert_eq!(parse_batch_retry_selection_input("2", 3), Some(Some(1)));
        assert_eq!(parse_batch_retry_selection_input("q", 3), Some(None));
        assert_eq!(parse_batch_retry_selection_input("4", 3), None);
    }

    #[test]
    fn parse_match_selection_supports_number_and_cancel() {
        assert_eq!(
            parse_match_selection_input("1", 2),
            Some(MatchSelection::Selected(0))
        );
        assert_eq!(
            parse_match_selection_input("q", 2),
            Some(MatchSelection::Cancelled)
        );
        assert_eq!(parse_match_selection_input("0", 2), None);
    }
}
