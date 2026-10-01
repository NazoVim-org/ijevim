use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::time::Instant;
use thiserror::Error;

/// Message severity level for UI display.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MessageLevel {
    #[default]
    Info,
    Warning,
    Error,
}

/// A structured message with level and timestamp.
#[derive(Debug, Clone)]
pub struct EditorMessage {
    pub text: String,
    pub level: MessageLevel,
    pub timestamp: Instant,
}

/// Window split direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitDirection {
    Horizontal,
    Vertical,
}

/// A window viewport within the terminal.
#[derive(Debug, Clone)]
pub struct Window {
    /// Top row (0-indexed, inclusive).
    pub row_start: usize,
    /// Bottom row (exclusive).
    pub row_end: usize,
    /// Left column (0-indexed, inclusive).
    pub col_start: usize,
    /// Right column (exclusive).
    pub col_end: usize,
    /// Buffer line at the top of this window (1-indexed).
    pub scroll_top: usize,
    /// Cursor position within this window.
    pub cursor: Position,
    /// Which buffer this window shows (0 = active, 1.. = inactive_buffers index).
    pub buf_idx: usize,
}

/// Layout of all windows.
#[derive(Debug, Clone, Default)]
pub struct WindowLayout {
    pub windows: Vec<Window>,
    /// Index of the currently focused window.
    pub focused: usize,
}

impl WindowLayout {
    /// Create a new layout with a single full-screen window.
    pub fn new_single(rows: usize, cols: usize) -> Self {
        // Reserve last row for status bar
        let content_rows = rows.saturating_sub(1);
        Self {
            windows: vec![Window {
                row_start: 0,
                row_end: content_rows,
                col_start: 0,
                col_end: cols,
                scroll_top: 1,
                cursor: Position { line: 1, col: 0 },
                buf_idx: 0,
            }],
            focused: 0,
        }
    }

    /// Split the focused window horizontally (top/bottom).
    pub fn split_horizontal(&mut self) {
        let focused_idx = self.focused;
        let window = self.windows[focused_idx].clone();

        // Calculate new heights
        let height = window.row_end - window.row_start;
        let top_height = height / 2;
        let _bottom_height = height - top_height;

        // Top window keeps the top portion
        self.windows[focused_idx].row_end = window.row_start + top_height;

        // Bottom window gets the bottom portion
        let mut new_window = window.clone();
        new_window.row_start = self.windows[focused_idx].row_end;
        new_window.row_end = window.row_end;
        new_window.scroll_top = window.scroll_top;
        new_window.cursor = window.cursor;

        self.windows.insert(focused_idx + 1, new_window);
        self.focused = focused_idx + 1;
    }

    /// Split the focused window vertically (left/right).
    pub fn split_vertical(&mut self) {
        let focused_idx = self.focused;
        let window = self.windows[focused_idx].clone();

        // Calculate new widths
        let width = window.col_end - window.col_start;
        let left_width = width / 2;
        let _right_width = width - left_width;

        // Left window keeps the left portion
        self.windows[focused_idx].col_end = window.col_start + left_width;

        // Right window gets the right portion
        let mut new_window = window.clone();
        new_window.col_start = self.windows[focused_idx].col_end;
        new_window.col_end = window.col_end;
        new_window.scroll_top = window.scroll_top;
        new_window.cursor = window.cursor;

        self.windows.insert(focused_idx + 1, new_window);
        self.focused = focused_idx + 1;
    }

    /// Close the focused window. If it's the last window, do nothing.
    pub fn close_focused(&mut self) {
        if self.windows.len() <= 1 {
            return;
        }
        self.windows.remove(self.focused);
        if self.focused >= self.windows.len() {
            self.focused = self.windows.len() - 1;
        }
    }

    /// Focus the next window (wrap around).
    pub fn focus_next(&mut self) {
        if !self.windows.is_empty() {
            self.focused = (self.focused + 1) % self.windows.len();
        }
    }

    /// Focus the window in the given direction from the focused window.
    pub fn focus_direction(&mut self, direction: Direction) {
        let focused = &self.windows[self.focused];
        let mut best_idx = None;
        let mut best_dist = usize::MAX;

        for (i, w) in self.windows.iter().enumerate() {
            if i == self.focused {
                continue;
            }
            let dist = match direction {
                Direction::Up => {
                    if w.row_end <= focused.row_start {
                        focused.row_start - w.row_end
                    } else {
                        continue;
                    }
                }
                Direction::Down => {
                    if w.row_start >= focused.row_end {
                        w.row_start - focused.row_end
                    } else {
                        continue;
                    }
                }
                Direction::Left => {
                    if w.col_end <= focused.col_start {
                        focused.col_start - w.col_end
                    } else {
                        continue;
                    }
                }
                Direction::Right => {
                    if w.col_start >= focused.col_end {
                        w.col_start - focused.col_end
                    } else {
                        continue;
                    }
                }
            };
            if dist < best_dist {
                best_dist = dist;
                best_idx = Some(i);
            }
        }
        if let Some(idx) = best_idx {
            self.focused = idx;
        }
    }

