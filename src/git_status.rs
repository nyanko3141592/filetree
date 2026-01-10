use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GitStatus {
    #[default]
    None,
    Modified,
    Added,
    Deleted,
    Renamed,
    Untracked,
    Ignored,
    Conflict,
}

#[derive(Debug, Default)]
pub struct GitRepo {
    pub root: Option<PathBuf>,
    pub statuses: HashMap<PathBuf, GitStatus>,
    pub branch: Option<String>,
}

impl GitRepo {
    pub fn new(path: &Path) -> Self {
        let mut repo = Self::default();
        repo.refresh(path);
        repo
    }

    pub fn refresh(&mut self, path: &Path) {
        self.root = find_git_root(path);
        self.statuses.clear();
        self.branch = None;

        if let Some(root) = self.root.clone() {
            self.load_statuses(&root);
            self.branch = get_current_branch(&root);
        }
    }

    fn load_statuses(&mut self, root: &Path) {
        // Get modified/staged/untracked files
        if let Ok(output) = Command::new("git")
            .args(["status", "--porcelain", "-uall"])
            .current_dir(root)
            .output()
        {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                for line in stdout.lines() {
                    if line.len() < 4 {
                        continue;
                    }
                    let status_chars: Vec<char> = line.chars().take(2).collect();
                    let file_path = &line[3..];

                    // Handle renamed files (R  old -> new)
                    let file_path = if file_path.contains(" -> ") {
                        file_path.split(" -> ").last().unwrap_or(file_path)
                    } else {
                        file_path
                    };

                    let full_path = root.join(file_path);
                    let status = parse_status(status_chars[0], status_chars[1]);
                    self.statuses.insert(full_path, status);
                }
            }
        }

        // Get ignored files
        if let Ok(output) = Command::new("git")
            .args(["status", "--porcelain", "--ignored", "-uall"])
            .current_dir(root)
            .output()
        {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                for line in stdout.lines() {
                    if line.starts_with("!! ") {
                        let file_path = &line[3..];
                        let full_path = root.join(file_path);
                        self.statuses.insert(full_path, GitStatus::Ignored);
                    }
                }
            }
        }
    }

    pub fn get_status(&self, path: &Path) -> GitStatus {
        // Direct match
        if let Some(&status) = self.statuses.get(path) {
            return status;
        }

        // For directories, check if any child has a status
        if path.is_dir() {
            let mut has_modified = false;
            let mut has_untracked = false;

            for (file_path, status) in &self.statuses {
                if file_path.starts_with(path) {
                    match status {
                        GitStatus::Modified | GitStatus::Added | GitStatus::Deleted |
                        GitStatus::Renamed | GitStatus::Conflict => {
                            has_modified = true;
                        }
                        GitStatus::Untracked => {
                            has_untracked = true;
                        }
                        _ => {}
                    }
                }
            }

            if has_modified {
                return GitStatus::Modified;
            }
            if has_untracked {
                return GitStatus::Untracked;
            }
        }

        GitStatus::None
    }

    pub fn is_inside_repo(&self) -> bool {
        self.root.is_some()
    }
}

fn find_git_root(path: &Path) -> Option<PathBuf> {
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(path)
        .output()
        .ok()?;

    if output.status.success() {
        let root = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Some(PathBuf::from(root))
    } else {
        None
    }
}

fn parse_status(index: char, worktree: char) -> GitStatus {
    match (index, worktree) {
        ('?', '?') => GitStatus::Untracked,
        ('!', '!') => GitStatus::Ignored,
        ('U', _) | (_, 'U') | ('A', 'A') | ('D', 'D') => GitStatus::Conflict,
        ('R', _) => GitStatus::Renamed,
        ('A', _) => GitStatus::Added,
        ('D', _) | (_, 'D') => GitStatus::Deleted,
        ('M', _) | (_, 'M') => GitStatus::Modified,
        _ => GitStatus::None,
    }
}

fn get_current_branch(root: &Path) -> Option<String> {
    let output = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(root)
        .output()
        .ok()?;

    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        None
    }
}
