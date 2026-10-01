use crate::editor::Editor;
use crate::types::{ConfirmAction, MessageLevel, Mode, SearchDirection};
use crossterm::event::{KeyCode, KeyModifiers};

impl Editor {
    pub(crate) async fn handle_command(&mut self, key: KeyCode, modifiers: KeyModifiers) {
        if self.engine.state.has_confirmation() {
            self.handle_confirmation(key).await;
            return;
        }

        match key {
            KeyCode::Enter => {
                let cmd = self.engine.state.command_buffer.trim().to_string();
                let cmd = cmd.strip_prefix(':').unwrap_or(&cmd);
                self.engine.state.command_buffer.clear();
                self.engine.state.command_cursor_pos = 0;

                match cmd {
                    "q" => {
                        self.handle_quit();
                    }
                    "q!" => {
                        self.running = false;
                    }
                    "w" | "w!" => {
                        self.save_and_emit().await;
                    }
                    "wq" | "wq!" | "wqa" | "wa" => {
                        self.save_and_emit().await;
                        self.running = false;
                    }
                    "qa" => {
                        self.running = false;
                    }
                    "e" => {
                        self.reload_file().await;
                    }
                    "e!" => {
                        self.reload_file_discard().await;
                    }
                    cmd if cmd.starts_with("keymap ") => {
                        let keymap_name = cmd.trim_start_matches("keymap ").trim();
                        self.switch_keymap(keymap_name);
                    }
                    "bn" | "bnext" => {
                        if let Err(e) = self.engine.buffer_next() {
                            self.engine.state.push_message(MessageLevel::Error, e);
                        }
                        self.needs_render = true;
                    }
                    "bp" | "bprevious" | "bprev" => {
                        if let Err(e) = self.engine.buffer_prev() {
                            self.engine.state.push_message(MessageLevel::Error, e);
                        }
                        self.needs_render = true;
                    }
                    "bd" | "bdelete" => {
                        if let Err(e) = self.engine.buffer_delete() {
                            self.engine.state.push_message(MessageLevel::Error, e);
                        }
                        self.needs_render = true;
                    }
                    "Ex" | "Explore" => {
                        self.engine.state.show_dashboard = false;
                        match std::env::current_dir() {
                            Ok(cwd) => {
                                let path = cwd.to_string_lossy().to_string();
                                if let Err(e) = self.engine.buffer_open_dir(&path) {
                                    self.engine.state.push_message(MessageLevel::Error, e);
                                }
                            }
                            Err(e) => {
                                self.engine.state.push_message(
                                    MessageLevel::Error,
                                    format!("Cannot get current directory: {}", e),
                                );
                            }
                        }
                        self.needs_render = true;
                    }
                    "ls" | "buffers" => {
                        let list = self.engine.buffer_list();
                        self.engine.state.push_message(MessageLevel::Info, list);
                    }
                    "sp" | "split" => {
                        if let Err(e) = self.engine.split_horizontal() {
                            self.engine.state.push_message(MessageLevel::Error, e);
                        }
                        self.needs_render = true;
                    }
                    "vs" | "vsplit" => {
                        if let Err(e) = self.engine.split_vertical() {
                            self.engine.state.push_message(MessageLevel::Error, e);
                        }
                        self.needs_render = true;
                    }
                    "messages" => {
                        let history: Vec<_> =
                            self.engine.state.message_history.iter().cloned().collect();
                        if history.is_empty() {
                            self.engine
                                .state
                                .push_message(MessageLevel::Info, "No messages".to_string());
                        } else {
                            // Display all messages in history
                            for msg in history {
                                let prefix = match msg.level {
                                    MessageLevel::Error => "[ERROR]",
                                    MessageLevel::Warning => "[WARN]",
                                    MessageLevel::Info => "[INFO]",
                                };
                                self.engine
                                    .state
                                    .push_message(msg.level, format!("{} {}", prefix, msg.text));
                            }
                        }
                    }
                    cmd if cmd.starts_with("e ") || cmd == "e." => {
                        self.engine.state.show_dashboard = false;
                        let path = if cmd == "e." {
                            "."
                        } else {
                            cmd.trim_start_matches("e ").trim()
                        };
                        let p = std::path::Path::new(path);
                        if p.is_dir() {
                            if let Err(e) = self.engine.buffer_open_dir(path) {
                                self.engine.state.push_message(MessageLevel::Error, e);
                            }
                        } else if let Err(e) = self.engine.buffer_open(path) {
                            self.engine.state.push_message(MessageLevel::Error, e);
                        }
                        self.needs_render = true;
                    }
                    _ => {
                        if let Some(query) = cmd.strip_prefix('/') {
                            let direction = SearchDirection::Forward;
                            self.do_search(query, direction);
                        } else if let Some(query) = cmd.strip_prefix('?') {
                            let direction = SearchDirection::Backward;
                            self.do_search(query, direction);
                        } else if cmd.starts_with("set ") {
                            self.handle_set_command(cmd);
                        } else if cmd == "nohlsearch" || cmd == "noh" {
                            self.engine.state.hlsearch = false;
                        } else if cmd == "review" || cmd.starts_with("review ") {
                            let args_str = cmd.trim_start_matches("review").trim();
                            let args = crate::review::ReviewArgs::parse(args_str);
                            self.enter_review_mode(args);
                            self.needs_render = true;
                        } else if cmd.starts_with("w ") || cmd.starts_with("w! ") {
                            self.handle_write_path(cmd).await;
                        } else if cmd.starts_with("wq ") || cmd.starts_with("wq! ") {
                            self.handle_write_quit_path(cmd).await;
                        } else if cmd == "branch" {
                            self.handle_git_command(&["branch"]);
                        } else if cmd.starts_with("branch ") {
                            let args = cmd.trim_start_matches("branch ").trim();
                            if args.starts_with("-d") || args.starts_with("-D") {
                                let parts: Vec<&str> = args.splitn(2, ' ').collect();
                                let flag = parts[0];
                                let name = parts.get(1).unwrap_or(&"");
                                self.handle_git_command(&["branch", flag, name]);
                            } else if args == "-a" || args == "-r" {
                                self.handle_git_command(&["branch", args]);
                            } else {
                                let branch_name = args;
                                self.handle_git_command(&["checkout", branch_name]);
                                self.reload_file_after_checkout();
                            }
                        } else if cmd.starts_with("git ") {
                            let git_args_str = cmd.trim_start_matches("git ").trim();
                            let git_args: Vec<&str> = git_args_str.split_whitespace().collect();
                            if !git_args.is_empty() {
                                self.handle_git_command(&git_args);
                                // Reload file if switching branches
                                if git_args.first() == Some(&"checkout") {
                                    self.reload_file_after_checkout();
                                }
                            }
                        } else if cmd.starts_with("%s/") || cmd.starts_with("s/") {
                            self.handle_substitute(cmd);
                        } else {
                            // Split cmd into command name and args
                            let parts: Vec<&str> = cmd.splitn(2, ' ').collect();
                            let cmd_name = parts[0];
                            let args: Vec<String> = if parts.len() > 1 {
                                parts[1].split(' ').map(|s| s.to_string()).collect()
                            } else {
                                vec![]
                            };
                            if !self.engine.plugin_manager.execute_command(cmd_name, args) {
                                self.engine.state.push_message(
                                    MessageLevel::Warning,
                                    format!("Unknown command: {}", cmd),
                                );
                            }
                        }
                    }
                }

                if !self.engine.state.has_confirmation() {
                    self.transition_mode(Mode::Normal);
                    self.needs_render = true;
                }
            }
            KeyCode::Esc => {
                self.engine.state.command_buffer.clear();
                self.engine.state.command_cursor_pos = 0;
                self.transition_mode(Mode::Normal);
                self.needs_render = true;
            }
            KeyCode::Backspace => {
                let pos = self.engine.state.command_cursor_pos;
                if pos > 0 {
                    // Find the previous char boundary.
                    let prev = self.engine.state.command_buffer[..pos]
                        .char_indices()
                        .next_back()
                        .map(|(i, _)| i)
                        .unwrap_or(0);
                    self.engine.state.command_buffer.drain(prev..pos);
                    self.engine.state.command_cursor_pos = prev;
                }
                self.needs_render = true;
            }
            // Ctrl-a: move cursor to beginning of command line
            KeyCode::Char('a') if modifiers.contains(KeyModifiers::CONTROL) => {
                self.engine.state.command_cursor_pos = 0;
                self.needs_render = true;
            }
            // Ctrl-e: move cursor to end of command line
            KeyCode::Char('e') if modifiers.contains(KeyModifiers::CONTROL) => {
                self.engine.state.command_cursor_pos = self.engine.state.command_buffer.len();
                self.needs_render = true;
            }
            // Ctrl-w: delete word backward
            KeyCode::Char('w') if modifiers.contains(KeyModifiers::CONTROL) => {
                let pos = self.engine.state.command_cursor_pos;
                if pos > 0 {
                    let buf = &self.engine.state.command_buffer[..pos];
                    // Skip trailing whitespace, then skip non-whitespace.
                    let without_trailing = buf.trim_end_matches(' ');
                    let without_word = without_trailing
                        .rsplit_once(char::is_whitespace)
                        .map(|(before, _)| before.len())
                        .unwrap_or(0);
                    self.engine.state.command_buffer.drain(without_word..pos);
                    self.engine.state.command_cursor_pos = without_word;
                }
                self.needs_render = true;
            }
            KeyCode::Char(c) => {
                let pos = self.engine.state.command_cursor_pos;
                self.engine.state.command_buffer.insert(pos, c);
                self.engine.state.command_cursor_pos = pos + c.len_utf8();
                self.needs_render = true;
            }
            // Left arrow: move cursor left
            KeyCode::Left => {
                let pos = self.engine.state.command_cursor_pos;
                if pos > 0 {
                    let prev = self.engine.state.command_buffer[..pos]
                        .char_indices()
                        .next_back()
                        .map(|(i, _)| i)
                        .unwrap_or(0);
                    self.engine.state.command_cursor_pos = prev;
                }
                self.needs_render = true;
            }
            // Right arrow: move cursor right
            KeyCode::Right => {
                let pos = self.engine.state.command_cursor_pos;
                if pos < self.engine.state.command_buffer.len() {
                    let next = self.engine.state.command_buffer[pos..]
                        .char_indices()
                        .nth(1)
                        .map(|(i, _)| pos + i)
                        .unwrap_or(self.engine.state.command_buffer.len());
                    self.engine.state.command_cursor_pos = next;
                }
                self.needs_render = true;
            }
            _ => {}
        }
    }