    /// Equalize all window sizes.
    pub fn equalize(&mut self, total_rows: usize, total_cols: usize) {
        let content_rows = total_rows.saturating_sub(1);
        let n = self.windows.len();
        if n == 0 {
            return;
        }

        // Check if layout is primarily horizontal or vertical
        let horizontal = self
            .windows
            .iter()
            .any(|w| w.row_start != 0 || w.row_end != content_rows);

        if horizontal {
            // Horizontal layout: equalize heights
            let base_height = content_rows / n;
            let extra = content_rows % n;
            let mut row = 0;
            for (i, w) in self.windows.iter_mut().enumerate() {
                let h = base_height + if i < extra { 1 } else { 0 };
                w.row_start = row;
                w.row_end = row + h;
                row += h;
                w.col_start = 0;
                w.col_end = total_cols;
            }
        } else {
            // Vertical layout: equalize widths
            let base_width = total_cols / n;
            let extra = total_cols % n;
            let mut col = 0;
            for (i, w) in self.windows.iter_mut().enumerate() {
                let w_w = base_width + if i < extra { 1 } else { 0 };
                w.col_start = col;
                w.col_end = col + w_w;
                col += w_w;
                w.row_start = 0;
                w.row_end = content_rows;
            }
        }
    }

    /// Maximize focused window vertically.
    pub fn maximize_vertical(&mut self, total_rows: usize, _total_cols: usize) {
        let content_rows = total_rows.saturating_sub(1);
        let focused = &mut self.windows[self.focused];
        focused.row_start = 0;
        focused.row_end = content_rows;
    }

    /// Maximize focused window horizontally.
    pub fn maximize_horizontal(&mut self, _total_rows: usize, total_cols: usize) {
        let focused = &mut self.windows[self.focused];
        focused.col_start = 0;
        focused.col_end = total_cols;
    }
}

/// Direction for window navigation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

/// Crate-wide error type.
///
/// `Box<dyn std::error::Error>` is intentionally avoided at the API boundary so
/// that callers can pattern-match on concrete failure modes. New variants
/// should preserve the `#[from]` conversions that downstream code relies on.
#[derive(Error, Debug)]
pub enum IjevimError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Plugin error: {0}")]
    Plugin(String),
    #[error("Terminal error: {0}")]
    Terminal(String),
    #[error("No file path set")]
    NoFilePath,
}

/// Crate-wide `Result` alias. Use this in lieu of `std::result::Result<T, IjevimError>`
/// to keep signatures short and consistent across the editor core.
pub type Result<T> = std::result::Result<T, IjevimError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Keymap {
    #[default]
    Vim,
    Emacs,
    /// Review mode — temporary overlay for code review workflows.
    Review,
}

impl std::fmt::Display for Keymap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Keymap::Vim => write!(f, "vim"),
            Keymap::Emacs => write!(f, "emacs"),
            Keymap::Review => write!(f, "review"),
        }
    }
}

/// Vim operator kind — replaces `char`-based operator dispatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operator {
    Delete,
    Yank,
    Change,
    IndentRight,
    IndentLeft,
    FindCharForward,
    FindCharBackward,
    TillCharForward,
    TillCharBackward,
    RecordMacro,
    Goto,     // 'g' prefix
    Scroll,   // 'z' prefix
    SaveQuit, // 'Z' prefix
}

