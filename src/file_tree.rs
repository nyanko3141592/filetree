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
                if show_hidden {
                    true
                } else {
                    // Filter out hidden files (starting with .)
                    e.file_name()
                        .to_str()
                        .map(|s| !s.starts_with('.'))
                        .unwrap_or(true)
                }
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
                    self.update_node_in_root(&node.path, node.clone());
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

    #[allow(dead_code)]
    fn update_node_in_root(&mut self, _path: &Path, _node: FileNode) {
        // Will be rebuilt via rebuild_flat_list
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn refresh(&mut self) -> anyhow::Result<()> {
        let root_path = self.root.path.clone();
        self.root = FileNode::new(root_path, 0);
        self.root.expanded = true;
        self.root.load_children(self.show_hidden)?;
        self.rebuild_flat_list();
        Ok(())
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
