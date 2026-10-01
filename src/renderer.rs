use crate::buffer::TextBuffer;
use crate::highlight::Style;
use crate::review::ReviewState;
use crate::screen::{build_styled_chars, CellStyle, ScreenBuffer};
use crate::terminal::Terminal;
use crate::types::{EditorState, MessageLevel, Mode, VisualType, Window, WindowLayout};
use crossterm::style::Color;
use std::io;
use unicode_width::UnicodeWidthChar;
use unicode_width::UnicodeWidthStr;

// ── Named colour constants ─────────────────────────────────────────────
const LINE_NUMBER_FG: Color = Color::AnsiValue(240);
const CURRENT_LINE_BG: Color = Color::AnsiValue(235);
const VISUAL_SELECTION_BG: Color = Color::AnsiValue(237);
const STATUS_BAR_BG: Color = Color::AnsiValue(236);
const STATUS_INFO_BG: Color = Color::AnsiValue(21);
const STATUS_WARN_BG: Color = Color::AnsiValue(226);
const CURSOR_FG: Color = Color::Black;
const CURSOR_BG: Color = Color::White;
const STATUS_ERROR_BG: Color = Color::Red;
const STATUS_ERROR_FG: Color = Color::White;
/// Colour for directory entries in a directory listing buffer.
const DIR_ENTRY_FG: Color = Color::AnsiValue(81);

/// Render context for a single window.
struct WindowRenderContext<'a> {
    window: &'a Window,
    active_buffer: &'a TextBuffer,
    inactive_buffers: &'a [(
        TextBuffer,
        crate::undo::UndoManager,
        Option<std::path::PathBuf>,
        crate::types::Position,
    )],
    state: &'a EditorState,
    highlights: &'a [Vec<(Style, String)>],
    is_focused: bool,
    total_cols: usize,
}

/// One visual (display) row produced from wrapping a buffer line.
struct VisualLine {
    /// Buffer line number (1-indexed).
    buffer_line: usize,
    /// Styled characters to display on this screen row.
    styled_chars: Vec<(char, CellStyle)>,
    /// Column offset within the buffer line where this segment starts.
    segment_start_col: usize,
}

pub struct Renderer {
    /// Screen cell buffer for flicker-free diff-based rendering.
    screen: ScreenBuffer,
    /// Cached show_line_numbers for dirty detection.
    #[allow(dead_code)]
    show_line_numbers: bool,
}

impl Renderer {
    pub fn new() -> Self {
        Self {
            screen: ScreenBuffer::new(24, 80),
            show_line_numbers: true,
        }
    }

