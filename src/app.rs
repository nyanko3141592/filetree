use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::file_ops::{self, Clipboard, ClipboardContent};
use crate::file_tree::FileTree;
use crate::git_status::GitRepo;

#[derive(Debug, Clone, PartialEq)]
pub enum InputMode {
    Normal,
    Search,
    Rename,
    NewFile,
    NewDir,
    Confirm(ConfirmAction),
    Preview,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConfirmAction {
    Delete,
}

pub struct App {
    pub tree: FileTree,
    pub git_repo: GitRepo,
    pub selected: usize,
    pub marked: HashSet<PathBuf>,
    pub clipboard: Clipboard,
    pub input_mode: InputMode,
    pub input_buffer: String,
    pub message: Option<String>,
    pub should_quit: bool,
    pub scroll_offset: usize,
    pub tree_area_height: usize,
    pub last_click_time: std::time::Instant,
    pub last_click_index: Option<usize>,
    pub show_hidden: bool,
    // Preview mode state
    pub preview_content: Vec<String>,
    pub preview_scroll: usize,
    pub preview_path: Option<PathBuf>,
    // Drop detection
    pub drop_buffer: String,
    pub last_char_time: std::time::Instant,
}

impl App {
    pub fn new(path: &Path) -> anyhow::Result<Self> {
        let show_hidden = false;
        let tree = FileTree::new(path, show_hidden)?;
        let git_repo = GitRepo::new(path);
        Ok(Self {
            tree,
            git_repo,
            selected: 0,
            marked: HashSet::new(),
            clipboard: Clipboard::default(),
            input_mode: InputMode::Normal,
            input_buffer: String::new(),
            message: None,
            should_quit: false,
            scroll_offset: 0,
            tree_area_height: 20,
            last_click_time: std::time::Instant::now(),
            last_click_index: None,
            show_hidden,
            preview_content: Vec::new(),
            preview_scroll: 0,
            preview_path: None,
            drop_buffer: String::new(),
            last_char_time: std::time::Instant::now(),
        })
    }

    pub fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    pub fn move_down(&mut self) {
        if self.selected < self.tree.len().saturating_sub(1) {
            self.selected += 1;
        }
    }

    pub fn move_to_top(&mut self) {
        self.selected = 0;
    }

    pub fn move_to_bottom(&mut self) {
        self.selected = self.tree.len().saturating_sub(1);
    }

    pub fn toggle_expand(&mut self) {
        if let Some(node) = self.tree.get_node(self.selected) {
            if node.is_dir {
                let path = node.path.clone();
                if node.expanded {
                    let _ = self.tree.collapse_node(self.selected);
                } else {
                    let _ = self.tree.expand_node(self.selected);
                }
                // Restore selection to the same path
                self.select_path(&path);
            }
        }
    }

    pub fn expand_current(&mut self) {
        if let Some(node) = self.tree.get_node(self.selected) {
            if node.is_dir && !node.expanded {
                let path = node.path.clone();
                let _ = self.tree.expand_node(self.selected);
                self.select_path(&path);
            }
        }
    }

    pub fn collapse_current(&mut self) {
        if let Some(node) = self.tree.get_node(self.selected) {
            if node.is_dir && node.expanded {
                let path = node.path.clone();
                let _ = self.tree.collapse_node(self.selected);
                self.select_path(&path);
            } else if let Some(parent) = node.path.parent() {
                // Go to parent directory
                let parent = parent.to_path_buf();
                self.select_path(&parent);
            }
        }
    }

    fn select_path(&mut self, path: &Path) {
        for (i, _) in self.tree.flat_list.iter().enumerate() {
            if let Some(node) = self.tree.get_node(i) {
                if node.path == path {
                    self.selected = i;
                    return;
                }
            }
        }
    }

    pub fn toggle_mark(&mut self) {
        if let Some(node) = self.tree.get_node(self.selected) {
            let path = node.path.clone();
            if self.marked.contains(&path) {
                self.marked.remove(&path);
            } else {
                self.marked.insert(path);
            }
        }
        self.move_down();
    }

    pub fn clear_marks(&mut self) {
        self.marked.clear();
    }

