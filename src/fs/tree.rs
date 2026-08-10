use getset::{CopyGetters, Getters, Setters};
use indextree::{Arena, NodeId};
use std::ffi::{OsStr, OsString};
use std::path::Path;
use std::time::SystemTime;

pub struct FsTree {
    arena: Arena<FsEntry>,
    root: NodeId,
}

impl FsTree {
    pub fn new() -> Self {
        let mut arena = Arena::new();
        let root = arena.new_node(FsEntry::dir("ROOT".into(), false, None, None));
        Self { arena, root }
    }

    pub fn is_scanned(&self, path: &Path) -> bool {
        self.find(path)
            .is_some_and(|id| self.arena[id].get().scanned())
    }

    pub fn entry_at(&self, path: &Path) -> Option<&FsEntry> {
        self.find(path).map(|id| self.arena[id].get())
    }

    pub fn entries_at(&self, path: &Path) -> Vec<&FsEntry> {
        let Some(id) = self.find(path) else {
            return Vec::new();
        };

        id.children(&self.arena)
            .map(|id| self.arena[id].get())
            .collect()
    }

    pub fn count_files(&self, path: &Path) -> u64 {
        let Some(id) = self.find(path) else {
            return 0;
        };

        if !self.arena[id].get().is_dir() {
            return 1;
        }

        id.children(&self.arena)
            .map(|child_id| {
                let child = self.arena[child_id].get();
                self.count_files(&path.join(child.name()))
            })
            .sum::<u64>()
    }

    pub fn add_entry(&mut self, entry: FsEntry, parent_path: &Path) {
        let Some(parent_id) = self.find(parent_path) else {
            return;
        };
        let node = self.arena.new_node(entry);
        parent_id.append(node, &mut self.arena);
    }

    pub fn ensure_dir(&mut self, path: &Path) -> NodeId {
        Self::path_components(path).fold(self.root, |cur_id, component| {
            self.find_child_by_name(cur_id, component)
                .unwrap_or_else(|| {
                    let node =
                        self.arena
                            .new_node(FsEntry::dir(component.to_owned(), false, None, None));
                    cur_id.append(node, &mut self.arena);
                    node
                })
        })
    }

    pub fn compute_dir_sizes(&mut self) {
        self.compute_dir_size(self.root);
    }

    pub fn mark_scanned(&mut self, id: NodeId) {
        if let Some(node) = self.arena.get_mut(id) {
            node.get_mut().set_scanned(true);
        }
    }

    pub fn remove_entries<'a>(&mut self, paths: impl IntoIterator<Item = &'a Path>) {
        for path in paths {
            self.remove_entry(path);
        }
    }

    fn find(&self, path: &Path) -> Option<NodeId> {
        Self::path_components(path).try_fold(self.root, |cur_id, component| {
            self.find_child_by_name(cur_id, component)
        })
    }

    fn path_components(path: &Path) -> impl Iterator<Item = &OsStr> {
        path.components()
            .filter(|c| !matches!(c, std::path::Component::RootDir))
            .map(|c| c.as_os_str())
    }

    fn find_child_by_name(&self, parent_id: NodeId, name: &OsStr) -> Option<NodeId> {
        parent_id
            .children(&self.arena)
            .find(|&id| self.arena[id].get().name() == name)
    }

    fn compute_dir_size(&mut self, id: NodeId) -> u64 {
        let total: u64 = id
            .children(&self.arena)
            .collect::<Vec<_>>()
            .into_iter()
            .map(|id| self.compute_dir_size(id))
            .sum();

        let entry = self.arena[id].get_mut();
        if entry.is_dir() {
            entry.set_size(Some(total));
        }
        entry.size().unwrap_or(0)
    }

    fn remove_entry(&mut self, path: &Path) {
        if let Some(id) = self.find(path) {
            id.remove_subtree(&mut self.arena);
        }
    }
}

#[derive(Clone, CopyGetters, Debug, Eq, Getters, PartialEq, Setters)]
#[getset(get_copy = "pub")]
pub struct FsEntry {
    #[getset(skip)]
    #[getset(get = "pub")]
    name: OsString,
    is_dir: bool,
    #[getset(set = "pub")]
    scanned: bool,
    #[getset(set = "pub")]
    size: Option<u64>,
    created: Option<SystemTime>,
    modified: Option<SystemTime>,
}

impl FsEntry {
    pub fn dir(
        name: OsString,
        scanned: bool,
        created: Option<SystemTime>,
        modified: Option<SystemTime>,
    ) -> Self {
        Self {
            name,
            is_dir: true,
            scanned,
            size: None,
            created,
            modified,
        }
    }

    pub fn file(
        name: OsString,
        size: Option<u64>,
        created: Option<SystemTime>,
        modified: Option<SystemTime>,
    ) -> Self {
        Self {
            name,
            is_dir: false,
            scanned: true,
            size,
            created,
            modified,
        }
    }
}

