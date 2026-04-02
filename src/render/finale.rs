use std::io::{self, Write};
use std::thread;
use std::time::Duration;

use crate::catalog::Package;

use super::{pick_finale_theme, style, FinaleTheme, Style, FINALE_CONFETTI};

pub(super) fn play_install_finale(package: &Package, enabled: bool) {
    if !enabled || !style().should_animate() {
        return;
    }

    let s = style();
    let theme = pick_finale_theme();
    let mut stdout = io::stdout();
    let token = &package.token;
    let total_frames = 12;
    let line_count = 16;

    for frame in 0..total_frames {
        if frame > 0 {
            print!("\x1b[{}A", line_count);
        }

        let lines = build_finale_frame(&s, theme, token, frame, total_frames);
        for (i, line) in lines.iter().enumerate() {
            print!("\r\x1b[2K{line}");
            if i + 1 < lines.len() {
                println!();
            }
        }

        let _ = stdout.flush();

        let delay = match frame {
            0..=2 => 160,
            3..=5 => 130,
            6..=7 => 110,
            8..=9 => 200,
            10 => 300,
            _ => 400,
        };
        thread::sleep(Duration::from_millis(delay));
    }

    println!();
    println!();
}

pub(super) fn play_batch_install_finale(tokens: &[&str], enabled: bool) {
    if !enabled || !style().should_animate() || tokens.is_empty() {
        return;
    }

    let s = style();
    let theme = pick_finale_theme();
    let mut stdout = io::stdout();
    let total_frames = 14;
    let extra_banner_lines = tokens.len().saturating_sub(1);
    let line_count = 16 + extra_banner_lines;

    for frame in 0..total_frames {
        if frame > 0 {
            print!("\x1b[{}A", line_count);
        }

        let lines = build_blender_frame(&s, theme, tokens, frame, total_frames);
        for (i, line) in lines.iter().enumerate() {
            print!("\r\x1b[2K{line}");
            if i + 1 < lines.len() {
                println!();
            }
        }

        let _ = stdout.flush();

        let delay = match frame {
            0..=2 => 140,
            3..=5 => 110,
            6..=8 => 90,
            9..=10 => 180,
            11..=12 => 260,
            _ => 380,
        };
        thread::sleep(Duration::from_millis(delay));
    }

    println!();
    println!();
}