    pub fn handle_quit(&mut self) {
        if self.engine.buffer.is_dirty() {
            self.engine.state.set_confirmation(
                "No write since last change. Quit anyway? (y/n Enter/Esc: yes, n: no)".to_string(),
                ConfirmAction::Quit,
            );
            self.needs_render = true;
        } else {
            self.running = false;
            self.engine.state.mode = Mode::Normal;
        }
    }

    pub(crate) async fn handle_confirmation(&mut self, key: KeyCode) {
        let should_quit = match key {
            KeyCode::Char('y') | KeyCode::Enter => true,
            KeyCode::Char('n') | KeyCode::Esc => false,
            _ => {
                self.engine.state.clear_confirmation();
                self.needs_render = true;
                return;
            }
        };

        let action = self
            .engine
            .state
            .confirmation_prompt
            .as_ref()
            .unwrap()
            .action
            .clone();
        self.engine.state.clear_confirmation();
        self.needs_render = true;

        match action {
            ConfirmAction::Quit => {
                if should_quit {
                    self.running = false;
                    self.engine.state.mode = Mode::Normal;
                }
            }
            ConfirmAction::QuitDiscard => {
                self.running = false;
                self.engine.state.mode = Mode::Normal;
            }
            ConfirmAction::WriteQuitAll => {
                if !should_quit {
                    return;
                }
                if let Err(e) = self.engine.buffer.save_file().await {
                    self.engine
                        .state
                        .push_message(MessageLevel::Error, format!("Save failed: {}", e));
                } else {
                    self.engine
                        .plugin_manager
                        .emit(crate::types::PluginEvent::BufferSave {
                            file_path: self.engine.state.file_path.clone(),
                        });
                    self.running = false;
                    self.engine.state.mode = Mode::Normal;
                }
            }
            ConfirmAction::ReloadDiscard => {
                if should_quit {
                    self.reload_file().await;
                }
            }
        }
    }

