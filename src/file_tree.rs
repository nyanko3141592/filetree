use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct FileNode {
    pub path: PathBuf,
    pub name: String,
    pub is_dir: bool,
    pub expanded: bool,
    pub depth: usize,
    pub children: Vec<FileNode>,
}

impl FileNode {
    pub fn new(path: PathBuf, depth: usize) -> Self {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.to_string_lossy().to_string());
        let is_dir = path.is_dir();

        Self {
            path,
            name,
            is_dir,
            expanded: false,
            depth,
            children: Vec::new(),
        }
    }

    pub fn load_children(&mut self, show_hidden: bool) -> anyhow::Result<()> {
        if !self.is_dir {
            return Ok(());
        }

        self.children.clear();
        let mut entries: Vec<_> = fs::read_dir(&self.path)?
            .filter_map(|e| e.ok())
            .filter(|e| {
                show_hidden
                    || e.file_name()
                        .to_str()
                        .map(|s| !s.starts_with('.'))
                        .unwrap_or(true)
            })
            .collect();

        entries.sort_by(|a, b| {
            let a_is_dir = a.path().is_dir();
            let b_is_dir = b.path().is_dir();
            match (a_is_dir, b_is_dir) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.file_name().cmp(&b.file_name()),
            }
        });

        for entry in entries {
            self.children
                .push(FileNode::new(entry.path(), self.depth + 1));
        }

        Ok(())
    }

    #[allow(dead_code)]
    pub fn toggle_expand(&mut self, show_hidden: bool) -> anyhow::Result<()> {
        if !self.is_dir {
            return Ok(());
        }

        self.expanded = !self.expanded;
        if self.expanded && self.children.is_empty() {
            self.load_children(show_hidden)?;
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct FileTree {
    pub root: FileNode,
    pub flat_list: Vec<usize>,
    nodes: Vec<FileNode>,
    pub show_hidden: bool,
}

impl FileTree {
    pub fn new(path: &Path, show_hidden: bool) -> anyhow::Result<Self> {
        let mut root = FileNode::new(path.to_path_buf(), 0);
        root.expanded = true;
        root.load_children(show_hidden)?;

        let mut tree = Self {
            root,
            flat_list: Vec::new(),
            nodes: Vec::new(),
            show_hidden,
        };
        tree.rebuild_flat_list();
        Ok(tree)
    }

    pub fn rebuild_flat_list(&mut self) {
        self.nodes.clear();
        self.flat_list.clear();
        self.flatten_node(&self.root.clone());
        for i in 0..self.nodes.len() {
            self.flat_list.push(i);
        }
    }

    fn flatten_node(&mut self, node: &FileNode) {
        self.nodes.push(node.clone());
        if node.expanded {
            for child in &node.children {
                self.flatten_node(child);
            }
        }
    }

    pub fn get_node(&self, index: usize) -> Option<&FileNode> {
        self.nodes.get(index)
    }

    #[allow(dead_code)]
    pub fn get_node_mut(&mut self, index: usize) -> Option<&mut FileNode> {
        self.nodes.get_mut(index)
    }

    #[allow(dead_code)]
    pub fn toggle_expand(&mut self, index: usize) -> anyhow::Result<()> {
        let path = {
            let node = self.nodes.get(index);
            node.map(|n| n.path.clone())
        };

        if let Some(path) = path {
            self.toggle_expand_recursive(&mut self.root.clone(), &path)?;
            self.rebuild_flat_list();
        }
        Ok(())
    }

    #[allow(dead_code)]
    fn toggle_expand_recursive(
        &mut self,
        node: &mut FileNode,
        target_path: &Path,
    ) -> anyhow::Result<bool> {
        if node.path == target_path {
            node.toggle_expand(self.show_hidden)?;
            self.update_root(node.clone());
            return Ok(true);
        }

        if node.expanded {
            for child in &mut node.children {
                if self.toggle_expand_recursive(child, target_path)? {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    #[allow(dead_code)]
    fn update_root(&mut self, new_root: FileNode) {
        if self.root.path == new_root.path {
            self.root = new_root;
        }
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn refresh(&mut self) -> anyhow::Result<()> {
        // Collect expanded paths before refresh
        let expanded_paths = self.collect_expanded_paths();

        let root_path = self.root.path.clone();
        self.root = FileNode::new(root_path, 0);
        self.root.expanded = true;
        self.root.load_children(self.show_hidden)?;

        // Restore expanded state
        for path in &expanded_paths {
            Self::restore_expanded_recursive(&mut self.root, path, self.show_hidden);
        }

        self.rebuild_flat_list();
        Ok(())
    }

    /// Collect all expanded directory paths
    fn collect_expanded_paths(&self) -> Vec<PathBuf> {
        let mut paths = Vec::new();
        Self::collect_expanded_recursive(&self.root, &mut paths);
        paths
    }

    fn collect_expanded_recursive(node: &FileNode, paths: &mut Vec<PathBuf>) {
        if node.is_dir && node.expanded {
            paths.push(node.path.clone());
            for child in &node.children {
                Self::collect_expanded_recursive(child, paths);
            }
        }
    }

    fn restore_expanded_recursive(node: &mut FileNode, target_path: &Path, show_hidden: bool) {
        if !node.is_dir {
            return;
        }

        if node.path == target_path {
            node.expanded = true;
            if node.children.is_empty() {
                let _ = node.load_children(show_hidden);
            }
            return;
        }

        // Check if target_path is under this node
        if target_path.starts_with(&node.path) {
            if !node.expanded {
                node.expanded = true;
                if node.children.is_empty() {
                    let _ = node.load_children(show_hidden);
                }
            }
            for child in &mut node.children {
                Self::restore_expanded_recursive(child, target_path, show_hidden);
            }
        }
    }

    pub fn set_show_hidden(&mut self, show_hidden: bool) -> anyhow::Result<()> {
        self.show_hidden = show_hidden;
        self.refresh()
    }

    pub fn collapse_all(&mut self) {
        Self::collapse_all_recursive(&mut self.root);
        self.root.expanded = true; // Keep root expanded
        self.rebuild_flat_list();
    }

    fn collapse_all_recursive(node: &mut FileNode) {
        node.expanded = false;
        for child in &mut node.children {
            Self::collapse_all_recursive(child);
        }
    }

    pub fn expand_all(&mut self) -> anyhow::Result<()> {
        Self::expand_all_recursive(&mut self.root, self.show_hidden)?;
        self.rebuild_flat_list();
        Ok(())
    }

    fn expand_all_recursive(node: &mut FileNode, show_hidden: bool) -> anyhow::Result<()> {
        if node.is_dir {
            node.expanded = true;
            if node.children.is_empty() {
                node.load_children(show_hidden)?;
            }
            for child in &mut node.children {
                Self::expand_all_recursive(child, show_hidden)?;
            }
        }
        Ok(())
    }

    pub fn expand_node(&mut self, index: usize) -> anyhow::Result<()> {
        if let Some(node) = self.nodes.get(index) {
            if node.is_dir && !node.expanded {
                let path = node.path.clone();
                self.expand_path(&path)?;
            }
        }
        Ok(())
    }

    pub fn collapse_node(&mut self, index: usize) -> anyhow::Result<()> {
        if let Some(node) = self.nodes.get(index) {
            if node.is_dir && node.expanded {
                let path = node.path.clone();
                self.collapse_path(&path)?;
            }
        }
        Ok(())
    }

    fn expand_path(&mut self, target_path: &Path) -> anyhow::Result<()> {
        Self::expand_path_recursive(&mut self.root, target_path, self.show_hidden)?;
        self.rebuild_flat_list();
        Ok(())
    }

    fn expand_path_recursive(
        node: &mut FileNode,
        target_path: &Path,
        show_hidden: bool,
    ) -> anyhow::Result<bool> {
        if node.path == target_path {
            if !node.expanded {
                node.expanded = true;
                if node.children.is_empty() {
                    node.load_children(show_hidden)?;
                }
            }
            return Ok(true);
        }

        if node.expanded {
            for child in &mut node.children {
                if Self::expand_path_recursive(child, target_path, show_hidden)? {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    fn collapse_path(&mut self, target_path: &Path) -> anyhow::Result<()> {
        Self::collapse_path_recursive(&mut self.root, target_path);
        self.rebuild_flat_list();
        Ok(())
    }

    fn collapse_path_recursive(node: &mut FileNode, target_path: &Path) -> bool {
        if node.path == target_path {
            node.expanded = false;
            return true;
        }

        if node.expanded {
            for child in &mut node.children {
                if Self::collapse_path_recursive(child, target_path) {
                    return true;
                }
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use tempfile::TempDir;

    fn create_test_structure() -> TempDir {
        let temp_dir = TempDir::new().unwrap();
        let base = temp_dir.path();

        // Create directories
        fs::create_dir(base.join("dir_a")).unwrap();
        fs::create_dir(base.join("dir_b")).unwrap();
        fs::create_dir(base.join(".hidden_dir")).unwrap();

        // Create files
        File::create(base.join("file1.txt")).unwrap();
        File::create(base.join("file2.rs")).unwrap();
        File::create(base.join(".hidden_file")).unwrap();
        File::create(base.join("dir_a/nested.txt")).unwrap();

        temp_dir
    }

    #[test]
    fn test_file_node_new_file() {
        let temp_dir = create_test_structure();
        let file_path = temp_dir.path().join("file1.txt");

        let node = FileNode::new(file_path.clone(), 1);

        assert_eq!(node.path, file_path);
        assert_eq!(node.name, "file1.txt");
        assert!(!node.is_dir);
        assert!(!node.expanded);
        assert_eq!(node.depth, 1);
        assert!(node.children.is_empty());
    }

    #[test]
    fn test_file_node_new_directory() {
        let temp_dir = create_test_structure();
        let dir_path = temp_dir.path().join("dir_a");

        let node = FileNode::new(dir_path.clone(), 2);

        assert_eq!(node.path, dir_path);
        assert_eq!(node.name, "dir_a");
        assert!(node.is_dir);
        assert!(!node.expanded);
        assert_eq!(node.depth, 2);
    }

    #[test]
    fn test_file_node_load_children_excludes_hidden() {
        let temp_dir = create_test_structure();
        let mut node = FileNode::new(temp_dir.path().to_path_buf(), 0);

        node.load_children(false).unwrap();

        let names: Vec<&str> = node.children.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"dir_a"));
        assert!(names.contains(&"dir_b"));
        assert!(names.contains(&"file1.txt"));
        assert!(!names.contains(&".hidden_dir"));
        assert!(!names.contains(&".hidden_file"));
    }

    #[test]
    fn test_file_node_load_children_includes_hidden() {
        let temp_dir = create_test_structure();
        let mut node = FileNode::new(temp_dir.path().to_path_buf(), 0);

        node.load_children(true).unwrap();

        let names: Vec<&str> = node.children.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"dir_a"));
        assert!(names.contains(&".hidden_dir"));
        assert!(names.contains(&".hidden_file"));
    }

    #[test]
    fn test_file_node_load_children_sorts_dirs_first() {
        let temp_dir = create_test_structure();
        let mut node = FileNode::new(temp_dir.path().to_path_buf(), 0);

        node.load_children(false).unwrap();

        // Find first file index
        let first_file_idx = node
            .children
            .iter()
            .position(|c| !c.is_dir)
            .unwrap_or(node.children.len());

        // All items before first file should be directories
        for child in node.children.iter().take(first_file_idx) {
            assert!(child.is_dir, "{} should be a directory", child.name);
        }

        // All items from first file onward should be files
        for child in node.children.iter().skip(first_file_idx) {
            assert!(!child.is_dir, "{} should be a file", child.name);
        }
    }

    #[test]
    fn test_file_tree_new() {
        let temp_dir = create_test_structure();

        let tree = FileTree::new(temp_dir.path(), false).unwrap();

        assert!(tree.root.expanded);
        assert!(!tree.root.children.is_empty());
        assert!(!tree.flat_list.is_empty());
    }

    #[test]
    fn test_file_tree_len() {
        let temp_dir = create_test_structure();
        let tree = FileTree::new(temp_dir.path(), false).unwrap();

        // Root + 2 dirs + 2 files (hidden excluded)
        assert_eq!(tree.len(), 5);
    }

    #[test]
    fn test_file_tree_get_node() {
        let temp_dir = create_test_structure();
        let tree = FileTree::new(temp_dir.path(), false).unwrap();

        let node = tree.get_node(0);
        assert!(node.is_some());
        assert_eq!(node.unwrap().path, temp_dir.path());

        let invalid = tree.get_node(1000);
        assert!(invalid.is_none());
    }

    #[test]
    fn test_file_tree_collapse_all() {
        let temp_dir = create_test_structure();
        let mut tree = FileTree::new(temp_dir.path(), false).unwrap();

        // Expand a child directory first
        if let Some(dir_idx) = (0..tree.len()).find(|&i| {
            tree.get_node(i)
                .map(|n| n.is_dir && n.name == "dir_a")
                .unwrap_or(false)
        }) {
            tree.expand_node(dir_idx).unwrap();
        }

        tree.collapse_all();

        // Root should still be expanded
        assert!(tree.root.expanded);
        // But children should be collapsed
        for child in &tree.root.children {
            assert!(!child.expanded);
        }
    }

    #[test]
    fn test_file_tree_set_show_hidden() {
        let temp_dir = create_test_structure();
        let mut tree = FileTree::new(temp_dir.path(), false).unwrap();

        let count_without_hidden = tree.len();

        tree.set_show_hidden(true).unwrap();

        let count_with_hidden = tree.len();

        assert!(count_with_hidden > count_without_hidden);
    }

    #[test]
    fn test_file_tree_refresh() {
        let temp_dir = create_test_structure();
        let mut tree = FileTree::new(temp_dir.path(), false).unwrap();

        let initial_len = tree.len();

        // Create a new file
        File::create(temp_dir.path().join("new_file.txt")).unwrap();

        tree.refresh().unwrap();

        assert_eq!(tree.len(), initial_len + 1);
    }

    #[test]
    fn test_file_tree_expand_and_collapse_node() {
        let temp_dir = create_test_structure();
        let mut tree = FileTree::new(temp_dir.path(), false).unwrap();

        // Find dir_a
        let dir_idx = (0..tree.len())
            .find(|&i| {
                tree.get_node(i)
                    .map(|n| n.is_dir && n.name == "dir_a")
                    .unwrap_or(false)
            })
            .unwrap();

        let len_before = tree.len();

        // Expand
        tree.expand_node(dir_idx).unwrap();
        let len_after_expand = tree.len();
        assert!(len_after_expand > len_before);

        // Collapse
        tree.collapse_node(dir_idx).unwrap();
        let len_after_collapse = tree.len();
        assert_eq!(len_after_collapse, len_before);
    }

    #[test]
    fn test_file_tree_is_empty() {
        let temp_dir = TempDir::new().unwrap();
        let tree = FileTree::new(temp_dir.path(), false).unwrap();

        // Tree has at least root
        assert!(!tree.is_empty());
    }
}