fn build_finale_frame(
    s: &Style,
    t: &FinaleTheme,
    token: &str,
    frame: usize,
    _total: usize,
) -> Vec<String> {
    let mut lines: Vec<String> = Vec::with_capacity(16);
    let fill_level = frame.min(8);
    let celebrating = frame >= 8;
    let banner_reveal = frame >= 10;

    lines.push(build_sparkle_line(s, t, frame, 52));

    if celebrating {
        let cheer_art = [
            r#"        🍻  C H E E R S !  🍻"#,
            r#"       ✦  freshly brewed  ✦"#,
        ];
        lines.push(s.paint_finale_gradient(cheer_art[0], &t.sparkle, frame));
        lines.push(s.paint_finale_gradient(cheer_art[1], &t.gold, frame));
    } else {
        let foam_patterns: [&str; 4] = [
            r#"            ° . ˚    ·  °"#,
            r#"          ˚   ° .  ˚  ·"#,
            r#"            · ˚  °  . ˚"#,
            r#"          °  ·  ˚ .  °"#,
        ];
        let steam_patterns: [&str; 4] = [
            r#"              ≈ ~ ≈"#,
            r#"             ~ ≈ ~"#,
            r#"              ≈ ~ ≈"#,
            r#"             ~ ≈ ~"#,
        ];
        if fill_level >= 4 {
            lines.push(s.paint_finale_gradient(
                steam_patterns[frame % steam_patterns.len()],
                &t.foam,
                frame,
            ));
        } else {
            lines.push(String::new());
        }
        lines.push(s.paint_finale_gradient(
            foam_patterns[frame % foam_patterns.len()],
            &t.foam,
            frame + 2,
        ));
    }

    let foam_top = if fill_level >= 7 {
        r#"          .~~~~~~~~~~~~~~~~~~~~."#
    } else {
        r#"          .--------------------."#
    };
    lines.push(s.paint_finale_gradient(
        foam_top,
        if fill_level >= 7 { &t.foam } else { &t.teal },
        frame,
    ));

    for row in 0..8u8 {
        let filled_from_bottom = fill_level;
        let row_from_bottom = 7 - row;
        let is_filled = (row_from_bottom as usize) < filled_from_bottom;
        let is_foam_row =
            is_filled && (row_from_bottom as usize) >= filled_from_bottom.saturating_sub(1);

        let inner = if !is_filled {
            "                    ".to_string()
        } else if is_foam_row && fill_level < 8 {
            build_foam_inner(t, frame, row)
        } else {
            build_beer_inner(t, frame, row)
        };

        let handle_part = match row {
            1 => "─╮",
            2 => " │",
            3 => " │",
            4 => " │",
            5 => " │",
            6 => "─╯",
            _ => "  ",
        };

        let left_wall = s.paint(t.teal[frame % t.teal.len()], "          |");
        let right_wall = s.paint(t.teal[frame % t.teal.len()], "|");
        let handle = s.paint(t.teal[(frame + 1) % t.teal.len()], handle_part);

        lines.push(format!("{left_wall}{inner}{right_wall}{handle}"));
    }

    lines.push(s.paint_finale_gradient(r#"          '===================='"#, &t.teal, frame));

    if banner_reveal {
        let padded_token = format!("  ✦  {}  ✦  ", token);
        let bar_len = padded_token.chars().count() + 4;
        let bar = "═".repeat(bar_len);
        let padding = " ".repeat(((52usize).saturating_sub(bar_len)) / 2);

        lines.push(s.paint_finale_gradient(&format!("{padding}╔{bar}╗"), &t.green, frame));
        lines.push(s.paint_finale_gradient(
            &format!("{padding}║ {padded_token} ║"),
            &t.sparkle,
            frame,
        ));
        lines.push(s.paint_finale_gradient(&format!("{padding}╚{bar}╝"), &t.green, frame));
    } else {
        let reveal_chars = (frame * token.len()).min(token.len() * 2) / 2;
        let partial: String = token.chars().take(reveal_chars).collect();
        let dots = "·".repeat(token.len().saturating_sub(reveal_chars));
        let loading = format!("          brewing: {partial}{dots}");
        lines.push(s.paint_finale_gradient(&loading, &t.amber, frame));
        lines.push(String::new());
        lines.push(String::new());
    }

    lines.push(build_sparkle_line(s, t, frame + 5, 52));

    while lines.len() < 16 {
        lines.push(String::new());
    }

    lines
}

fn build_beer_inner(t: &FinaleTheme, frame: usize, row: u8) -> String {
    let s = style();
    let mut out = String::new();
    let bubble_positions: [usize; 3] = [
        (frame * 3 + row as usize * 7) % 18,
        (frame * 5 + row as usize * 11 + 3) % 18,
        (frame * 7 + row as usize * 3 + 9) % 18,
    ];

    for col in 0..20 {
        let is_bubble = bubble_positions.contains(&col);
        if is_bubble {
            let bubble_char = match (frame + col) % 3 {
                0 => "°",
                1 => "o",
                _ => "·",
            };
            out.push_str(&s.paint(t.foam[(frame + col) % t.foam.len()], bubble_char));
        } else {
            let fill_char = match (frame + col + row as usize) % 5 {
                0 => "▓",
                1 => "▓",
                2 => "█",
                3 => "▓",
                _ => "█",
            };
            out.push_str(&s.paint(
                t.amber[(frame + col + row as usize) % t.amber.len()],
                fill_char,
            ));
        }
    }
    out
}

fn build_foam_inner(t: &FinaleTheme, frame: usize, row: u8) -> String {
    let s = style();
    let mut out = String::new();
    for col in 0..20 {
        let foam_char = match (frame + col + row as usize) % 6 {
            0 => "░",
            1 => "▒",
            2 => "~",
            3 => "≈",
            4 => "▒",
            _ => "░",
        };
        out.push_str(&s.paint(t.foam[(frame + col) % t.foam.len()], foam_char));
    }
    out
}

fn build_sparkle_line(s: &Style, t: &FinaleTheme, frame: usize, width: usize) -> String {
    let mut out = String::new();
    for i in 0..width {
        let seed = (frame.wrapping_mul(7).wrapping_add(i.wrapping_mul(13))) % 31;
        if seed < 6 {
            let ch = FINALE_CONFETTI[(frame + i) % FINALE_CONFETTI.len()];
            let color = t.sparkle[(frame + i) % t.sparkle.len()];
            out.push_str(&s.paint(color, &ch.to_string()));
        } else {
            out.push(' ');
        }
    }
    out
}

fn build_blender_frame(
    s: &Style,
    t: &FinaleTheme,
    tokens: &[&str],
    frame: usize,
    _total: usize,
) -> Vec<String> {
    let extra_banner = tokens.len().saturating_sub(1);
    let target_lines = 16 + extra_banner;
    let mut lines: Vec<String> = Vec::with_capacity(target_lines);
    let blend_level = frame.min(9);
    let celebrating = frame >= 9;
    let banner_reveal = frame >= 11;

    lines.push(build_sparkle_line(s, t, frame, 52));

    if celebrating {
        let cheer = [
            r#"       🍹  B L E N D E D !  🍹"#,
            r#"      ✦  freshly mixed  ✦"#,
        ];
        lines.push(s.paint_finale_gradient(cheer[0], &t.sparkle, frame));
        lines.push(s.paint_finale_gradient(cheer[1], &t.gold, frame));
    } else {
        let steam: [&str; 4] = [
            r#"              ≈ ~ ≈"#,
            r#"             ~ ≈ ~"#,
            r#"              ≈ ~ ≈"#,
            r#"             ~ ≈ ~"#,
        ];
        if blend_level >= 5 {
            lines.push(s.paint_finale_gradient(steam[frame % steam.len()], &t.foam, frame));
        } else {
            lines.push(String::new());
        }
        let drops: [&str; 4] = [
            r#"          ·  ˚  ∘    ✦  ·  ˚"#,
            r#"            ˚  ·  ✦    ∘  ·"#,
            r#"          ∘    ·  ˚  ·  ✦  ˚"#,
            r#"            ·  ✦  ˚    ·  ∘"#,
        ];
        lines.push(s.paint_finale_gradient(drops[frame % drops.len()], &t.sparkle, frame));
    }

    let rim = if blend_level >= 8 {
        r#"          .~~~~~~~~~~~~~~~~~~~~."#
    } else {
        r#"          .--------------------."#
    };
    lines.push(s.paint_finale_gradient(
        rim,
        if blend_level >= 8 { &t.foam } else { &t.teal },
        frame,
    ));

    for row in 0..8u8 {
        let row_from_bottom = 7 - row;
        let is_filled = (row_from_bottom as usize) < blend_level;

        let inner = if !is_filled {
            "                    ".to_string()
        } else {
            build_blender_inner(t, frame, row)
        };

        let handle_part = match row {
            1 => "─╮",
            2 => " │",
            3 => " │",
            4 => " │",
            5 => " │",
            6 => "─╯",
            _ => "  ",
        };

        let left_wall = s.paint(t.teal[frame % t.teal.len()], "          |");
        let right_wall = s.paint(t.teal[frame % t.teal.len()], "|");
        let handle = s.paint(t.teal[(frame + 1) % t.teal.len()], handle_part);

        lines.push(format!("{left_wall}{inner}{right_wall}{handle}"));
    }

    lines.push(s.paint_finale_gradient(r#"          '===================='"#, &t.teal, frame));

    if banner_reveal {
        let longest = tokens.iter().map(|t| t.chars().count()).max().unwrap_or(0);
        let inner_width = longest + 12;
        let bar_len = inner_width + 2;
        let bar = "═".repeat(bar_len);
        let padding = " ".repeat(52usize.saturating_sub(bar_len + 2) / 2);

        lines.push(s.paint_finale_gradient(&format!("{padding}╔{bar}╗"), &t.green, frame));
        for (i, token) in tokens.iter().enumerate() {
            let label = format!("  ✦  {}  ✦", token);
            let pad_right = inner_width.saturating_sub(label.chars().count());
            let content = format!("{label}{}", " ".repeat(pad_right));
            lines.push(s.paint_finale_gradient(
                &format!("{padding}║ {content} ║"),
                &t.sparkle,
                frame + i,
            ));
        }
        lines.push(s.paint_finale_gradient(&format!("{padding}╚{bar}╝"), &t.green, frame));
    } else {
        let bar_chars = ['▏', '▎', '▍', '▌', '▋', '▊', '▉', '█'];
        let filled = (frame * 2).min(bar_chars.len());
        let progress: String = bar_chars[..filled].iter().collect();
        let loading = format!("          blending: {progress}");
        lines.push(s.paint_finale_gradient(&loading, &t.sparkle, frame));
        for _ in 0..extra_banner {
            lines.push(String::new());
        }
        lines.push(String::new());
        lines.push(String::new());
    }

    lines.push(build_sparkle_line(s, t, frame + 5, 52));

    while lines.len() < target_lines {
        lines.push(String::new());
    }

    lines
}

fn build_blender_inner(t: &FinaleTheme, frame: usize, row: u8) -> String {
    let s = style();
    let mut out = String::new();

    for col in 0..20 {
        let band = (col + (row as usize) * 2 + frame * 3) % 10;

        let ch = match band {
            0 | 5 => "╲",
            1 | 6 => "▓",
            2 | 7 => "█",
            3 | 8 => "╱",
            _ => "▒",
        };

        let color_idx = (frame
            .wrapping_mul(2)
            .wrapping_add(col)
            .wrapping_add((row as usize).wrapping_mul(3)))
            % t.sparkle.len();

        out.push_str(&s.paint(t.sparkle[color_idx], ch));
    }
    out
}
