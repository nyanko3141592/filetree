use std::fs::{self, OpenOptions};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub enum ClipboardContent {
    Copy(Vec<PathBuf>),
    Cut(Vec<PathBuf>),
}

#[derive(Default)]
pub struct Clipboard {
    pub content: Option<ClipboardContent>,
}

impl Clipboard {
    pub fn copy(&mut self, paths: Vec<PathBuf>) {
        self.content = Some(ClipboardContent::Copy(paths));
    }

    pub fn cut(&mut self, paths: Vec<PathBuf>) {
        self.content = Some(ClipboardContent::Cut(paths));
    }

    #[allow(dead_code)]
    pub fn clear(&mut self) {
        self.content = None;
    }

    pub fn is_empty(&self) -> bool {
        self.content.is_none()
    }
}

pub fn copy_file(src: &Path, dest_dir: &Path) -> anyhow::Result<PathBuf> {
    let file_name = src
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("Invalid file name"))?;
    let dest = dest_dir.join(file_name);
    let dest = get_unique_path(&dest);

    if src.is_dir() {
        copy_dir_recursive(src, &dest)?;
    } else {
        fs::copy(src, &dest)?;
    }
    Ok(dest)
}

pub fn move_file(src: &Path, dest_dir: &Path) -> anyhow::Result<PathBuf> {
    let file_name = src
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("Invalid file name"))?;
    let dest = dest_dir.join(file_name);
    let dest = get_unique_path(&dest);

    if fs::rename(src, &dest).is_err() {
        if src.is_dir() {
            copy_dir_recursive(src, &dest)?;
            fs::remove_dir_all(src)?;
        } else {
            fs::copy(src, &dest)?;
            fs::remove_file(src)?;
        }
    }
    Ok(dest)
}

pub fn delete_file(path: &Path) -> anyhow::Result<()> {
    if path.is_dir() {
        fs::remove_dir_all(path)?;
    } else {
        fs::remove_file(path)?;
    }
    Ok(())
}

pub fn rename_file(path: &Path, new_name: &str) -> anyhow::Result<PathBuf> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("No parent directory"))?;
    let new_path = parent.join(new_name);

    // Avoid renaming to the same path
    if path == new_path {
        return Ok(new_path);
    }

    // Try rename directly - avoids TOCTOU race condition
    match fs::rename(path, &new_path) {
        Ok(()) => Ok(new_path),
        Err(e) if e.kind() == ErrorKind::AlreadyExists => {
            anyhow::bail!("File already exists: {}", new_path.display())
        }
        Err(e) if e.kind() == ErrorKind::CrossesDevices => {
            // Cross-device rename: check destination first, then copy+delete
            if new_path.exists() {
                anyhow::bail!("File already exists: {}", new_path.display());
            }
            if path.is_dir() {
                copy_dir_recursive(path, &new_path)?;
                fs::remove_dir_all(path)?;
            } else {
                fs::copy(path, &new_path)?;
                fs::remove_file(path)?;
            }
            Ok(new_path)
        }
        Err(e) => Err(e.into()),
    }
}

pub fn create_file(parent_dir: &Path, name: &str) -> anyhow::Result<PathBuf> {
    let path = parent_dir.join(name);

    // Use create_new for atomic "create if not exists" - avoids TOCTOU race condition
    match OpenOptions::new().write(true).create_new(true).open(&path) {
        Ok(_) => Ok(path),
        Err(e) if e.kind() == ErrorKind::AlreadyExists => {
            anyhow::bail!("File already exists: {}", path.display())
        }
        Err(e) => Err(e.into()),
    }
}

pub fn create_directory(parent_dir: &Path, name: &str) -> anyhow::Result<PathBuf> {
    let path = parent_dir.join(name);

    // fs::create_dir fails atomically if directory exists - avoids TOCTOU race condition
    match fs::create_dir(&path) {
        Ok(()) => Ok(path),
        Err(e) if e.kind() == ErrorKind::AlreadyExists => {
            anyhow::bail!("Directory already exists: {}", path.display())
        }
        Err(e) => Err(e.into()),
    }
}

fn copy_dir_recursive(src: &Path, dest: &Path) -> anyhow::Result<()> {
    fs::create_dir_all(dest)?;

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dest_path = dest.join(entry.file_name());

        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dest_path)?;
        } else {
            fs::copy(&src_path, &dest_path)?;
        }
    }
    Ok(())
}

