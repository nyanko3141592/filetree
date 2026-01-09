use std::fs;
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

    pub fn clear(&mut self) {
        self.content = None;
    }

    pub fn is_empty(&self) -> bool {
        self.content.is_none()
    }
}

pub fn copy_file(src: &Path, dest_dir: &Path) -> anyhow::Result<PathBuf> {
    let file_name = src.file_name().ok_or_else(|| anyhow::anyhow!("Invalid file name"))?;
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
    let file_name = src.file_name().ok_or_else(|| anyhow::anyhow!("Invalid file name"))?;
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
    let parent = path.parent().ok_or_else(|| anyhow::anyhow!("No parent directory"))?;
    let new_path = parent.join(new_name);

    if new_path.exists() {
        anyhow::bail!("File already exists: {}", new_path.display());
    }

    fs::rename(path, &new_path)?;
    Ok(new_path)
}

pub fn create_file(parent_dir: &Path, name: &str) -> anyhow::Result<PathBuf> {
    let path = parent_dir.join(name);

    if path.exists() {
        anyhow::bail!("File already exists: {}", path.display());
    }

    fs::write(&path, "")?;
    Ok(path)
}

pub fn create_directory(parent_dir: &Path, name: &str) -> anyhow::Result<PathBuf> {
    let path = parent_dir.join(name);

    if path.exists() {
        anyhow::bail!("Directory already exists: {}", path.display());
    }

    fs::create_dir(&path)?;
    Ok(path)
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