/// Tree structure:
///
/// /home
/// /home/user/
/// /home/user/cat.png
/// /home/user/notes.txt
/// /home/guest/
/// /home/guest/dog.png
/// /home/guest/empty/
#[cfg(test)]
pub fn populated_tree() -> FsTree {
    let mut tree = FsTree::new();

    tree.add_entry(
        FsEntry::dir("home".into(), false, None, None),
        Path::new("/"),
    );
    tree.add_entry(
        FsEntry::dir("user".into(), true, None, None),
        Path::new("/home"),
    );
    tree.add_entry(
        FsEntry::dir("guest".into(), true, None, None),
        Path::new("/home"),
    );
    tree.add_entry(
        FsEntry::file("cat.png".into(), Some(1024 * 1024), None, None),
        Path::new("/home/user"),
    );
    tree.add_entry(
        FsEntry::file("notes.txt".into(), Some(1024), None, None),
        Path::new("/home/user"),
    );
    tree.add_entry(
        FsEntry::file("dog.png".into(), Some(1024 * 1024), None, None),
        Path::new("/home/guest"),
    );
    tree.add_entry(
        FsEntry::dir("empty".into(), true, None, None),
        Path::new("/home/guest"),
    );

    tree
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_scanned() {
        let tree = populated_tree();

        let home_scanned = tree.is_scanned(Path::new("/home"));
        let user_scanned = tree.is_scanned(Path::new("/home/user"));
        let guest_scanned = tree.is_scanned(Path::new("/home/guest"));
        let missing_scanned = tree.is_scanned(Path::new("/missing"));

        assert!(!home_scanned);
        assert!(user_scanned);
        assert!(guest_scanned);
        assert!(!missing_scanned);
    }

    #[test]
    fn entry_at() {
        let tree = populated_tree();

        let home_entry = tree.entry_at(Path::new("/home"));
        let user_entry = tree.entry_at(Path::new("/home/user"));
        let cat_entry = tree.entry_at(Path::new("/home/user/cat.png"));
        let missing_entry = tree.entry_at(Path::new("/missing"));

        assert!(home_entry.is_some());
        assert!(user_entry.is_some());
        assert!(cat_entry.is_some());
        assert!(missing_entry.is_none());
    }

    #[test]
    fn entries_at() {
        let tree = populated_tree();

        let home_entries = tree.entries_at(Path::new("/home"));
        let user_entries = tree.entries_at(Path::new("/home/user"));
        let guest_entries = tree.entries_at(Path::new("/home/guest"));
        let missing_entries = tree.entries_at(Path::new("/missing"));

        assert_eq!(home_entries.len(), 2);
        assert_eq!(user_entries.len(), 2);
        assert_eq!(guest_entries.len(), 2);
        assert!(missing_entries.is_empty());
    }

    #[test]
    fn count_files() {
        let tree = populated_tree();

        let home_files = tree.count_files(Path::new("/home"));
        let user_files = tree.count_files(Path::new("/home/user"));
        let guest_files = tree.count_files(Path::new("/home/guest"));
        let missing_files = tree.count_files(Path::new("/missing"));

        assert_eq!(home_files, 3);
        assert_eq!(user_files, 2);
        assert_eq!(guest_files, 1);
        assert_eq!(missing_files, 0);
    }

    #[test]
    fn add_entry() {
        let mut tree = populated_tree();
        let old_nodes = tree.arena.count();

        tree.add_entry(
            FsEntry::dir("admin".into(), true, None, None),
            Path::new("/home"),
        );
        tree.add_entry(
            FsEntry::file("password.txt".into(), Some(128), None, None),
            Path::new("/home/admin"),
        );
        tree.add_entry(
            FsEntry::file("something".into(), Some(0), None, None),
            Path::new("/missing"),
        );
        let new_nodes = tree.arena.count();

        assert_eq!(new_nodes - 2, old_nodes);
        assert!(tree.find(Path::new("/home/admin")).is_some());
        assert!(tree.find(Path::new("/home/admin/password.txt")).is_some());
        assert!(tree.find(Path::new("/missing/something")).is_none());
    }

    #[test]
    fn ensure_dir() {
        let mut tree = populated_tree();

        tree.ensure_dir(Path::new("/home/admin/.config/diskly"));

        assert!(tree.find(Path::new("home/admin/.config")).is_some());
        assert!(tree.find(Path::new("home/admin/.config/diskly")).is_some());
    }

    #[test]
    fn compute_dir_sizes() {
        // Given
        let mut tree = populated_tree();

        // When
        tree.compute_dir_sizes();

        // Then
        let user_size = tree.arena[tree.find(Path::new("/home/user")).unwrap()]
            .get()
            .size();
        let guest_size = tree.arena[tree.find(Path::new("/home/guest")).unwrap()]
            .get()
            .size();
        let empty_size = tree.arena[tree.find(Path::new("/home/guest/empty")).unwrap()]
            .get()
            .size();
        let home_size = tree.arena[tree.find(Path::new("/home")).unwrap()]
            .get()
            .size();

        assert_eq!(user_size, Some(1024 * 1024 + 1024));
        assert_eq!(guest_size, Some(1024 * 1024));
        assert_eq!(empty_size, Some(0));
        assert_eq!(home_size, Some(user_size.unwrap() + guest_size.unwrap()));
    }

    #[test]
    fn remove_entries() {
        let mut tree = populated_tree();

        tree.remove_entries([Path::new("/home/user"), Path::new("/home/guest")]);

        assert!(tree.find(Path::new("/home/user")).is_none());
        assert!(tree.find(Path::new("/home/user/cat.png")).is_none());
        assert!(tree.find(Path::new("/home/user/notes.txt")).is_none());
        assert!(tree.find(Path::new("/home/guest")).is_none());
        assert!(tree.find(Path::new("/home/guest/dog.png")).is_none());
        assert!(tree.find(Path::new("/home/guest/empty")).is_none());
    }
}