    pub fn yank(&mut self) {
        let paths = self.get_selected_paths();
        if !paths.is_empty() {
            self.clipboard.copy(paths.clone());
            self.message = Some(format!("Copied {} item(s)", paths.len()));
            self.clear_marks();
        }
    }

    pub fn cut(&mut self) {
        let paths = self.get_selected_paths();
        if !paths.is_empty() {
            self.clipboard.cut(paths.clone());
            self.message = Some(format!("Cut {} item(s)", paths.len()));
        }
    }

    pub fn paste(&mut self) {
        let dest_dir = self.get_paste_destination();
        if let Some(dest_dir) = dest_dir {
            if let Some(content) = self.clipboard.content.take() {
                let count = match content {
                    ClipboardContent::Copy(paths) => {
                        let mut success = 0;
                        for path in &paths {
                            if file_ops::copy_file(path, &dest_dir).is_ok() {
                                success += 1;
                            }
                        }
                        self.clipboard.copy(paths);
                        success
                    }
                    ClipboardContent::Cut(paths) => {
                        let mut success = 0;
                        for path in &paths {
                            if file_ops::move_file(path, &dest_dir).is_ok() {
                                success += 1;
                            }
                        }
                        self.clear_marks();
                        success
                    }
                };

                self.message = Some(format!("Pasted {} item(s)", count));
                let _ = self.tree.refresh();
            }
        }
    }

    fn get_paste_destination(&self) -> Option<PathBuf> {
        self.tree.get_node(self.selected).map(|node| {
            if node.is_dir {
                node.path.clone()
            } else {
                node.path.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| node.path.clone())
            }
        })
    }

    fn get_selected_paths(&self) -> Vec<PathBuf> {
        if self.marked.is_empty() {
            if let Some(node) = self.tree.get_node(self.selected) {
                return vec![node.path.clone()];
            }
            vec![]
        } else {
            self.marked.iter().cloned().collect()
        }
    }

    pub fn start_rename(&mut self) {
        if let Some(node) = self.tree.get_node(self.selected) {
            self.input_buffer = node.name.clone();
            self.input_mode = InputMode::Rename;
        }
    }

    pub fn start_new_file(&mut self) {
        self.input_buffer.clear();
        self.input_mode = InputMode::NewFile;
    }

    pub fn start_new_dir(&mut self) {
        self.input_buffer.clear();
        self.input_mode = InputMode::NewDir;
    }

    pub fn start_search(&mut self) {
        self.input_buffer.clear();
        self.input_mode = InputMode::Search;
    }

    pub fn confirm_delete(&mut self) {
        let paths = self.get_selected_paths();
        if !paths.is_empty() {
            self.input_mode = InputMode::Confirm(ConfirmAction::Delete);
        }
    }

    pub fn execute_delete(&mut self) {
        let paths = self.get_selected_paths();
        let mut success = 0;
        for path in &paths {
            if file_ops::delete_file(path).is_ok() {
                success += 1;
            }
        }
        self.message = Some(format!("Deleted {} item(s)", success));
        self.clear_marks();
        let _ = self.tree.refresh();
        if self.selected >= self.tree.len() {
            self.selected = self.tree.len().saturating_sub(1);
        }
    }

