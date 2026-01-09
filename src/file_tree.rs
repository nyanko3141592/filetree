use std::path::{Path, PathBuf};
use std::fs;

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

    pub fn load_children(&mut self) -> anyhow::Result<()> {
        if !self.is_dir {
            return Ok(());
        }

        self.children.clear();
        let mut entries: Vec<_> = fs::read_dir(&self.path)?
            .filter_map(|e| e.ok())
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
            self.children.push(FileNode::new(entry.path(), self.depth + 1));
        }

        Ok(())
    }

    pub fn toggle_expand(&mut self) -> anyhow::Result<()> {
        if !self.is_dir {
            return Ok(());
        }

        self.expanded = !self.expanded;
        if self.expanded && self.children.is_empty() {
            self.load_children()?;
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct FileTree {
    pub root: FileNode,
    pub flat_list: Vec<usize>,
    nodes: Vec<FileNode>,
}

impl FileTree {
    pub fn new(path: &Path) -> anyhow::Result<Self> {
        let mut root = FileNode::new(path.to_path_buf(), 0);
        root.expanded = true;
        root.load_children()?;

        let mut tree = Self {
            root,
            flat_list: Vec::new(),
            nodes: Vec::new(),
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

    pub fn get_node_mut(&mut self, index: usize) -> Option<&mut FileNode> {
        self.nodes.get_mut(index)
    }

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

    fn toggle_expand_recursive(&mut self, node: &mut FileNode, target_path: &Path) -> anyhow::Result<bool> {
        if node.path == target_path {
            node.toggle_expand()?;
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

    fn update_root(&mut self, new_root: FileNode) {
        if self.root.path == new_root.path {
            self.root = new_root;
        }
    }

    fn update_node_in_root(&mut self, _path: &Path, _node: FileNode) {
        // Will be rebuilt via rebuild_flat_list
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn refresh(&mut self) -> anyhow::Result<()> {
        let root_path = self.root.path.clone();
        self.root = FileNode::new(root_path, 0);
        self.root.expanded = true;
        self.root.load_children()?;
        self.rebuild_flat_list();
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
        Self::expand_path_recursive(&mut self.root, target_path)?;
        self.rebuild_flat_list();
        Ok(())
    }

    fn expand_path_recursive(node: &mut FileNode, target_path: &Path) -> anyhow::Result<bool> {
        if node.path == target_path {
            if !node.expanded {
                node.expanded = true;
                if node.children.is_empty() {
                    node.load_children()?;
                }
            }
            return Ok(true);
        }

        if node.expanded {
            for child in &mut node.children {
                if Self::expand_path_recursive(child, target_path)? {
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