    fn handle_set_command(&mut self, cmd: &str) {
        let args = cmd.trim_start_matches("set ").trim();

        match args {
            "number" => {
                self.engine.state.show_line_numbers = true;
            }
            "nonumber" | "nonumber!" => {
                self.engine.state.show_line_numbers = false;
            }
            "number!" => {
                self.engine.state.show_line_numbers = !self.engine.state.show_line_numbers;
            }
            "relativenumber" => {
                self.engine.state.show_relative_numbers = true;
            }
            "norelativenumber" | "norelativenumber!" => {
                self.engine.state.show_relative_numbers = false;
            }
            "relativenumber!" => {
                self.engine.state.show_relative_numbers = !self.engine.state.show_relative_numbers;
            }
            "hlsearch" => {
                self.engine.state.hlsearch = true;
            }
            "nohlsearch" | "nohlsearch!" => {
                self.engine.state.hlsearch = false;
            }
            "hlsearch!" => {
                self.engine.state.hlsearch = !self.engine.state.hlsearch;
            }
            "expandtab" | "et" => {
                self.engine.state.expandtab = true;
            }
            "noexpandtab" | "noet" => {
                self.engine.state.expandtab = false;
            }
            "expandtab!" | "et!" => {
                self.engine.state.expandtab = !self.engine.state.expandtab;
            }
            args if args.starts_with("tabstop=") || args.starts_with("ts=") => {
                let val = args.split_once('=').map(|x| x.1).unwrap_or("8");
                if let Ok(n) = val.parse::<usize>() {
                    self.engine.state.tabstop = n;
                } else {
                    self.engine
                        .state
                        .push_message(MessageLevel::Warning, format!("Invalid value: {}", val));
                }
            }
            args if args.starts_with("shiftwidth=") || args.starts_with("sw=") => {
                let val = args.split_once('=').map(|x| x.1).unwrap_or("4");
                if let Ok(n) = val.parse::<usize>() {
                    self.engine.state.shiftwidth = n;
                } else {
                    self.engine
                        .state
                        .push_message(MessageLevel::Warning, format!("Invalid value: {}", val));
                }
            }
            args if args.starts_with("scrolloff=") || args.starts_with("so=") => {
                let val = args.split_once('=').map(|x| x.1).unwrap_or("0");
                if let Ok(n) = val.parse::<usize>() {
                    self.engine.state.scrolloff = n;
                } else {
                    self.engine
                        .state
                        .push_message(MessageLevel::Warning, format!("Invalid value: {}", val));
                }
            }
            _ => {
                self.engine.state.push_message(
                    MessageLevel::Warning,
                    format!("Unknown set option: {}", args),
                );
            }
        }
    }