    /// Main entry point: compose a frame into the screen buffer and emit
    /// minimal ANSI diffs to the terminal.
    ///
    /// `window_layout` contains all windows with their viewports and buffers.
    /// `inactive_buffers` holds buffers not currently in the active window.
    /// `highlights` is the per-line syntect highlight data for the active buffer.
    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &mut self,
        terminal: &mut Terminal,
        active_buffer: &TextBuffer,
        inactive_buffers: &[(
            TextBuffer,
            crate::undo::UndoManager,
            Option<std::path::PathBuf>,
            crate::types::Position,
        )],
        state: &EditorState,
        window_layout: &WindowLayout,
        highlights: &[Vec<(Style, String)>],
        review_state: Option<&ReviewState>,
    ) -> io::Result<()> {
        let rows = terminal.rows() as usize;
        let cols = terminal.cols() as usize;

        // ── Resize screen if needed (handles terminal resize) ──
        let size_changed = self.screen.rows != rows || self.screen.cols != cols;
        self.screen.resize(rows, cols);
        if size_changed {
            self.screen.invalidate_all();
        }

        // ── Welcome dashboard (shown when no file is loaded) ──
        if state.show_dashboard && state.file_path.is_none() {
            crate::dashboard::render_dashboard(&mut self.screen, rows, cols);
            self.render_status_bar(rows, cols, state);
            let ansi_output = self.screen.render();
            if !ansi_output.is_empty() {
                terminal.write_raw(ansi_output.as_bytes())?;
            }
            return Ok(());
        }

        // ── Review mode uses its own rendering ──
        if let Some(rv) = review_state {
            crate::review::render::render_review(&mut self.screen, rv, rows, cols);
            // Status bar is part of review rendering, just emit to terminal
            let ansi_output = self.screen.render();
            if !ansi_output.is_empty() {
                terminal.write_raw(ansi_output.as_bytes())?;
            }
            return Ok(());
        }

        // If no windows, just show empty screen with status bar
        if window_layout.windows.is_empty() {
            self.render_status_bar(rows, cols, state);
            let ansi_output = self.screen.render();
            if !ansi_output.is_empty() {
                terminal.write_raw(ansi_output.as_bytes())?;
            }
            return Ok(());
        }

        // Render each window
        for (idx, window) in window_layout.windows.iter().enumerate() {
            let is_focused = idx == window_layout.focused;
            self.render_window(WindowRenderContext {
                window,
                active_buffer,
                inactive_buffers,
                state,
                highlights,
                is_focused,
                total_cols: cols,
            });
        }

        // Draw window separators
        self.draw_window_separators(window_layout, cols);

        // Render global status bar (last row)
        self.render_status_bar(rows, cols, state);

        // Compute line number prefix for cursor positioning (same logic as render_window).
        let show_any_numbers = state.show_line_numbers || state.show_relative_numbers;
        let line_number_width = if show_any_numbers {
            let total = active_buffer.line_count().max(1);
            total.to_string().len()
        } else {
            0
        };
        let line_number_prefix = if show_any_numbers {
            line_number_width + 2
        } else {
            0
        };

        // Position visual cursor at focused window's cursor position.
        if let Some(focused_window) = window_layout.windows.get(window_layout.focused) {
            // Select the buffer shown in the focused window (same logic as render_window).
            let buf = if focused_window.buf_idx == 0 {
                active_buffer
            } else if focused_window.buf_idx > 0
                && focused_window.buf_idx.saturating_sub(1) < inactive_buffers.len()
            {
                &inactive_buffers[focused_window.buf_idx.saturating_sub(1)].0
            } else {
                active_buffer
            };
            self.position_cursor(focused_window, state, buf, cols, line_number_prefix);
        }

        // Emit frame to terminal
        let ansi_output = self.screen.render();
        if !ansi_output.is_empty() {
            terminal.write_raw(ansi_output.as_bytes())?;
        }

        Ok(())
    }

    /// Render a single window into its viewport.
    fn render_window(&mut self, ctx: WindowRenderContext<'_>) {
        let window = ctx.window;
        let active_buffer = ctx.active_buffer;
        let inactive_buffers = ctx.inactive_buffers;
        let state = ctx.state;
        let highlights = ctx.highlights;
        let _is_focused = ctx.is_focused;
        let total_cols = ctx.total_cols;

        let window_rows = window.row_end.saturating_sub(window.row_start);
        if window_rows == 0 || window.col_end <= window.col_start {
            return;
        }

        // Select the buffer for this window
        let buffer = if window.buf_idx == 0 {
            active_buffer
        } else if window.buf_idx > 0 && window.buf_idx - 1 < inactive_buffers.len() {
            &inactive_buffers[window.buf_idx - 1].0
        } else {
            // Fallback
            active_buffer
        };

        let window_cols = window.col_end.saturating_sub(window.col_start);
        if window_cols == 0 {
            return;
        }

        // Line number metrics for this window
        let show_any_numbers = state.show_line_numbers || state.show_relative_numbers;
        let line_number_width = if show_any_numbers {
            let total = buffer.line_count().max(1);
            total.to_string().len()
        } else {
            0
        };
        let line_number_prefix = if show_any_numbers {
            line_number_width + 2
        } else {
            0
        };
        let content_width = window_cols.saturating_sub(line_number_prefix).max(1);

        // Build visual lines for this window
        let mut visual_lines: Vec<VisualLine> = Vec::new();

        // Build layout starting from window's scroll_top
        let mut buf_line = window.scroll_top.max(1);
        let mut screen_rows_used = 0;
        while screen_rows_used < window_rows && buf_line <= buffer.line_count() {
            let raw_line = buffer.get_line(buf_line);
            let line_highlights = highlights
                .get(buf_line.saturating_sub(1))
                .map(|v| v.as_slice());
            let styled_chars = build_styled_chars(&raw_line, line_highlights);
            let segments = wrap_styled(&styled_chars, content_width, state.wrap, state.tabstop);

            for segment in segments.iter() {
                if screen_rows_used >= window_rows {
                    break;
                }
                visual_lines.push(VisualLine {
                    buffer_line: buf_line,
                    styled_chars: segment.chars.clone(),
                    segment_start_col: segment.start_col,
                });
                screen_rows_used += 1;
            }
            buf_line += 1;
        }

        // Render content rows into screen buffer
        let gray_style = CellStyle {
            fg: LINE_NUMBER_FG,
            ..CellStyle::default()
        };

        for (screen_row, vline) in visual_lines.iter().enumerate() {
            let target_row = window.row_start + screen_row;
            if target_row >= window.row_end {
                break;
            }

            // Line number prefix
            if show_any_numbers {
                let display_num = if state.show_relative_numbers {
                    let dist =
                        (vline.buffer_line as isize - window.scroll_top as isize).unsigned_abs();
                    if dist == 0 && state.show_line_numbers {
                        vline.buffer_line
                    } else {
                        dist
                    }
                } else {
                    vline.buffer_line
                };
                let num_str = format!("{:width$}  ", display_num, width = line_number_width);
                for (col, ch) in num_str.chars().enumerate() {
                    let style = if col < line_number_width {
                        gray_style
                    } else {
                        CellStyle::default()
                    };
                    let target_col = window.col_start + col;
                    if target_col < window.col_end {
                        self.screen.set_cell(target_row, target_col, ch, style);
                    }
                }
            }

            // Content
            let is_dir_line = buffer.is_directory_listing()
                && buffer
                    .selected_entry(vline.buffer_line)
                    .is_some_and(|e| e.is_dir);
            for (col, (ch, style)) in vline.styled_chars.iter().enumerate() {
                let target_col = window.col_start + line_number_prefix + col;
                if target_col < window.col_end && target_col < total_cols {
                    let mut cell_style = *style;
                    // Visual/highlight selection background
                    if let Some((sel_start, sel_end)) =
                        self.selection_line_range(state, buffer, vline.buffer_line)
                    {
                        let buf_col = vline.segment_start_col + col;
                        if buf_col >= sel_start && buf_col < sel_end {
                            cell_style.bg = VISUAL_SELECTION_BG;
                        }
                    }
                    // Highlight current line in focused window
                    if vline.buffer_line == self.get_focused_cursor_line(window)
                        && cell_style.bg != VISUAL_SELECTION_BG
                    {
                        cell_style.bg = CURRENT_LINE_BG;
                    }
                    // Directory entries in file browser are shown in a distinct colour
                    if is_dir_line {
                        cell_style.fg = DIR_ENTRY_FG;
                    }
                    self.screen
                        .set_cell(target_row, target_col, *ch, cell_style);
                }
            }

            // Fill remainder with spaces — use display-width aware fill position
            let content_display_width: usize = vline
                .styled_chars
                .iter()
                .map(|(ch, _)| char_display_width(*ch))
                .sum();
            let last_col =
                window.col_start + line_number_prefix + content_display_width.min(content_width);
            for col in last_col..window.col_end.min(total_cols) {
                self.screen
                    .set_cell(target_row, col, ' ', CellStyle::default());
            }
        }

        // Fill remaining rows with ~ or spaces
        for screen_row in visual_lines.len()..window_rows {
            let target_row = window.row_start + screen_row;
            if target_row >= window.row_end {
                break;
            }
            if show_any_numbers {
                let num_str = format!("{:width$}  ", "~", width = line_number_width);
                for (col, ch) in num_str.chars().enumerate() {
                    let style = if col < line_number_width {
                        gray_style
                    } else {
                        CellStyle::default()
                    };
                    let target_col = window.col_start + col;
                    if target_col < window.col_end {
                        self.screen.set_cell(target_row, target_col, ch, style);
                    }
                }
            }
            for col in (window.col_start + line_number_prefix)..window.col_end.min(total_cols) {
                self.screen
                    .set_cell(target_row, col, ' ', CellStyle::default());
            }
        }
    }

    /// Get the cursor line for the focused window (for line highlighting).
    fn get_focused_cursor_line(&self, window: &Window) -> usize {
        window.cursor.line
    }

    /// Return the column range `(start, end)` of visual selection on `buf_line`,
    /// or `None` if the given line is not part of the selection.
    ///
    /// Handles Vim visual (character/line/block) and Emacs region.
    fn selection_line_range(
        &self,
        state: &EditorState,
        _buffer: &TextBuffer,
        buf_line: usize,
    ) -> Option<(usize, usize)> {
        // ── Emacs region ──
        if state.region_active {
            if let Some(mark) = state.mark {
                let s_line = mark.line.min(state.cursor.line);
                let e_line = mark.line.max(state.cursor.line);
                if buf_line < s_line || buf_line > e_line {
                    return None;
                }
                let (s_col, e_col) = if mark.line < state.cursor.line {
                    if buf_line == mark.line {
                        (mark.col, usize::MAX)
                    } else if buf_line == state.cursor.line {
                        (0, state.cursor.col + 1)
                    } else {
                        (0, usize::MAX)
                    }
                } else if mark.line > state.cursor.line {
                    if buf_line == state.cursor.line {
                        (state.cursor.col, usize::MAX)
                    } else if buf_line == mark.line {
                        (0, mark.col + 1)
                    } else {
                        (0, usize::MAX)
                    }
                } else {
                    // Same line
                    let s = mark.col.min(state.cursor.col);
                    let e = mark.col.max(state.cursor.col);
                    (s, e + 1)
                };
                return Some((s_col, e_col));
            }
        }

        // ── Vim visual mode ──
        if state.mode != Mode::Visual {
            return None;
        }
        let visual_start = state.visual_start?;
        let cursor = state.cursor;
        let vtype = state.visual_type?;

        Some(match vtype {
            VisualType::Line => {
                let s_line = visual_start.line.min(cursor.line);
                let e_line = visual_start.line.max(cursor.line);
                if buf_line < s_line || buf_line > e_line {
                    return None;
                }
                (0, usize::MAX)
            }
            VisualType::Block => {
                let s_line = visual_start.line.min(cursor.line);
                let e_line = visual_start.line.max(cursor.line);
                if buf_line < s_line || buf_line > e_line {
                    return None;
                }
                let s_col = visual_start.col.min(cursor.col);
                let e_col = visual_start.col.max(cursor.col);
                (s_col, e_col + 1)
            }
            VisualType::Character => {
                let s_line = visual_start.line.min(cursor.line);
                let e_line = visual_start.line.max(cursor.line);
                if buf_line < s_line || buf_line > e_line {
                    return None;
                }
                let (s_col, e_col) = if visual_start.line < cursor.line {
                    if buf_line == visual_start.line {
                        (visual_start.col, usize::MAX)
                    } else if buf_line == cursor.line {
                        (0, cursor.col + 1)
                    } else {
                        (0, usize::MAX)
                    }
                } else if visual_start.line > cursor.line {
                    if buf_line == cursor.line {
                        (cursor.col, usize::MAX)
                    } else if buf_line == visual_start.line {
                        (0, visual_start.col + 1)
                    } else {
                        (0, usize::MAX)
                    }
                } else {
                    // Same line
                    let s = visual_start.col.min(cursor.col);
                    let e = visual_start.col.max(cursor.col);
                    (s, e + 1)
                };
                (s_col, e_col)
            }
        })
    }

    /// Draw vertical/horizontal separators between windows.
    fn draw_window_separators(&mut self, layout: &WindowLayout, total_cols: usize) {
        if layout.windows.len() <= 1 {
            return;
        }

        // Find vertical splits (same row range, adjacent columns)
        for i in 0..layout.windows.len() {
            for j in i + 1..layout.windows.len() {
                let w1 = &layout.windows[i];
                let w2 = &layout.windows[j];

                // Vertical separator: same rows, adjacent columns
                if w1.row_start == w2.row_start && w1.row_end == w2.row_end {
                    if w1.col_end == w2.col_start {
                        // Vertical line at w1.col_end
                        let col = w1.col_end.min(total_cols);
                        for row in w1.row_start..w1.row_end {
                            self.screen.set_cell(row, col, '│', CellStyle::default());
                        }
                    } else if w2.col_end == w1.col_start {
                        let col = w2.col_end.min(total_cols);
                        for row in w1.row_start..w1.row_end {
                            self.screen.set_cell(row, col, '│', CellStyle::default());
                        }
                    }
                }

                // Horizontal separator: same columns, adjacent rows
                if w1.col_start == w2.col_start && w1.col_end == w2.col_end {
                    if w1.row_end == w2.row_start {
                        let row = w1.row_end;
                        for col in w1.col_start..w1.col_end.min(total_cols) {
                            self.screen.set_cell(row, col, '─', CellStyle::default());
                        }
                    } else if w2.row_end == w1.row_start {
                        let row = w2.row_end;
                        for col in w1.col_start..w1.col_end.min(total_cols) {
                            self.screen.set_cell(row, col, '─', CellStyle::default());
                        }
                    }
                }
            }
        }
    }

    /// Render global status bar at the bottom.
    fn render_status_bar(&mut self, rows: usize, cols: usize, state: &EditorState) {
        let status_row = rows.saturating_sub(1);
        let is_show_message = state.last_message.is_some();
        let status_style = if is_show_message {
            let level = state
                .message_history
                .back()
                .map(|m| m.level)
                .unwrap_or(MessageLevel::Info);
            match level {
                MessageLevel::Error => CellStyle {
                    bg: STATUS_ERROR_BG,
                    fg: STATUS_ERROR_FG,
                    bold: true,
                    ..CellStyle::default()
                },
                MessageLevel::Warning => CellStyle {
                    bg: STATUS_WARN_BG,
                    fg: Color::Black,
                    bold: true,
                    ..CellStyle::default()
                },
                MessageLevel::Info => CellStyle {
                    bg: STATUS_INFO_BG,
                    fg: Color::White,
                    bold: true,
                    ..CellStyle::default()
                },
            }
        } else {
            CellStyle {
                bg: STATUS_BAR_BG,
                fg: Color::White,
                ..CellStyle::default()
            }
        };

        let text = if let Some(ref msg) = state.last_message {
            format!(" {} ", msg)
        } else if state.has_confirmation() {
            state
                .confirmation_prompt
                .as_ref()
                .map(|c| c.message.clone())
                .unwrap_or_default()
        } else if state.mode == Mode::Command {
            format!(":{}", state.command_buffer)
        } else if state.show_dashboard {
            " DASHBOARD ".to_string()
        } else {
            let mode_label = if state.region_active {
                if state.mark == Some(state.cursor) {
                    " MARK "
                } else {
                    " REGION "
                }
            } else {
                match state.mode {
                    Mode::Normal => " NORMAL ",
                    Mode::Insert => " INSERT ",
                    Mode::Visual => " VISUAL ",
                    Mode::Command => " COMMAND ",
                    Mode::Replace => " REPLACE ",
                }
            };

            let file = state
                .file_path
                .as_ref()
                .map(|p| p.to_str().unwrap_or("[Invalid Path]"))
                .unwrap_or("[No Name]");

            if state.mode == Mode::Replace {
                format!(
                    "--{}-- {} {}:{}",
                    mode_label,
                    file,
                    state.cursor.line,
                    state.cursor.col + 1,
                )
            } else {
                let dirty = if state.dirty { "[+]" } else { "" };
                format!(
                    "--{}-- {} {} {}:{}",
                    mode_label,
                    file,
                    dirty,
                    state.cursor.line,
                    state.cursor.col + 1,
                )
            }
        };

        // Clear the whole status row with status style
        for col in 0..cols {
            self.screen.set_cell(status_row, col, ' ', status_style);
        }

        // Write text
        for (col, ch) in text.chars().enumerate() {
            if col < cols {
                self.screen.set_cell(status_row, col, ch, status_style);
            }
        }
    }

    /// Position a visual block cursor at the focused window's cursor position.
    ///
    /// Uses `window.cursor` (synced with `state.cursor` by the editor before
    /// every render) and computes the visual row/column on the fly via
    /// [`wrapped_rows`] so wrapped lines are handled correctly.
    ///
    /// Unlike the previous implementation, this preserves the character
    /// under the cursor and renders it with inverted colours.
    fn position_cursor(
        &mut self,
        window: &Window,
        state: &EditorState,
        buffer: &TextBuffer,
        _total_cols: usize,
        line_number_prefix: usize,
    ) {
        let window_height = window.row_end.saturating_sub(window.row_start);
        let window_cols = window.col_end.saturating_sub(window.col_start);
        let content_width = window_cols.saturating_sub(line_number_prefix).max(1);
        let tabstop = state.tabstop;

        let cursor_line = window.cursor.line;
        let cursor_col = window.cursor.col;

        // ── Count visual rows consumed by lines before the cursor line ──
        let start_line = window.scroll_top.max(1);
        let end_line = cursor_line.min(buffer.line_count());
        let mut visual_row_offset = 0usize;

        for line_num in start_line..end_line {
            let line_text = buffer.get_line(line_num);
            visual_row_offset += wrapped_rows(&line_text, content_width, true, tabstop);
        }

        // ── Find the segment within the cursor's own line ──
        let mut segment_start_col = 0usize;
        let mut in_view = false;

        if cursor_line >= start_line && cursor_line <= buffer.line_count() {
            let line_text = buffer.get_line(cursor_line);
            let line_width = unicode_width_str(&line_text);
            let num_segments = line_width.div_ceil(content_width).max(1);

            // Use display-width-aware segment calculation
            let seg_idx = find_segment_for_col(&line_text, cursor_col, content_width, tabstop);
            let seg_idx = seg_idx.min(num_segments.saturating_sub(1));
            visual_row_offset += seg_idx;
            // Compute segment start column in display-width terms
            segment_start_col = find_segment_start_col(&line_text, seg_idx, content_width, tabstop);
            in_view = true;
        }

        // ── Row within the window viewport ──
        let row = if in_view {
            window.row_start + visual_row_offset.min(window_height.saturating_sub(1))
        } else {
            // Fallback: approximate (no wrapping)
            window.row_start
                + (cursor_line.saturating_sub(window.scroll_top))
                    .min(window_height.saturating_sub(1))
        };

        // ── Column within the segment (display-width aware) ──
        let visual_col = display_col_within_segment(
            &buffer.get_line(cursor_line),
            cursor_col,
            segment_start_col,
            tabstop,
        );
        let col = (window.col_start + line_number_prefix + visual_col)
            .min(window.col_end.saturating_sub(1));

        // Get the character under the cursor to render it inverted
        let line_chars: Vec<char> = buffer.line_chars(cursor_line);
        let cursor_ch = line_chars.get(cursor_col).copied().unwrap_or(' ');

        self.screen.set_cell(
            row,
            col,
            cursor_ch,
            CellStyle {
                fg: CURSOR_FG,
                bg: CURSOR_BG,
                ..CellStyle::default()
            },
        );
    }
}