fn get_unique_path(path: &Path) -> PathBuf {
    if !path.exists() {
        return path.to_path_buf();
    }

    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    let ext = path.extension().and_then(|s| s.to_str());
    let parent = path.parent().unwrap_or(Path::new("."));

    let mut counter = 1;
    loop {
        let new_name = match ext {
            Some(e) => format!("{}_{}.{}", stem, counter, e),
            None => format!("{}_{}", stem, counter),
        };
        let new_path = parent.join(new_name);
        if !new_path.exists() {
            return new_path;
        }
        counter += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static TEST_COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn setup_test_dir() -> PathBuf {
        let id = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let test_dir = std::env::temp_dir().join(format!(
            "ft_test_{}_{}_{}",
            std::process::id(),
            id,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = fs::remove_dir_all(&test_dir);
        fs::create_dir_all(&test_dir).unwrap();
        test_dir
    }

    fn cleanup_test_dir(path: &Path) {
        let _ = fs::remove_dir_all(path);
    }

    #[test]
    fn test_create_file_success() {
        let test_dir = setup_test_dir();
        let result = create_file(&test_dir, "test.txt");
        assert!(result.is_ok());
        assert!(test_dir.join("test.txt").exists());
        cleanup_test_dir(&test_dir);
    }

    #[test]
    fn test_create_file_already_exists() {
        let test_dir = setup_test_dir();
        fs::write(test_dir.join("existing.txt"), "content").unwrap();
        let result = create_file(&test_dir, "existing.txt");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("already exists"));
        cleanup_test_dir(&test_dir);
    }

    #[test]
    fn test_create_directory_success() {
        let test_dir = setup_test_dir();
        let result = create_directory(&test_dir, "subdir");
        assert!(result.is_ok());
        assert!(test_dir.join("subdir").is_dir());
        cleanup_test_dir(&test_dir);
    }

    #[test]
    fn test_create_directory_already_exists() {
        let test_dir = setup_test_dir();
        fs::create_dir(test_dir.join("existing_dir")).unwrap();
        let result = create_directory(&test_dir, "existing_dir");
        assert!(result.is_err());
        cleanup_test_dir(&test_dir);
    }

    #[test]
    fn test_copy_file_success() {
        let test_dir = setup_test_dir();
        let src = test_dir.join("source.txt");
        fs::write(&src, "hello").unwrap();
        let dest_dir = test_dir.join("dest");
        fs::create_dir(&dest_dir).unwrap();

        let result = copy_file(&src, &dest_dir);
        assert!(result.is_ok());
        assert!(dest_dir.join("source.txt").exists());
        assert!(src.exists()); // Original still exists
        cleanup_test_dir(&test_dir);
    }

    #[test]
    fn test_copy_file_unique_name() {
        let test_dir = setup_test_dir();
        let src = test_dir.join("file.txt");
        fs::write(&src, "content").unwrap();

        // Create existing file in dest
        fs::write(test_dir.join("file.txt"), "existing").unwrap();

        let result = copy_file(&src, &test_dir);
        assert!(result.is_ok());
        let new_path = result.unwrap();
        assert_eq!(new_path.file_name().unwrap(), "file_1.txt");
        cleanup_test_dir(&test_dir);
    }

    #[test]
    fn test_delete_file() {
        let test_dir = setup_test_dir();
        let file = test_dir.join("to_delete.txt");
        fs::write(&file, "").unwrap();
        assert!(file.exists());

        let result = delete_file(&file);
        assert!(result.is_ok());
        assert!(!file.exists());
        cleanup_test_dir(&test_dir);
    }

    #[test]
    fn test_delete_directory() {
        let test_dir = setup_test_dir();
        let dir = test_dir.join("to_delete_dir");
        fs::create_dir(&dir).unwrap();
        fs::write(dir.join("file.txt"), "").unwrap();

        let result = delete_file(&dir);
        assert!(result.is_ok());
        assert!(!dir.exists());
        cleanup_test_dir(&test_dir);
    }

    #[test]
    fn test_rename_file_success() {
        let test_dir = setup_test_dir();
        let file = test_dir.join("old_name.txt");
        fs::write(&file, "content").unwrap();

        let result = rename_file(&file, "new_name.txt");
        assert!(result.is_ok());
        assert!(!file.exists());
        assert!(test_dir.join("new_name.txt").exists());
        cleanup_test_dir(&test_dir);
    }

    #[test]
    fn test_rename_file_same_name() {
        let test_dir = setup_test_dir();
        let file = test_dir.join("same.txt");
        fs::write(&file, "content").unwrap();

        let result = rename_file(&file, "same.txt");
        assert!(result.is_ok());
        assert!(file.exists());
        cleanup_test_dir(&test_dir);
    }

    #[test]
    fn test_move_file_success() {
        let test_dir = setup_test_dir();
        let src = test_dir.join("to_move.txt");
        fs::write(&src, "content").unwrap();
        let dest_dir = test_dir.join("dest");
        fs::create_dir(&dest_dir).unwrap();

        let result = move_file(&src, &dest_dir);
        assert!(result.is_ok());
        assert!(!src.exists()); // Original removed
        assert!(dest_dir.join("to_move.txt").exists());
        cleanup_test_dir(&test_dir);
    }

    #[test]
    fn test_clipboard_operations() {
        let mut clipboard = Clipboard::default();
        assert!(clipboard.is_empty());

        clipboard.copy(vec![PathBuf::from("/test/path")]);
        assert!(!clipboard.is_empty());

        clipboard.clear();
        assert!(clipboard.is_empty());

        clipboard.cut(vec![PathBuf::from("/test/path")]);
        assert!(!clipboard.is_empty());
    }
}
