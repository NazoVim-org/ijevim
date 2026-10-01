//! Welcome dashboard — shown when opening the editor without a file.
//!
//! Renders a centered ASCII art logo, version info, and quick action list
//! into the content area. The dashboard is keyboard-interactive: pressing
//! a shortcut key triggers the associated action.

use crate::screen::{CellStyle, ScreenBuffer};
use crossterm::style::Color;

// ── Named colours ─────────────────────────────────────────────────────
/// Colour for the ASCII art logo text.
const LOGO_FG: Color = Color::AnsiValue(81);
/// Colour for shortcut key brackets (`[e]`, `[q]`, etc.).
const HINT_KEY_FG: Color = Color::AnsiValue(220);
/// Colour for action descriptions.
const HINT_DESC_FG: Color = Color::White;
/// Colour for separators (horizontal lines) and secondary text.
const SEPARATOR_FG: Color = Color::AnsiValue(240);
/// Colour for the version string.
const VERSION_FG: Color = Color::AnsiValue(245);

// ── Logo (IJEVIM in block ASCII) ─────────────────────────────────
static LOGO: &[&str] = &[
    "██╗██████╗██████╗██╗   ██╗██╗███╗   ███╗",
    "██║    ██║██╔═══╝██║   ██║██║████╗ ████║",
    "██║    ██║██████╗██║   ██║██║██╔████╔██║",
    "██║    ██║██╔═══╝╚██╗ ██╔╝██║██║╚██╔╝██║",
    "██║██████║██████╗ ╚████╔╝ ██║██║ ╚═╝ ██║",
    "╚═╝╚═════╝╚═════╝  ╚═══╝  ╚═╝╚═╝     ╚═╝",
];

/// Quick action items: `(key_char, description)`.
const ACTIONS: &[(char, &str)] = &[
    ('e', "Open file explorer"),
    ('n', "New file"),
    ('f', "Find files (git ls-files)"),
    ('q', "Quit"),
];

/// Set all cells in a row to the given character and style.
fn clear_row(screen: &mut ScreenBuffer, row: usize, cols: usize, style: CellStyle) {
    for col in 0..cols {
        screen.set_cell(row, col, ' ', style);
    }
}