    /// Handle `:s/pattern/replacement/[g]` and `:%s/pattern/replacement/[g]`.
    fn handle_substitute(&mut self, cmd: &str) {
        // Parse the delimiter (always `/` in our implementation).
        let (range, rest) = if let Some(rest) = cmd.strip_prefix("%s/") {
            ("%", rest)
        } else if let Some(rest) = cmd.strip_prefix("s/") {
            (".", rest)
        } else {
            self.engine.state.push_message(
                MessageLevel::Warning,
                "Invalid substitute command".to_string(),
            );
            return;
        };

        // Split into pattern / replacement / flags.
        let parts: Vec<&str> = rest.splitn(3, '/').collect();
        if parts.len() < 2 {
            self.engine.state.push_message(
                MessageLevel::Warning,
                "Invalid substitute syntax".to_string(),
            );
            return;
        }
        let pattern_str = parts[0];
        let replacement = parts[1];
        let flags = parts.get(2).copied().unwrap_or("");

        // Compile regex pattern.
        let re = match regex::Regex::new(pattern_str) {
            Ok(re) => re,
            Err(e) => {
                self.engine
                    .state
                    .push_message(MessageLevel::Warning, format!("Regex error: {}", e));
                return;
            }
        };

        let global = flags.contains('g');
        let line_count = self.engine.buffer.line_count();

        // Determine line range.
        let (start_line, end_line) = if range == "%" {
            (1, line_count)
        } else {
            let cur = self.engine.state.cursor.line;
            (cur, cur)
        };

        let mut replacements = 0;

        // We need to process lines in reverse order if substituting across multiple
        // lines, because inserts/deletes shift line indices.
        for line_idx in (start_line..=end_line).rev() {
            let line = self.engine.buffer.get_line(line_idx);
            let new_line = if global {
                re.replace_all(&line, replacement)
            } else {
                re.replace(&line, replacement)
            };
            if new_line != line {
                replacements += 1;
                let old_chars = line.chars().count();
                let mod_count = self.engine.buffer.modification_count();

                use crate::undo::{Edit, EditType};
                let old_trimmed = line.trim_end_matches('\n').to_string();
                let new_trimmed = new_line.trim_end_matches('\n').to_string();

                // Push a single Replace undo entry instead of Delete+Insert.
                self.engine.undo_manager.push(Edit {
                    edit_type: EditType::Replace {
                        line: line_idx,
                        col: 0,
                        old_text: old_trimmed.clone(),
                        new_text: new_trimmed.clone(),
                    },
                    cursor_before: self.engine.state.cursor,
                    cursor_after: self.engine.state.cursor,
                    modification_count: mod_count,
                });

                // Replace the line content.
                let char_start = self.engine.buffer.line_to_char(line_idx - 1);
                let char_end = char_start + old_chars;
                self.engine.buffer.remove_range(char_start, char_end);
                self.engine.buffer.insert(line_idx, 0, &new_trimmed);
            }
        }

        self.engine.state.dirty = self.engine.buffer.is_dirty();
        self.engine.state.push_message(
            MessageLevel::Info,
            format!(
                "{} substitutions on {} line{}",
                replacements,
                if range == "%" { "all" } else { "1" },
                if replacements == 1 { "" } else { "s" }
            ),
        );
        self.needs_render = true;
    }

