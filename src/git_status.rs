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
    pub dir_status_cache: HashMap<PathBuf, GitStatus>,
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
        self.dir_status_cache.clear();
        self.branch = None;

        if let Some(root) = self.root.clone() {
            self.load_statuses(&root);
            self.build_directory_cache();
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
                    if let Some(file_path) = line.strip_prefix("!! ") {
                        let full_path = root.join(file_path);
                        self.statuses.insert(full_path, GitStatus::Ignored);
                    }
                }
            }
        }
    }

    fn build_directory_cache(&mut self) {
        // Build a set of all directories that contain changed files
        let mut dir_statuses: HashMap<PathBuf, (bool, bool)> = HashMap::new(); // (has_modified, has_untracked)

        for (file_path, status) in &self.statuses {
            // Walk up the directory tree for each changed file
            let mut current = file_path.parent();
            while let Some(dir) = current {
                let entry = dir_statuses
                    .entry(dir.to_path_buf())
                    .or_insert((false, false));

                match status {
                    GitStatus::Modified
                    | GitStatus::Added
                    | GitStatus::Deleted
                    | GitStatus::Renamed
                    | GitStatus::Conflict => {
                        entry.0 = true;
                    }
                    GitStatus::Untracked => {
                        entry.1 = true;
                    }
                    _ => {}
                }

                current = dir.parent();
            }
        }

        // Convert to GitStatus
        for (dir, (has_modified, has_untracked)) in dir_statuses {
            let status = if has_modified {
                GitStatus::Modified
            } else if has_untracked {
                GitStatus::Untracked
            } else {
                GitStatus::None
            };

            if status != GitStatus::None {
                self.dir_status_cache.insert(dir, status);
            }
        }
    }

    pub fn get_status(&self, path: &Path) -> GitStatus {
        // Direct match for files
        if let Some(&status) = self.statuses.get(path) {
            return status;
        }

        // For directories, use the cache
        if path.is_dir() {
            if let Some(&status) = self.dir_status_cache.get(path) {
                return status;
            }
        }

        GitStatus::None
    }

    #[allow(dead_code)]
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
