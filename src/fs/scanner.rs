use crate::app::core::AppEvent;
use crate::fs::tree::{FsEntry, FsTree};
use jwalk::WalkDir;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard, mpsc};
use std::thread;

pub struct FsScanner {
    pub scanning: Arc<AtomicBool>,
    pub scanning_entries: Arc<AtomicU64>,
    pub scanning_size: Arc<AtomicU64>,
    tree: Arc<RwLock<FsTree>>,
    event_sender: mpsc::Sender<AppEvent>,
}

impl FsScanner {
    pub fn new(event_sender: mpsc::Sender<AppEvent>) -> Self {
        Self {
            scanning: Arc::new(AtomicBool::new(false)),
            scanning_entries: Arc::new(AtomicU64::new(0)),
            scanning_size: Arc::new(AtomicU64::new(0)),
            tree: Arc::new(RwLock::new(FsTree::new())),
            event_sender,
        }
    }

    pub fn read_tree(&self) -> RwLockReadGuard<'_, FsTree> {
        self.tree.read().unwrap()
    }

    pub fn write_tree(&self) -> RwLockWriteGuard<'_, FsTree> {
        self.tree.write().unwrap()
    }

    pub fn scan(&self, path: &Path) -> bool {
        if self.scanning.swap(true, Ordering::AcqRel) {
            return false;
        }

        let path = path.to_path_buf();
        let scanning = self.scanning.clone();
        let scanning_entries = self.scanning_entries.clone();
        let scanning_size = self.scanning_size.clone();
        let tree = self.tree.clone();
        let event_sender = self.event_sender.clone();

        thread::spawn(move || {
            *tree.write().unwrap() = Self::scan_tree(&path, &scanning_entries, &scanning_size);

            scanning.store(false, Ordering::Relaxed);
            scanning_entries.store(0, Ordering::Relaxed);
            scanning_size.store(0, Ordering::Relaxed);
            let _ = event_sender.send(AppEvent::ScanCompleted);
        });

        true
    }

    fn scan_tree(path: &Path, scanning_entries: &AtomicU64, scanning_size: &AtomicU64) -> FsTree {
        let mut tree = FsTree::new();
        let root_id = tree.ensure_dir(path);
        tree.mark_scanned(root_id);

        tree = WalkDir::new(path)
            .into_iter()
            .filter_map(Result::ok)
            .skip(1) // skip root
            .fold(tree, |mut tree, entry| {
                if entry.depth() == 1 {
                    scanning_entries.fetch_add(1, Ordering::Relaxed);
                }

                let (size, created, modified) = match entry.metadata() {
                    Ok(m) => (Some(m.len()), m.created().ok(), m.modified().ok()),
                    Err(_) => (None, None, None),
                };
                if let Some(size) = size {
                    scanning_size.fetch_add(size, Ordering::Relaxed);
                }

                let name = entry.file_name.to_owned();
                let fs_entry = if entry.file_type().is_dir() {
                    FsEntry::dir(name, true, created, modified)
                } else {
                    FsEntry::file(name, size, created, modified)
                };

                tree.add_entry(fs_entry, entry.parent_path());
                tree
            });

        tree.compute_dir_sizes();
        tree
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::Duration;
    use tempfile::{TempDir, tempdir};

    /// Dir structure:
    ///
    /// notes.txt (100 bytes)
    /// docs/password.txt (50 bytes)
    /// docs/dot.txt (2 bytes)
    fn populated_dir() -> TempDir {
        let dir = tempdir().unwrap();

        fs::write(dir.path().join("notes.txt"), [0u8; 100]).unwrap();
        fs::create_dir(dir.path().join("docs")).unwrap();
        fs::write(dir.path().join("docs/password.txt"), [0u8; 50]).unwrap();
        fs::write(dir.path().join("docs/dot.txt"), [0u8; 2]).unwrap();

        dir
    }

    #[test]
    fn scan() {
        // Given
        let dir = populated_dir();
        let (tx, rx) = mpsc::channel();
        let scanner = FsScanner::new(tx);

        // When
        assert!(scanner.scan(dir.path()));
        let event = rx
            .recv_timeout(Duration::from_secs(5))
            .expect("scan should complete");

        // Then
        assert_eq!(event, AppEvent::ScanCompleted);

        let tree = scanner.read_tree();
        let files_count = tree.count_files(dir.path());
        let total_size = tree
            .entry_at(dir.path())
            .expect("root entry not found after scan")
            .size()
            .expect("root entry size not computed after scan");

        assert_eq!(files_count, 3);
        assert_eq!(total_size, 100 + 50 + 2);
    }
}