/// Render the welcome dashboard into the screen buffer.
///
/// The dashboard occupies the content area (all rows before the status bar).
/// It is drawn centred both vertically and horizontally.
pub fn render_dashboard(screen: &mut ScreenBuffer, rows: usize, cols: usize) {
    let content_rows = rows.saturating_sub(1); // reserve last row for status bar
    if content_rows == 0 {
        return;
    }

    // ── Clear content area ──
    let empty_style = CellStyle::default();
    for row in 0..content_rows {
        clear_row(screen, row, cols, empty_style);
    }

    // ── Compute layout ──
    let logo_height = LOGO.len();

    // How many rows we need for everything
    let needed = logo_height       // logo
        + 1                        // spacing
        + 1                        // version
        + 2                        // spacing + separator
        + 1                        // separator
        + 1                        // spacing
        + ACTIONS.len()            // action items
        + 2; // bottom hint + spacing

    if needed >= content_rows {
        // Not enough room — draw a minimal dashboard
        render_minimal(screen, content_rows, cols);
        return;
    }

    let start_row = (content_rows.saturating_sub(needed)) / 2;

    // ── Logo ──
    let logo_style = CellStyle {
        fg: LOGO_FG,
        bold: true,
        ..CellStyle::default()
    };
    for (i, line) in LOGO.iter().enumerate() {
        let row = start_row + i;
        if row >= content_rows {
            break;
        }
        let line_width = line.chars().count();
        let col = cols.saturating_sub(line_width) / 2;
        for (j, ch) in line.chars().enumerate() {
            let target_col = col + j;
            if target_col < cols {
                screen.set_cell(row, target_col, ch, logo_style);
            }
        }
    }

    // ── Version info ──
    let version_str = format!("v{} — IJEVIM", env!("CARGO_PKG_VERSION"));
    let version_row = start_row + logo_height + 1;
    let version_style = CellStyle {
        fg: VERSION_FG,
        ..CellStyle::default()
    };
    let vcol = cols.saturating_sub(version_str.chars().count()) / 2;
    for (j, ch) in version_str.chars().enumerate() {
        let target_col = vcol + j;
        if target_col < cols {
            screen.set_cell(version_row, target_col, ch, version_style);
        }
    }

    // ── Separator ──
    let sep_row = version_row + 2;
    let sep_style = CellStyle {
        fg: SEPARATOR_FG,
        ..CellStyle::default()
    };
    let sep_len = 40.min(cols.saturating_sub(4));
    let sep_str: String = std::iter::repeat('─').take(sep_len).collect();
    let sep_col = cols.saturating_sub(sep_len) / 2;
    for (j, ch) in sep_str.chars().enumerate() {
        let target_col = sep_col + j;
        if target_col < cols {
            screen.set_cell(sep_row, target_col, ch, sep_style);
        }
    }

    // ── Action items ──
    let action_start_row = sep_row + 2;
    let key_style = CellStyle {
        fg: HINT_KEY_FG,
        bold: true,
        ..CellStyle::default()
    };
    let desc_style = CellStyle {
        fg: HINT_DESC_FG,
        ..CellStyle::default()
    };

    // Width of the widest action line, for centering
    let max_action_width = ACTIONS
        .iter()
        .map(|(key, desc)| format!("[{}]  {}", key, desc).chars().count())
        .max()
        .unwrap_or(0);

    for (i, (key, desc)) in ACTIONS.iter().enumerate() {
        let row = action_start_row + i;
        if row >= content_rows {
            break;
        }

        let action_col = cols.saturating_sub(max_action_width) / 2;

        // Render key part (`[e]`, `[q]`, etc.) in key_style
        let key_text = format!("[{}]", key);
        for (j, ch) in key_text.chars().enumerate() {
            let target_col = action_col + j;
            if target_col < cols {
                screen.set_cell(row, target_col, ch, key_style);
            }
        }

        // Render description in desc_style
        let desc_offset = key_text.chars().count();
        for (j, ch) in desc.chars().enumerate() {
            let target_col = action_col + desc_offset + 2 + j;
            // +2 for the two spaces between key_text and desc
            if target_col < cols {
                screen.set_cell(row, target_col, ch, desc_style);
            }
        }
    }

    // ── Bottom hint line ──
    let hint_row = content_rows.saturating_sub(1);
    let hint_text = "Press any key to dismiss";
    let hint_style = CellStyle {
        fg: SEPARATOR_FG,
        ..CellStyle::default()
    };
    let hint_col = cols.saturating_sub(hint_text.chars().count()) / 2;
    if hint_row < content_rows && hint_row > start_row + needed - 2 {
        for (j, ch) in hint_text.chars().enumerate() {
            let target_col = hint_col + j;
            if target_col < cols {
                screen.set_cell(hint_row, target_col, ch, hint_style);
            }
        }
    }
}

/// Minimal dashboard for tiny terminals (just show actions in a list).
fn render_minimal(screen: &mut ScreenBuffer, content_rows: usize, cols: usize) {
    let key_style = CellStyle {
        fg: HINT_KEY_FG,
        bold: true,
        ..CellStyle::default()
    };
    let desc_style = CellStyle {
        fg: HINT_DESC_FG,
        ..CellStyle::default()
    };

    for (i, (key, desc)) in ACTIONS.iter().enumerate() {
        if i >= content_rows {
            break;
        }
        let key_text = format!("[{}]", key);
        let row = i;
        for (j, ch) in key_text.chars().enumerate() {
            let target_col = 1 + j;
            if target_col < cols {
                screen.set_cell(row, target_col, ch, key_style);
            }
        }
        let desc_offset = key_text.chars().count();
        for (j, ch) in desc.chars().enumerate() {
            let target_col = 1 + desc_offset + 2 + j;
            if target_col < cols {
                screen.set_cell(row, target_col, ch, desc_style);
            }
        }
    }
}