    async fn reload_file(&mut self) {
        // Directory listings cannot be reloaded as text files — refresh instead.
        if self.engine.buffer.is_directory_listing() {
            self.refresh_directory();
            return;
        }
        if let Some(path) = &self.engine.state.file_path {
            match crate::buffer::TextBuffer::load_file(path.to_str().unwrap_or("")).await {
                Ok(buf) => {
                    self.engine.buffer = buf;
                }
                Err(e) => {
                    self.engine
                        .state
                        .push_message(MessageLevel::Error, format!("Failed to reload file: {}", e));
                }
            }
        }
    }

    async fn reload_file_discard(&mut self) {
        if self.engine.buffer.is_dirty() {
            self.engine.state.set_confirmation(
                "No write since last change. Reload anyway? (y/n)".to_string(),
                ConfirmAction::ReloadDiscard,
            );
            self.needs_render = true;
        } else {
            self.reload_file().await;
        }
    }

    /// Save the current buffer and emit a BufferSave plugin event.
    /// If the save fails, set an error message instead.
    async fn save_and_emit(&mut self) {
        match self.engine.buffer.save_file().await {
            Ok(()) => {
                self.engine
                    .plugin_manager
                    .emit(crate::types::PluginEvent::BufferSave {
                        file_path: self.engine.state.file_path.clone(),
                    });
            }
            Err(e) => {
                self.engine
                    .state
                    .push_message(MessageLevel::Error, format!("Save failed: {}", e));
            }
        }
    }

