use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;

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
                self.search_next();
            }
            InputMode::Confirm(ConfirmAction::Delete) => {
                self.execute_delete();
            }
            InputMode::Normal => {}
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

    pub fn open_in_editor(&mut self) {
        if let Some(node) = self.tree.get_node(self.selected) {
            let path = node.path.clone();

            // Try $EDITOR, then common editors
            let editor = std::env::var("EDITOR")
                .ok()
                .or_else(|| std::env::var("VISUAL").ok())
                .unwrap_or_else(|| {
                    // Default editors by platform
                    if cfg!(target_os = "macos") {
                        "open".to_string()
                    } else if cfg!(target_os = "windows") {
                        "notepad".to_string()
                    } else {
                        "xdg-open".to_string()
                    }
                });

            let result = Command::new(&editor)
                .arg(&path)
                .spawn();

            match result {
                Ok(_) => {
                    self.message = Some(format!("Opened with {}", editor));
                }
                Err(e) => {
                    self.message = Some(format!("Failed to open: {}", e));
                }
            }
        }
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
}