    pub fn confirm_input(&mut self) {
        match &self.input_mode {
            InputMode::Rename => {
                if let Some(node) = self.tree.get_node(self.selected) {
                    let path = node.path.clone();
                    match file_ops::rename_file(&path, &self.input_buffer) {
                        Ok(new_path) => {
                            self.message = Some(format!("Renamed to {}", new_path.display()));
                            let _ = self.tree.refresh();
                            self.select_path(&new_path);
                        }
                        Err(e) => {
                            self.message = Some(format!("Error: {}", e));
                        }
                    }
                }
            }
            InputMode::NewFile => {
                if let Some(dest_dir) = self.get_paste_destination() {
                    match file_ops::create_file(&dest_dir, &self.input_buffer) {
                        Ok(new_path) => {
                            self.message = Some(format!("Created {}", new_path.display()));
                            let _ = self.tree.refresh();
                            self.select_path(&new_path);
                        }
                        Err(e) => {
                            self.message = Some(format!("Error: {}", e));
                        }
                    }
                }
            }
            InputMode::NewDir => {
                if let Some(dest_dir) = self.get_paste_destination() {
                    match file_ops::create_directory(&dest_dir, &self.input_buffer) {
                        Ok(new_path) => {
                            self.message = Some(format!("Created {}", new_path.display()));
                            let _ = self.tree.refresh();
                            self.select_path(&new_path);
                        }
                        Err(e) => {
                            self.message = Some(format!("Error: {}", e));
                        }
                    }
                }
            }
            InputMode::Search => {
                // Check if input looks like a dropped file path
                if self.try_handle_as_drop() {
                    self.input_mode = InputMode::Normal;
                    self.input_buffer.clear();
                    return;
                }
                self.search_next();
            }
            InputMode::Confirm(ConfirmAction::Delete) => {
                self.execute_delete();
            }
            InputMode::Normal | InputMode::Preview => {}
        }
        self.input_mode = InputMode::Normal;
        self.input_buffer.clear();
    }

    pub fn cancel_input(&mut self) {
        self.input_mode = InputMode::Normal;
        self.input_buffer.clear();
    }

    pub fn search_next(&mut self) {
        let query = self.input_buffer.to_lowercase();
        if query.is_empty() {
            return;
        }

        let start = self.selected + 1;
        let len = self.tree.len();

        for i in 0..len {
            let idx = (start + i) % len;
            if let Some(node) = self.tree.get_node(idx) {
                if node.name.to_lowercase().contains(&query) {
                    self.selected = idx;
                    return;
                }
            }
        }
        self.message = Some("No match found".to_string());
    }

    pub fn adjust_scroll(&mut self, visible_height: usize) {
        if self.selected < self.scroll_offset {
            self.scroll_offset = self.selected;
        } else if self.selected >= self.scroll_offset + visible_height {
            self.scroll_offset = self.selected - visible_height + 1;
        }
    }

    pub fn refresh(&mut self) {
        if let Err(e) = self.tree.refresh() {
            self.message = Some(format!("Refresh error: {}", e));
        } else {
            self.message = Some("Refreshed".to_string());
        }
        self.git_repo.refresh(&self.tree.root.path);
        if self.selected >= self.tree.len() {
            self.selected = self.tree.len().saturating_sub(1);
        }
    }

    pub fn toggle_hidden(&mut self) {
        self.show_hidden = !self.show_hidden;
        if let Err(e) = self.tree.set_show_hidden(self.show_hidden) {
            self.message = Some(format!("Error: {}", e));
        } else {
            self.message = Some(if self.show_hidden {
                "Showing hidden files".to_string()
            } else {
                "Hiding hidden files".to_string()
            });
        }
        if self.selected >= self.tree.len() {
            self.selected = self.tree.len().saturating_sub(1);
        }
    }

    pub fn collapse_all(&mut self) {
        self.tree.collapse_all();
        self.selected = 0;
        self.scroll_offset = 0;
        self.message = Some("Collapsed all".to_string());
    }

    pub fn expand_all(&mut self) {
        if let Err(e) = self.tree.expand_all() {
            self.message = Some(format!("Error: {}", e));
        } else {
            self.message = Some("Expanded all".to_string());
        }
    }

    pub fn copy_path(&mut self) {
        if let Some(node) = self.tree.get_node(self.selected) {
            let path_str = node.path.to_string_lossy().to_string();
            match arboard::Clipboard::new() {
                Ok(mut clip) => {
                    if clip.set_text(&path_str).is_ok() {
                        self.message = Some(format!("Copied: {}", path_str));
                    } else {
                        self.message = Some("Failed to copy to clipboard".to_string());
                    }
                }
                Err(_) => {
                    self.message = Some("Clipboard not available".to_string());
                }
            }
        }
    }

    pub fn copy_filename(&mut self) {
        if let Some(node) = self.tree.get_node(self.selected) {
            match arboard::Clipboard::new() {
                Ok(mut clip) => {
                    if clip.set_text(&node.name).is_ok() {
                        self.message = Some(format!("Copied: {}", node.name));
                    } else {
                        self.message = Some("Failed to copy to clipboard".to_string());
                    }
                }
                Err(_) => {
                    self.message = Some("Clipboard not available".to_string());
                }
            }
        }
    }

