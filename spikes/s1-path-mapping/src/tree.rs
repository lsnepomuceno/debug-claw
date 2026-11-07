//! Finds the local files that share the entry script's name. The walk
//! runs once per inference, has a cap on the entries it visits, does not
//! follow symlinks and skips folders that never hold PHP sources.

use std::fs;
use std::path::{Path, PathBuf};

const SKIP: &[&str] = &[".git", ".hg", ".svn", "node_modules", ".idea"];

#[derive(Debug, Default)]
pub struct Walk {
    /// Matching files, as components relative to the root.
    pub found: Vec<Vec<String>>,
    /// Symlinks met on the way, relative to the root, with their target.
    pub symlinks: Vec<(String, PathBuf)>,
    pub visited: usize,
    pub truncated: bool,
}

pub fn find_named(root: &Path, name: &str, case_insensitive: bool, limit: usize) -> Walk {
    let mut walk = Walk::default();
    let mut stack: Vec<(PathBuf, Vec<String>)> = vec![(root.to_path_buf(), Vec::new())];
    let want = if case_insensitive { name.to_lowercase() } else { name.to_string() };
    while let Some((dir, rel)) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            walk.visited += 1;
            if walk.visited > limit {
                walk.truncated = true;
                return walk;
            }
            let file_name = entry.file_name().to_string_lossy().into_owned();
            let Ok(kind) = entry.file_type() else { continue };
            let mut child = rel.clone();
            child.push(file_name.clone());
            if kind.is_symlink() {
                let target = fs::read_link(entry.path()).unwrap_or_default();
                walk.symlinks.push((child.join("/"), target));
                continue;
            }
            if kind.is_dir() {
                if !SKIP.contains(&file_name.as_str()) {
                    stack.push((entry.path(), child));
                }
                continue;
            }
            let cmp = if case_insensitive { file_name.to_lowercase() } else { file_name };
            if cmp == want {
                walk.found.push(child);
            }
        }
    }
    walk.found.sort();
    walk
}