impl Default for Renderer {
    fn default() -> Self {
        Self::new()
    }
}

// ── Free helper functions ──

/// Display width of a character using unicode-width.
/// Treats tabs as having width 1 (actual tab rendering is handled by the
/// terminal when we output the tab character).
fn char_display_width(ch: char) -> usize {
    if ch == '\n' {
        return 0;
    }
    UnicodeWidthChar::width(ch).unwrap_or(0)
}

/// Display width of a string.
fn unicode_width_str(s: &str) -> usize {
    // Exclude trailing newline from width calculation
    let trimmed = s.strip_suffix('\n').unwrap_or(s);
    UnicodeWidthStr::width(trimmed)
}

/// Number of wrapped rows a line would occupy.
pub fn wrapped_rows(text: &str, max_width: usize, wrap: bool, _tabstop: usize) -> usize {
    if !wrap {
        return 1;
    }
    let width = unicode_width_str(text);
    if width == 0 {
        return 1;
    }
    width.div_ceil(max_width)
}

/// A wrapped segment: the characters + the starting column offset in the buffer line.
pub(crate) struct WrappedSegment {
    pub(crate) chars: Vec<(char, CellStyle)>,
    pub(crate) start_col: usize,
}

/// Split styled chars into screen rows, tracking display width and segment start.
pub(crate) fn wrap_styled(
    styled_chars: &[(char, CellStyle)],
    max_width: usize,
    wrap: bool,
    _tabstop: usize,
) -> Vec<WrappedSegment> {
    if !wrap || styled_chars.is_empty() {
        return vec![WrappedSegment {
            chars: styled_chars.to_vec(),
            start_col: 0,
        }];
    }

    let mut rows: Vec<WrappedSegment> = Vec::new();
    let mut current = Vec::new();
    let mut current_width = 0usize;
    let mut col_offset = 0usize;
    let mut segment_start = 0usize;

    for (ch, style) in styled_chars {
        let ch_width = char_display_width(*ch);
        if ch_width == 0 {
            // Zero-width characters (combining, control): still include in output
            // but don't advance display width for wrapping purposes.
            current.push((*ch, *style));
            col_offset += 1; // Advance char index even if display width is 0
            continue;
        }
        if current_width + ch_width > max_width && !current.is_empty() {
            rows.push(WrappedSegment {
                chars: current,
                start_col: segment_start,
            });
            current = Vec::new();
            current_width = 0;
            segment_start = col_offset;
        }
        current.push((*ch, *style));
        current_width += ch_width;
        col_offset += 1; // Track char index offset (not display width)
    }

    if !current.is_empty() {
        rows.push(WrappedSegment {
            chars: current,
            start_col: segment_start,
        });
    }
    if rows.is_empty() {
        rows.push(WrappedSegment {
            chars: Vec::new(),
            start_col: 0,
        });
    }
    rows
}