/// Pending Vim operator state — replaces multiple `pending_*` fields.
///
/// After pressing an operator key (d, y, c, >, <, etc.), the editor waits
/// for a motion/text-object/second key to complete the operation.
#[derive(Debug, Clone, Default)]
pub enum PendingOperator {
    /// No pending operator.
    #[default]
    Idle,
    /// Operator decided (count stays in `pending_count` for flexibility).
    /// Waiting for motion/text-object/second key.
    AwaitingOperator {
        op: Operator,
        register: Option<char>,
    },
    /// Text object prefix ('i'/'a') decided, waiting for target (w, ", (, etc.).
    AwaitingTextObject {
        op: Operator,
        register: char,
        inner: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Insert,
    Command,
    Visual,
    Replace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisualType {
    Character,
    Line,
    Block,
}

#[derive(Clone, Debug)]
pub struct SearchResult {
    pub line: usize,
    pub start_col: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchDirection {
    Forward,
    Backward,
}

#[derive(Clone, Debug, Default)]
pub struct Marks {
    marks: HashMap<char, Position>,
}

impl Marks {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&mut self, name: char, position: Position) {
        self.marks.insert(name, position);
    }

    pub fn get(&self, name: char) -> Option<Position> {
        self.marks.get(&name).copied()
    }
}

#[derive(Clone, Debug, Default)]
pub struct Macros {
    macros: HashMap<char, Vec<String>>,
    recording: Option<char>,
    /// Recursion depth guard — prevents infinite macro loops.
    depth: usize,
}

impl Macros {
    const MAX_DEPTH: usize = 20;

    pub fn new() -> Self {
        Self::default()
    }

    pub fn start_recording(&mut self, name: char) {
        self.recording = Some(name);
        self.macros.entry(name).or_default();
    }

    pub fn stop_recording(&mut self) -> Option<char> {
        // Pop the last recorded key — it's always the `q` that
        // triggered the stop, emitted by the keymap handler before
        // `handle_normal` had a chance to check `is_recording()`.
        if let Some(name) = self.recording {
            if let Some(keys) = self.macros.get_mut(&name) {
                keys.pop();
            }
        }
        self.recording.take()
    }

    pub fn add_key(&mut self, key: String) {
        if let Some(name) = self.recording {
            if let Some(keys) = self.macros.get_mut(&name) {
                keys.push(key);
            }
        }
    }

    pub fn get(&self, name: char) -> Option<&Vec<String>> {
        self.macros.get(&name)
    }

    pub fn is_recording(&self) -> bool {
        self.recording.is_some()
    }

    /// Try to enter macro playback. Returns `false` if depth limit exceeded.
    pub fn enter_playback(&mut self) -> bool {
        if self.depth >= Self::MAX_DEPTH {
            return false;
        }
        self.depth += 1;
        true
    }

    /// Exit macro playback, decrementing the recursion depth.
    pub fn exit_playback(&mut self) {
        self.depth = self.depth.saturating_sub(1);
    }
}

impl std::fmt::Display for Mode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Mode::Normal => write!(f, "NORMAL"),
            Mode::Insert => write!(f, "INSERT"),
            Mode::Command => write!(f, "COMMAND"),
            Mode::Visual => write!(f, "VISUAL"),
            Mode::Replace => write!(f, "REPLACE"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Position {
    pub line: usize,
    pub col: usize,
}

#[derive(Debug, Clone)]
pub struct EditorState {
    pub mode: Mode,
    pub cursor: Position,
    pub file_path: Option<PathBuf>,
    pub dirty: bool,
    pub command_buffer: String,
    /// Cursor position within `command_buffer` (byte index).
    pub command_cursor_pos: usize,
    pub last_message: Option<String>,
    pub visual_start: Option<Position>,
    pub visual_type: Option<VisualType>,
    pub last_visual_start: Option<Position>,
    pub last_visual_type: Option<VisualType>,
    pub marks: Marks,
    pub macros: Macros,
    pub confirmation_prompt: Option<ConfirmationPrompt>,
    pub show_line_numbers: bool,
    pub show_relative_numbers: bool,
    pub hlsearch: bool,
    pub wrap: bool,
    pub mark: Option<Position>,
    pub region_active: bool,
    /// Display width of a tab character (default: 8).
    pub tabstop: usize,
    /// Number of spaces for each indentation level (default: 4).
    pub shiftwidth: usize,
    /// Insert spaces instead of tab characters (default: false).
    pub expandtab: bool,
    /// Number of lines to keep visible above/below cursor (default: 0).
    pub scrolloff: usize,
    /// Message history (max 100 entries).
    pub message_history: VecDeque<EditorMessage>,
    /// Show the welcome dashboard instead of an empty buffer.
    pub show_dashboard: bool,
}

impl Default for EditorState {
    fn default() -> Self {
        Self {
            mode: Mode::Normal,
            cursor: Position { line: 1, col: 0 },
            file_path: None,
            dirty: false,
            command_buffer: String::new(),
            command_cursor_pos: 0,
            last_message: None,
            visual_start: None,
            visual_type: None,
            last_visual_start: None,
            last_visual_type: None,
            marks: Marks::new(),
            macros: Macros::new(),
            confirmation_prompt: None,
            show_line_numbers: true,
            show_relative_numbers: false,
            hlsearch: true,
            wrap: true,
            mark: None,
            region_active: false,
            tabstop: 8,
            shiftwidth: 4,
            expandtab: false,
            scrolloff: 0,
            message_history: VecDeque::new(),
            show_dashboard: false,
        }
    }
}

/// A single entry in a directory listing buffer.
#[derive(Debug, Clone)]
pub struct DirEntry {
    /// Display name (e.g. "dir_name/" for directories).
    pub name: String,
    /// Absolute path of the entry.
    pub path: PathBuf,
    /// True if this entry is a directory.
    pub is_dir: bool,
}

#[derive(Debug, Clone)]
pub enum PluginEvent {
    ModeChange {
        from: Mode,
        to: Mode,
    },
    BufferChange,
    Key {
        mode: Mode,
        /// String representation of the key (e.g. "a", "Enter", "Esc", "F1").
        /// For `KeyCode::Char(c)`, this is `c.to_string()`.
        /// For other keys, this is the Debug name.
        key: String,
    },
    BufferSave {
        file_path: Option<PathBuf>,
    },
    /// All plugins have been loaded and their `setup()` methods called.
    /// The editor event loop is about to start (or has just started).
    Ready,
}

#[derive(Clone, Debug)]
pub enum ConfirmAction {
    Quit,
    QuitDiscard,
    WriteQuitAll,
    ReloadDiscard,
}

#[derive(Clone, Debug)]
pub struct ConfirmationPrompt {
    pub message: String,
    pub action: ConfirmAction,
}

impl EditorState {
    pub fn has_confirmation(&self) -> bool {
        self.confirmation_prompt.is_some()
    }

    pub fn set_confirmation(&mut self, message: String, action: ConfirmAction) {
        self.confirmation_prompt = Some(ConfirmationPrompt { message, action });
    }

    pub fn clear_confirmation(&mut self) {
        self.confirmation_prompt = None;
    }

    /// Set a transient message (error/status) displayed in the status line.
    /// Shown until the next key press replaces it.
    pub fn set_message(&mut self, msg: impl Into<String>) {
        self.push_message(MessageLevel::Info, msg);
    }

    /// Push a message with a specific level to history and status line.
    pub fn push_message(&mut self, level: MessageLevel, msg: impl Into<String>) {
        let text = msg.into();
        self.last_message = Some(text.clone());
        self.message_history.push_back(EditorMessage {
            text,
            level,
            timestamp: Instant::now(),
        });
        // Limit history to 100 entries.
        if self.message_history.len() > 100 {
            self.message_history.pop_front();
        }
    }

    /// Clear the transient message.
    pub fn clear_message(&mut self) {
        self.last_message = None;
    }
}

/// Cyclic kill-ring: holds Emacs kill history entries.
/// - push(): insert newest entry at front (up to MAX_ENTRIES)
/// - yank(): get the newest entry
/// - yank_pop(): cycle to the previous entry (for future M-y)
#[derive(Clone)]
pub struct KillRing {
    entries: Vec<String>,
    index: usize,
    max_entries: usize,
}

impl KillRing {
    const MAX_ENTRIES: usize = 60;

    pub fn new() -> Self {
        Self {
            entries: Vec::with_capacity(Self::MAX_ENTRIES),
            index: 0,
            max_entries: Self::MAX_ENTRIES,
        }
    }

    pub fn push(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        if self.entries.len() >= self.max_entries {
            self.entries.pop();
        }
        self.entries.insert(0, text.to_string());
        self.index = 0;
    }

    pub fn yank(&self) -> Option<&str> {
        self.entries.first().map(|s| s.as_str())
    }

    /// Return the entry at the current index (for yank-pop cycling).
    pub fn current_yank(&self) -> Option<&str> {
        self.entries.get(self.index).map(|s| s.as_str())
    }

    /// Advance the index and return the next entry (yank-pop forward).
    pub fn yank_pop(&mut self) -> Option<&str> {
        if self.entries.is_empty() {
            return None;
        }
        self.index = (self.index + 1) % self.entries.len();
        self.entries.get(self.index).map(|s| s.as_str())
    }

    pub fn reset_index(&mut self) {
        self.index = 0;
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Default for KillRing {
    fn default() -> Self {
        Self::new()
    }
}

/// Action that can be repeated with `.` in Vim normal mode.
#[derive(Clone)]
pub enum DotAction {
    Insert {
        text: String,
    },
    Delete {
        text: String,
        line: usize,
        col: usize,
    },
    Change {
        text: String,
        line: usize,
        col: usize,
    },
    Replace {
        ch: char,
    },
    /// `J` — join current line with next.
    Join,
    /// `>>` or `<<` — indent or unindent a line.
    Indent {
        unindent: bool,
    },
    /// `~` — toggle case of character under cursor.
    ToggleCase,
}