    async fn handle_write_path(&mut self, cmd: &str) {
        let path_str = cmd.trim_start_matches("w!").trim_start_matches("w ").trim();
        if path_str.is_empty() {
            self.engine
                .state
                .push_message(MessageLevel::Warning, "Expected filename after 'w'");
            return;
        }
        let path = std::path::PathBuf::from(path_str);
        match self.engine.buffer.save_to_path(&path).await {
            Ok(_) => {
                self.engine.state.file_path = Some(path);
                self.engine
                    .plugin_manager
                    .emit(crate::types::PluginEvent::BufferSave {
                        file_path: self.engine.state.file_path.clone(),
                    });
            }
            Err(e) => {
                self.engine
                    .state
                    .push_message(MessageLevel::Error, format!("Save failed: {}", e));
            }
        }
    }

    async fn handle_write_quit_path(&mut self, cmd: &str) {
        let path_str = cmd
            .trim_start_matches("wq!")
            .trim_start_matches("wq ")
            .trim();
        if path_str.is_empty() {
            self.engine
                .state
                .push_message(MessageLevel::Warning, "Expected filename after 'wq'");
            return;
        }
        let path = std::path::PathBuf::from(path_str);
        match self.engine.buffer.save_to_path(&path).await {
            Ok(_) => {
                self.engine.state.file_path = Some(path);
                self.engine
                    .plugin_manager
                    .emit(crate::types::PluginEvent::BufferSave {
                        file_path: self.engine.state.file_path.clone(),
                    });
                self.running = false;
            }
            Err(e) => {
                self.engine
                    .state
                    .push_message(MessageLevel::Error, format!("Save failed: {}", e));
            }
        }
    }

    /// Run a git command and show its output in messages.
    pub(crate) fn handle_git_command(&mut self, args: &[&str]) {
        let cmd_str = args.join(" ");
        match crate::git::run(args) {
            Ok(stdout) => {
                if stdout.is_empty() {
                    self.engine
                        .state
                        .push_message(MessageLevel::Info, format!("git {}: OK", cmd_str));
                } else {
                    self.engine
                        .state
                        .push_message(MessageLevel::Info, format!("git {}\n{}", cmd_str, stdout));
                    // For branch listing, also push a compact summary.
                    if stdout.lines().count() > 1 && args == ["branch"] {
                        let branches: Vec<&str> = stdout
                            .lines()
                            .map(|l| l.trim_start_matches("* ").trim())
                            .collect();
                        self.engine.state.push_message(
                            MessageLevel::Info,
                            format!("Branches: {}", branches.join(", ")),
                        );
                    }
                }
            }
            Err(stderr) => {
                self.engine.state.push_message(
                    MessageLevel::Error,
                    format!("git {} failed: {}", cmd_str, stderr),
                );
            }
        }
    }

    /// Reload the current file after a `git checkout` — file contents may have
    /// changed (or the file may no longer exist).
    fn reload_file_after_checkout(&mut self) {
        self.needs_render = true;
        let Some(path) = &self.engine.state.file_path.clone() else {
            return;
        };
        let path_str = path.to_string_lossy().to_string();
        if path.exists() {
            match std::fs::read_to_string(&path_str) {
                Ok(content) => {
                    self.engine.buffer = crate::buffer::TextBuffer::with_text(&content);
                    self.engine.state.file_path = Some(path.clone());
                    self.engine.state.dirty = false;
                    self.engine.state.push_message(
                        MessageLevel::Info,
                        format!("Reloaded after checkout: {}", path_str),
                    );
                }
                Err(e) => {
                    self.engine.state.push_message(
                        MessageLevel::Warning,
                        format!("Could not reload {}: {}", path_str, e),
                    );
                }
            }
        } else {
            self.engine.buffer = crate::buffer::TextBuffer::new();
            self.engine.state.file_path = None;
            self.engine.state.push_message(
                MessageLevel::Warning,
                format!("File not found in this branch: {}", path_str),
            );
        }
    }
}
