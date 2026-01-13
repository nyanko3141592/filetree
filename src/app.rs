use std::collections::HashSet;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use crate::file_ops::{self, Clipboard, ClipboardContent};
use crate::file_tree::FileTree;
use crate::git_status::GitRepo;

/// Image pixel data for terminal preview (RGB values)
#[derive(Clone)]
pub struct ImagePreview {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<(u8, u8, u8)>, // RGB values
}

#[derive(Debug, Clone, PartialEq)]
pub enum InputMode {
    Normal,
    Search,
    Rename,
    NewFile,
    NewDir,
    Confirm(ConfirmAction),
    Preview,
    ExternalCommand,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DeleteInfo {
    pub paths: Vec<PathBuf>,
    pub has_directories: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConfirmAction {
    Delete(DeleteInfo),
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
    // Preview mode state (full screen)
    pub preview_content: Vec<String>,
    pub preview_scroll: usize,
    pub preview_path: Option<PathBuf>,
    pub image_preview: Option<ImagePreview>,
    // Quick preview panel (bottom panel, Quick Look style)
    pub quick_preview_enabled: bool,
    pub quick_preview_content: Vec<String>,
    pub quick_preview_scroll: usize,
    pub quick_preview_path: Option<PathBuf>,
    pub quick_preview_image: Option<ImagePreview>,
    // Drop detection
    pub drop_buffer: String,
    pub last_char_time: std::time::Instant,
    // External command execution
    pub last_command: Option<String>,
    pub default_command: Option<String>,
    pub command_history: Vec<String>,
    pub history_index: Option<usize>,
}

impl App {
    fn get_history_file_path() -> Option<PathBuf> {
        let config_dir = if let Ok(xdg_config) = std::env::var("XDG_CONFIG_HOME") {
            PathBuf::from(xdg_config).join("filetree")
        } else if let Ok(home) = std::env::var("HOME") {
            PathBuf::from(home).join(".config").join("filetree")
        } else {
            return None;
        };
        Some(config_dir.join("history.txt"))
    }

    fn load_history() -> Vec<String> {
        let history_path = match Self::get_history_file_path() {
            Some(path) => path,
            None => return Vec::new(),
        };

        if !history_path.exists() {
            return Vec::new();
        }

        match fs::File::open(&history_path) {
            Ok(file) => {
                let reader = BufReader::new(file);
                reader
                    .lines()
                    .filter_map(|line| line.ok())
                    .filter(|line| !line.trim().is_empty())
                    .collect()
            }
            Err(_) => Vec::new(),
        }
    }

    fn save_history(&self) {
        let history_path = match Self::get_history_file_path() {
            Some(path) => path,
            None => return,
        };

        // Create directory if it doesn't exist
        if let Some(parent) = history_path.parent() {
            let _ = fs::create_dir_all(parent);
        }

        // Write history to file
        if let Ok(mut file) = fs::File::create(&history_path) {
            for cmd in &self.command_history {
                let _ = writeln!(file, "{}", cmd);
            }
        }
    }

    pub fn new(path: &Path, default_command: Option<String>) -> anyhow::Result<Self> {
        let show_hidden = false;
        let tree = FileTree::new(path, show_hidden)?;
        let git_repo = GitRepo::new(path);
        let command_history = Self::load_history();
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
            image_preview: None,
            quick_preview_enabled: false,
            quick_preview_content: Vec::new(),
            quick_preview_scroll: 0,
            quick_preview_path: None,
            quick_preview_image: None,
            drop_buffer: String::new(),
            last_char_time: std::time::Instant::now(),
            last_command: None,
            default_command,
            command_history,
            history_index: None,
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
        if let Some(idx) = (0..self.tree.len()).find(|&i| {
            self.tree
                .get_node(i)
                .map(|n| n.path == path)
                .unwrap_or(false)
        }) {
            self.selected = idx;
        }
    }

    pub fn toggle_mark(&mut self) {
        if let Some(node) = self.tree.get_node(self.selected) {
            let path = node.path.clone();
            if !self.marked.remove(&path) {
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
                node.path
                    .parent()
                    .map(|p| p.to_path_buf())
                    .unwrap_or_else(|| node.path.clone())
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

    pub fn confirm_delete(&mut self) {
        let paths = self.get_selected_paths();
        if !paths.is_empty() {
            let has_directories = paths.iter().any(|p| p.is_dir());
            let delete_info = DeleteInfo {
                paths,
                has_directories,
            };
            self.input_mode = InputMode::Confirm(ConfirmAction::Delete(delete_info));
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
            InputMode::ExternalCommand => {
                let command = self.input_buffer.clone();
                if !command.is_empty() {
                    // Remove duplicate from history if exists
                    self.command_history.retain(|c| c != &command);
                    // Add to end of history
                    self.command_history.push(command.clone());
                    // Save history to file
                    self.save_history();
                }
                self.execute_external_command(Some(command));
            }
            InputMode::Confirm(ConfirmAction::Delete(_)) => {
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

    fn format_hex_preview(bytes: &[u8], max_lines: usize) -> Vec<String> {
        bytes
            .chunks(16)
            .take(max_lines)
            .map(|chunk| {
                let hex: Vec<String> = chunk.iter().map(|b| format!("{:02x}", b)).collect();
                let ascii: String = chunk
                    .iter()
                    .map(|&b| {
                        if b.is_ascii_graphic() || b == b' ' {
                            b as char
                        } else {
                            '.'
                        }
                    })
                    .collect();
                format!("{:<48} {}", hex.join(" "), ascii)
            })
            .collect()
    }

    fn format_dir_preview(path: &Path) -> Vec<String> {
        let mut lines = vec!["[Directory]".to_string(), String::new()];

        if let Ok(entries) = std::fs::read_dir(path) {
            let mut files = 0;
            let mut dirs = 0;
            let mut hidden = 0;
            let mut total_size: u64 = 0;

            for entry in entries.filter_map(|e| e.ok()) {
                let name = entry.file_name();
                let is_hidden = name.to_str().map(|s| s.starts_with('.')).unwrap_or(false);

                if is_hidden {
                    hidden += 1;
                }

                if let Ok(meta) = entry.metadata() {
                    if meta.is_dir() {
                        dirs += 1;
                    } else {
                        files += 1;
                        total_size += meta.len();
                    }
                }
            }

            lines.push(format!("  Files: {}", files));
            lines.push(format!("  Directories: {}", dirs));
            if hidden > 0 {
                lines.push(format!("  Hidden: {}", hidden));
            }
            lines.push(format!("  Size: {}", Self::format_size(total_size)));
        }

        lines
    }

    fn format_size(bytes: u64) -> String {
        const KB: u64 = 1024;
        const MB: u64 = KB * 1024;
        const GB: u64 = MB * 1024;

        if bytes >= GB {
            format!("{:.1} GB", bytes as f64 / GB as f64)
        } else if bytes >= MB {
            format!("{:.1} MB", bytes as f64 / MB as f64)
        } else if bytes >= KB {
            format!("{:.1} KB", bytes as f64 / KB as f64)
        } else {
            format!("{} B", bytes)
        }
    }

    fn copy_to_system_clipboard(&mut self, text: &str) {
        match arboard::Clipboard::new() {
            Ok(mut clip) => {
                if clip.set_text(text).is_ok() {
                    self.message = Some(format!("Copied: {}", text));
                } else {
                    self.message = Some("Failed to copy to clipboard".to_string());
                }
            }
            Err(_) => {
                self.message = Some("Clipboard not available".to_string());
            }
        }
    }

    pub fn copy_path(&mut self) {
        if let Some(node) = self.tree.get_node(self.selected) {
            let path_str = node.path.to_string_lossy().to_string();
            self.copy_to_system_clipboard(&path_str);
        }
    }

    pub fn copy_filename(&mut self) {
        if let Some(node) = self.tree.get_node(self.selected) {
            let name = node.name.clone();
            self.copy_to_system_clipboard(&name);
        }
    }

    pub fn preview_file(&mut self) {
        if let Some(node) = self.tree.get_node(self.selected) {
            if node.is_dir {
                self.message = Some("Cannot preview directory".to_string());
                return;
            }

            let path = node.path.clone();

            // Check if it's an image file
            if Self::is_image_file(&path) {
                match self.load_image_preview(&path) {
                    Ok(()) => return,
                    Err(e) => {
                        self.message = Some(format!("Image error: {}", e));
                        // Fall through to binary preview
                    }
                }
            }

            match std::fs::read_to_string(&path) {
                Ok(content) => {
                    self.preview_content = content.lines().map(|s| s.to_string()).collect();
                    self.preview_scroll = 0;
                    self.preview_path = Some(path);
                    self.image_preview = None;
                    self.input_mode = InputMode::Preview;
                }
                Err(e) => {
                    // Try to read as binary and show hex preview
                    if let Ok(bytes) = std::fs::read(&path) {
                        self.preview_content = Self::format_hex_preview(&bytes, 100);
                        self.preview_scroll = 0;
                        self.preview_path = Some(path);
                        self.image_preview = None;
                        self.input_mode = InputMode::Preview;
                    } else {
                        self.message = Some(format!("Cannot read file: {}", e));
                    }
                }
            }
        }
    }

    fn is_image_file(path: &Path) -> bool {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase());
        matches!(
            ext.as_deref(),
            Some("png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp")
        )
    }

    fn load_image_preview(&mut self, path: &Path) -> Result<(), String> {
        let img = image::open(path).map_err(|e| e.to_string())?;
        let img = img.to_rgb8();
        let (width, height) = img.dimensions();
        let pixels: Vec<(u8, u8, u8)> = img.pixels().map(|p| (p[0], p[1], p[2])).collect();

        self.image_preview = Some(ImagePreview {
            width,
            height,
            pixels,
        });
        self.preview_path = Some(path.to_path_buf());
        self.preview_content.clear();
        self.preview_scroll = 0;
        self.input_mode = InputMode::Preview;
        Ok(())
    }

    pub fn close_preview(&mut self) {
        self.input_mode = InputMode::Normal;
        self.preview_content.clear();
        self.preview_path = None;
        self.preview_scroll = 0;
        self.image_preview = None;
    }

    pub fn toggle_quick_preview(&mut self) {
        self.quick_preview_enabled = !self.quick_preview_enabled;
        if self.quick_preview_enabled {
            self.update_quick_preview();
        } else {
            self.quick_preview_content.clear();
            self.quick_preview_path = None;
            self.quick_preview_scroll = 0;
            self.quick_preview_image = None;
        }
    }

    pub fn update_quick_preview(&mut self) {
        if !self.quick_preview_enabled {
            return;
        }

        let node = match self.tree.get_node(self.selected) {
            Some(n) => n,
            None => return,
        };

        if node.is_dir {
            self.quick_preview_content = Self::format_dir_preview(&node.path);
            self.quick_preview_path = Some(node.path.clone());
            self.quick_preview_scroll = 0;
            self.quick_preview_image = None;
            return;
        }

        let path = node.path.clone();

        // Check if it's the same file
        if self.quick_preview_path.as_ref() == Some(&path) {
            return;
        }

        // Check if it's an image file
        if Self::is_image_file(&path) {
            if let Ok(img) = image::open(&path) {
                let img = img.to_rgb8();
                let (width, height) = img.dimensions();
                let pixels: Vec<(u8, u8, u8)> = img.pixels().map(|p| (p[0], p[1], p[2])).collect();

                self.quick_preview_image = Some(ImagePreview {
                    width,
                    height,
                    pixels,
                });
                self.quick_preview_content.clear();
                self.quick_preview_path = Some(path);
                self.quick_preview_scroll = 0;
                return;
            }
        }

        // Try to read as text
        self.quick_preview_image = None;
        match std::fs::read_to_string(&path) {
            Ok(content) => {
                self.quick_preview_content = content.lines().map(|s| s.to_string()).collect();
            }
            Err(_) => {
                // Try to read as binary and show hex preview
                if let Ok(bytes) = std::fs::read(&path) {
                    self.quick_preview_content = Self::format_hex_preview(&bytes, 50);
                } else {
                    self.quick_preview_content = vec!["[Cannot read file]".to_string()];
                }
            }
        }
        self.quick_preview_path = Some(path);
        self.quick_preview_scroll = 0;
    }

    #[allow(dead_code)]
    pub fn quick_preview_scroll_up(&mut self) {
        if self.quick_preview_scroll > 0 {
            self.quick_preview_scroll -= 1;
        }
    }

    #[allow(dead_code)]
    pub fn quick_preview_scroll_down(&mut self, visible_height: usize) {
        if self.quick_preview_scroll + visible_height < self.quick_preview_content.len() {
            self.quick_preview_scroll += 1;
        }
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

        // Normalize the path: remove quotes and unescape backslashes
        let normalized = Self::normalize_dropped_path(&text);

        // Check if it's an absolute path that exists
        if normalized.starts_with('/') {
            let path = PathBuf::from(&normalized);
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
        if let Some(rest) = text.strip_prefix('/') {
            // Start search with remaining chars
            self.input_buffer = rest.to_string();
            self.input_mode = InputMode::Search;
        }
    }

    /// Normalize a dropped path by removing quotes and unescaping backslashes
    fn normalize_dropped_path(text: &str) -> String {
        let text = text.trim();

        // Remove surrounding quotes if present
        let text = if (text.starts_with('\'') && text.ends_with('\''))
            || (text.starts_with('"') && text.ends_with('"'))
        {
            &text[1..text.len() - 1]
        } else {
            text
        };

        // Unescape backslash-escaped characters (e.g., "\ " -> " ")
        let mut result = String::with_capacity(text.len());
        let mut chars = text.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '\\' {
                if let Some(&next) = chars.peek() {
                    // Common escaped characters in shell paths
                    if matches!(
                        next,
                        ' ' | '\''
                            | '"'
                            | '\\'
                            | '('
                            | ')'
                            | '['
                            | ']'
                            | '&'
                            | ';'
                            | '!'
                            | '$'
                            | '`'
                    ) {
                        result.push(chars.next().unwrap());
                        continue;
                    }
                }
            }
            result.push(c);
        }
        result
    }

    fn try_handle_as_drop(&mut self) -> bool {
        let text = self.input_buffer.trim();
        // Normalize the path (remove quotes, unescape)
        let normalized = Self::normalize_dropped_path(text);

        // Check if it looks like an absolute path
        if !normalized.starts_with('/') {
            return false;
        }

        // Try as single path first
        let path = PathBuf::from(&normalized);
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
                    self.message = Some(format!(
                        "Dropped: {}",
                        path.file_name().unwrap_or_default().to_string_lossy()
                    ));
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
            if path.exists() && file_ops::copy_file(path, &dest_dir).is_ok() {
                success += 1;
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
                let normalized = Self::normalize_dropped_path(line);
                if !normalized.is_empty() {
                    let path = PathBuf::from(&normalized);
                    if path.is_absolute() && path.exists() {
                        paths.push(path);
                    }
                }
            }
            return paths;
        }

        // Single path or space-separated paths
        // Handle quoted paths and escaped spaces
        let mut chars = text.chars().peekable();
        let mut current = String::new();
        let mut in_quote = false;
        let mut quote_char: Option<char> = None;

        while let Some(c) = chars.next() {
            match c {
                '"' | '\'' => {
                    if in_quote && Some(c) == quote_char {
                        in_quote = false;
                        quote_char = None;
                    } else if !in_quote {
                        in_quote = true;
                        quote_char = Some(c);
                    } else {
                        // Different quote inside quoted string
                        current.push(c);
                    }
                }
                '\\' if !in_quote => {
                    // Handle escaped characters outside quotes
                    if let Some(next) = chars.next() {
                        current.push(next);
                    }
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

    pub fn execute_external_command(&mut self, command_override: Option<String>) {
        // Determine which command to use
        let command_template = command_override
            .as_ref()
            .or(self.last_command.as_ref())
            .or(self.default_command.as_ref());

        let command_template = match command_template {
            Some(cmd) => cmd,
            None => {
                self.message = Some("No command available. Enter a command first.".to_string());
                return;
            }
        };

        // Get the selected file path
        let filepath = match self.tree.get_node(self.selected) {
            Some(node) => node.path.to_string_lossy().to_string(),
            None => {
                self.message = Some("No file selected".to_string());
                return;
            }
        };

        // Replace <filepath> placeholder with actual path (quoted)
        let command = command_template.replace("<filepath>", &format!("'{}'", filepath));

        // Execute the command with stdout/stderr redirected to null to prevent terminal corruption
        match std::process::Command::new("sh")
            .arg("-c")
            .arg(&command)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
        {
            Ok(_) => {
                self.message = Some(format!("Executed: {}", command));
                // Save the command for next time
                if let Some(cmd) = command_override {
                    self.last_command = Some(cmd);
                }
            }
            Err(e) => {
                self.message = Some(format!("Command failed: {}", e));
            }
        }
    }

    pub fn start_external_command(&mut self) {
        self.input_buffer.clear();
        self.history_index = None;
        self.input_mode = InputMode::ExternalCommand;
    }

    pub fn history_prev(&mut self) {
        if self.command_history.is_empty() {
            return;
        }

        let new_index = match self.history_index {
            None => Some(self.command_history.len() - 1),
            Some(0) => Some(0), // Already at oldest
            Some(i) => Some(i - 1),
        };

        if let Some(idx) = new_index {
            self.input_buffer = self.command_history[idx].clone();
            self.history_index = new_index;
        }
    }

    pub fn history_next(&mut self) {
        if self.command_history.is_empty() {
            return;
        }

        let new_index = match self.history_index {
            None => None,
            Some(i) if i + 1 >= self.command_history.len() => {
                // Back to empty input
                self.input_buffer.clear();
                None
            }
            Some(i) => Some(i + 1),
        };

        if let Some(idx) = new_index {
            self.input_buffer = self.command_history[idx].clone();
        }
        self.history_index = new_index;
    }
}