    pub fn preview_file(&mut self) {
        if let Some(node) = self.tree.get_node(self.selected) {
            if node.is_dir {
                self.message = Some("Cannot preview directory".to_string());
                return;
            }

            let path = node.path.clone();
            match std::fs::read_to_string(&path) {
                Ok(content) => {
                    self.preview_content = content.lines().map(|s| s.to_string()).collect();
                    self.preview_scroll = 0;
                    self.preview_path = Some(path);
                    self.input_mode = InputMode::Preview;
                }
                Err(e) => {
                    // Try to read as binary and show hex preview
                    if let Ok(bytes) = std::fs::read(&path) {
                        let preview: Vec<String> = bytes
                            .chunks(16)
                            .take(100)
                            .map(|chunk| {
                                let hex: Vec<String> = chunk.iter().map(|b| format!("{:02x}", b)).collect();
                                let ascii: String = chunk.iter()
                                    .map(|&b| if b.is_ascii_graphic() || b == b' ' { b as char } else { '.' })
                                    .collect();
                                format!("{:<48} {}", hex.join(" "), ascii)
                            })
                            .collect();
                        self.preview_content = preview;
                        self.preview_scroll = 0;
                        self.preview_path = Some(path);
                        self.input_mode = InputMode::Preview;
                    } else {
                        self.message = Some(format!("Cannot read file: {}", e));
                    }
                }
            }
        }
    }

    pub fn close_preview(&mut self) {
        self.input_mode = InputMode::Normal;
        self.preview_content.clear();
        self.preview_path = None;
        self.preview_scroll = 0;
    }

    pub fn preview_scroll_up(&mut self) {
        if self.preview_scroll > 0 {
            self.preview_scroll -= 1;
        }
    }

    pub fn preview_scroll_down(&mut self, visible_height: usize) {
        if self.preview_scroll + visible_height < self.preview_content.len() {
            self.preview_scroll += 1;
        }
    }

    pub fn preview_page_up(&mut self, visible_height: usize) {
        self.preview_scroll = self.preview_scroll.saturating_sub(visible_height);
    }

    pub fn preview_page_down(&mut self, visible_height: usize) {
        let max_scroll = self.preview_content.len().saturating_sub(visible_height);
        self.preview_scroll = (self.preview_scroll + visible_height).min(max_scroll);
    }

    pub fn select_by_row(&mut self, row: u16) {
        let index = self.scroll_offset + row as usize;
        if index < self.tree.len() {
            self.selected = index;
        }
    }

    pub fn handle_click(&mut self, row: u16) {
        let index = self.scroll_offset + row as usize;
        if index >= self.tree.len() {
            return;
        }

        let now = std::time::Instant::now();
        let is_double_click = self.last_click_index == Some(index)
            && now.duration_since(self.last_click_time).as_millis() < 400;

        self.selected = index;
        self.last_click_time = now;
        self.last_click_index = Some(index);

        if is_double_click {
            self.toggle_expand();
        }
    }

    pub fn scroll_up(&mut self, lines: usize) {
        for _ in 0..lines {
            self.move_up();
        }
    }

    pub fn scroll_down(&mut self, lines: usize) {
        for _ in 0..lines {
            self.move_down();
        }
    }

    pub fn buffer_char(&mut self, c: char) {
        let now = std::time::Instant::now();
        let elapsed = now.duration_since(self.last_char_time).as_millis();
        // If more than 50ms since last char, start new buffer
        if elapsed > 50 {
            self.drop_buffer.clear();
        }
        self.drop_buffer.push(c);
        self.last_char_time = now;
    }