/// Find which display-width segment the given column falls into.
fn find_segment_for_col(line: &str, col: usize, content_width: usize, _tabstop: usize) -> usize {
    if content_width == 0 {
        return 0;
    }
    let mut seg_idx = 0usize;
    let mut w = 0usize;
    for (i, ch) in line.chars().enumerate() {
        if i >= col {
            break;
        }
        let cw = char_display_width(ch);
        if w + cw > content_width {
            seg_idx += 1;
            w = 0;
        }
        w += cw;
    }
    seg_idx
}

/// Find the starting column (in char index terms) of the given segment.
fn find_segment_start_col(
    line: &str,
    seg_idx: usize,
    content_width: usize,
    _tabstop: usize,
) -> usize {
    let mut seg = 0usize;
    let mut w = 0usize;
    for (i, ch) in line.chars().enumerate() {
        if seg == seg_idx {
            return i;
        }
        let cw = char_display_width(ch);
        if w + cw > content_width {
            seg += 1;
            w = 0;
        }
        w += cw;
    }
    0
}

/// Compute the visual column position of `cursor_col` within its wrapped segment.
fn display_col_within_segment(
    line: &str,
    cursor_col: usize,
    segment_start_col: usize,
    _tabstop: usize,
) -> usize {
    let mut dcol = 0usize;
    for (i, ch) in line.chars().enumerate() {
        if i >= cursor_col || i < segment_start_col {
            continue;
        }
        dcol += char_display_width(ch);
    }
    dcol
}