    pub fn check_drop_buffer(&mut self) {
        if self.drop_buffer.is_empty() {
            return;
        }

        let elapsed = std::time::Instant::now()
            .duration_since(self.last_char_time)
            .as_millis();

        // Wait for input to stop (100ms)
        if elapsed < 100 {
            return;
        }

        let text = self.drop_buffer.trim().to_string();
        self.drop_buffer.clear();

        // Check if it's an absolute path that exists
        if text.starts_with('/') {
            let path = PathBuf::from(&text);
            if path.exists() {
                if let Some(dest_dir) = self.get_paste_destination() {
                    match file_ops::copy_file(&path, &dest_dir) {
                        Ok(_) => {
                            self.message = Some(format!(
                                "Dropped: {}",
                                path.file_name().unwrap_or_default().to_string_lossy()
                            ));
                            let _ = self.tree.refresh();
                        }
                        Err(e) => {
                            self.message = Some(format!("Copy error: {}", e));
                        }
                    }
                }
                return;
            }
        }

        // Not a valid path, treat first char as command
        if let Some(first) = text.chars().next() {
            match first {
                '/' => {
                    // Start search with remaining chars
                    self.input_buffer = text[1..].to_string();
                    self.input_mode = InputMode::Search;
                }
                _ => {}
            }
        }
    }

    fn try_handle_as_drop(&mut self) -> bool {
        let text = self.input_buffer.trim();
        // Check if it looks like an absolute path
        if !text.starts_with('/') {
            return false;
        }

        // Try as single path first
        let path = PathBuf::from(text);
        if path.exists() {
            let dest_dir = match self.get_paste_destination() {
                Some(dir) => dir,
                None => {
                    self.message = Some("No destination".to_string());
                    return false;
                }
            };

            match file_ops::copy_file(&path, &dest_dir) {
                Ok(_) => {
                    self.message = Some(format!("Dropped: {}", path.file_name().unwrap_or_default().to_string_lossy()));
                    let _ = self.tree.refresh();
                    return true;
                }
                Err(e) => {
                    self.message = Some(format!("Copy error: {}", e));
                    return false;
                }
            }
        }

        // Try parsing multiple paths
        let paths = Self::parse_dropped_paths(text);
        if paths.is_empty() {
            return false;
        }

        let dest_dir = match self.get_paste_destination() {
            Some(dir) => dir,
            None => return false,
        };

        let mut success = 0;
        for path in &paths {
            if file_ops::copy_file(path, &dest_dir).is_ok() {
                success += 1;
            }
        }

        if success > 0 {
            self.message = Some(format!("Dropped {} item(s)", success));
            let _ = self.tree.refresh();
            true
        } else {
            false
        }
    }

    pub fn handle_drop(&mut self, text: &str) {
        // Parse dropped text as file paths
        // Paths can be separated by newlines or spaces (with quotes for paths containing spaces)
        let paths: Vec<PathBuf> = Self::parse_dropped_paths(text);

        if paths.is_empty() {
            return;
        }

        // Get destination directory
        let dest_dir = match self.get_paste_destination() {
            Some(dir) => dir,
            None => return,
        };

        let mut success = 0;
        for path in &paths {
            if path.exists() {
                if file_ops::copy_file(path, &dest_dir).is_ok() {
                    success += 1;
                }
            }
        }

        if success > 0 {
            self.message = Some(format!("Dropped {} item(s)", success));
            let _ = self.tree.refresh();
        }
    }

    fn parse_dropped_paths(text: &str) -> Vec<PathBuf> {
        let mut paths = Vec::new();
        let text = text.trim();

        // Try newline-separated first
        if text.contains('\n') {
            for line in text.lines() {
                let line = line.trim();
                if !line.is_empty() {
                    let path = PathBuf::from(line);
                    if path.is_absolute() && path.exists() {
                        paths.push(path);
                    }
                }
            }
            return paths;
        }

        // Single path or space-separated paths
        // Handle quoted paths
        let mut chars = text.chars().peekable();
        let mut current = String::new();
        let mut in_quote = false;

        while let Some(c) = chars.next() {
            match c {
                '"' | '\'' => {
                    in_quote = !in_quote;
                }
                ' ' if !in_quote => {
                    if !current.is_empty() {
                        let path = PathBuf::from(&current);
                        if path.is_absolute() && path.exists() {
                            paths.push(path);
                        }
                        current.clear();
                    }
                }
                _ => {
                    current.push(c);
                }
            }
        }

        if !current.is_empty() {
            let path = PathBuf::from(&current);
            if path.is_absolute() && path.exists() {
                paths.push(path);
            }
        }

        paths
    }
}
