use crate::fs::tree::FsEntry;
use getset::{CopyGetters, Getters};
use ratatui::layout::Rect;
use ratatui::widgets::TableState;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(CopyGetters, Default, Getters)]
pub struct BinState {
    #[getset(get = "pub")]
    rows: Vec<BinEntry>,
    #[getset(get_copy = "pub")]
    total_size: u64,
    #[getset(get_copy = "pub")]
    total_files: u64,
    #[getset(get = "pub")]
    table_state: TableState,
    pub table_area: Rect,
    pub scrollbar_area: Rect,
}

impl BinState {
    pub fn new() -> Self {
        Default::default()
    }

    /// Exposes table state mutably for rendering.
    ///
    /// # Warning
    /// Do not modify this state manually outside of rendering.
    pub fn table_state_mut(&mut self) -> &mut TableState {
        &mut self.table_state
    }

    pub fn contains_path(&self, path: &Path) -> bool {
        self.rows.iter().any(|e| e.path == path)
    }

    pub fn push_entry(&mut self, entry: BinEntry) {
        self.total_size += entry.size;
        self.total_files += entry.file_count;
        self.rows.push(entry);
        self.table_state.select(Some(self.rows.len() - 1));
    }

    pub fn push_entries(&mut self, entries: Vec<BinEntry>) {
        for entry in entries {
            self.push_entry(entry);
        }
    }

    pub fn remove_entry(&mut self, index: usize) {
        let entry = self.rows.remove(index);
        self.total_size -= entry.size;
        self.total_files -= entry.file_count;
        self.table_state.select_previous();
    }

    pub fn restore_by_path(&mut self, path: &Path) {
        let Some(index) = self.rows.iter().position(|e| e.path == path) else {
            return;
        };
        self.remove_entry(index)
    }

    pub fn restore_selected(&mut self) {
        let Some(selected) = self.table_state.selected() else {
            return;
        };
        self.remove_entry(selected)
    }

    pub fn clear(&mut self) -> Vec<BinEntry> {
        let cleared: Vec<BinEntry> = self
            .rows
            .extract_if(.., |entry| {
                if entry.is_dir {
                    fs::remove_dir_all(&entry.path).is_ok()
                } else {
                    fs::remove_file(&entry.path).is_ok()
                }
            })
            .collect();

        self.total_size = self.rows.iter().map(|e| e.size).sum();
        self.total_files = self.rows.iter().map(|e| e.file_count).sum();
        self.table_state.select_first();

        cleared
    }

    pub fn select_row(&mut self, index: Option<usize>) -> bool {
        match index {
            Some(idx) => {
                if idx < self.rows.len() {
                    self.table_state.select(Some(idx));
                    true
                } else {
                    false
                }
            }
            None => {
                self.table_state.select(index);
                true
            }
        }
    }

    pub fn select_previous_row(&mut self) -> bool {
        let selected = self.table_state().selected();
        self.select_row(Some(selected.unwrap_or(0).saturating_sub(1)))
    }

    pub fn select_next_row(&mut self) -> bool {
        let selected = self.table_state().selected();
        self.select_row(Some(selected.unwrap_or(0).saturating_add(1)))
    }
}

#[derive(Clone, Default)]
pub struct BinEntry {
    pub path: PathBuf,
    pub size: u64,
    pub is_dir: bool,
    pub file_count: u64,
}

impl BinEntry {
    pub fn from_entry_at(entry: &FsEntry, path: PathBuf, file_count: u64) -> Self {
        Self {
            path,
            size: entry.size().unwrap_or(0),
            is_dir: entry.is_dir(),
            file_count,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn state() -> BinState {
        BinState::new()
    }

    #[test]
    fn push_entry() {
        let mut state = state();
        let entry = BinEntry {
            size: 100,
            file_count: 10,
            ..Default::default()
        };

        state.push_entry(entry);

        assert_eq!(state.total_size(), 100);
        assert_eq!(state.total_files(), 10);
        assert_eq!(state.rows().len(), 1);
        assert_eq!(state.table_state().selected(), Some(0));
    }

    #[test]
    fn remove_entry() {
        let mut state = state();
        let entry = BinEntry {
            size: 100,
            file_count: 10,
            ..Default::default()
        };
        state.push_entry(entry);

        state.remove_entry(0);

        assert_eq!(state.total_size(), 0);
        assert_eq!(state.total_files(), 0);
        assert_eq!(state.rows().len(), 0);
        assert_eq!(state.table_state().selected(), Some(0));
    }

    #[test]
    fn restore_by_path() {
        let mut state = state();
        let path = PathBuf::from("file.txt");
        state.push_entry(BinEntry {
            path: path.clone(),
            size: 100,
            file_count: 1,
            ..Default::default()
        });

        state.restore_by_path(&path);

        assert_eq!(state.rows().len(), 0);
        assert_eq!(state.total_size(), 0);
        assert_eq!(state.total_files(), 0);
    }

    #[test]
    fn restore_by_path_not_found() {
        let mut state = state();
        state.push_entry(BinEntry {
            path: PathBuf::from("file.txt"),
            size: 100,
            file_count: 1,
            ..Default::default()
        });

        state.restore_by_path(Path::new("other.txt"));

        assert_eq!(state.rows().len(), 1);
        assert_eq!(state.total_size(), 100);
        assert_eq!(state.total_files(), 1);
    }

    #[test]
    fn restore_selected() {
        let mut state = state();
        state.push_entry(BinEntry {
            size: 100,
            file_count: 1,
            ..Default::default()
        });
        state.push_entry(BinEntry {
            size: 200,
            is_dir: true,
            file_count: 2,
            ..Default::default()
        });

        state.restore_selected();

        assert_eq!(state.rows().len(), 1);
        assert_eq!(state.total_size(), 100);
        assert_eq!(state.total_files(), 1);
    }

    #[test]
    fn clear() {
        // Given
        let mut state = state();

        let dir = tempdir().unwrap();
        let dir_path = dir.path();

        let file_path = dir_path.join("test_file.txt");
        fs::write(&file_path, "hello world").unwrap();

        let dir_path = dir_path.join("test_dir");
        fs::create_dir(&dir_path).unwrap();

        let entry_file = BinEntry {
            path: file_path.clone(),
            size: 100,
            is_dir: false,
            file_count: 1,
        };
        let entry_dir = BinEntry {
            path: dir_path.clone(),
            size: 200,
            is_dir: true,
            file_count: 0,
        };

        state.push_entry(entry_file);
        state.push_entry(entry_dir);

        // When
        let cleared = state.clear();

        // Then
        assert!(!file_path.exists());
        assert!(!dir_path.exists());

        assert_eq!(cleared.len(), 2);

        assert_eq!(state.rows().len(), 0);
        assert_eq!(state.total_size(), 0);
        assert_eq!(state.total_files(), 0);
    }

    #[test]
    fn clear_non_existent_file() {
        let mut state = state();
        let file_path = PathBuf::from("non_existent_file_12345.tmp");
        state.push_entry(BinEntry {
            path: file_path.clone(),
            size: 100,
            is_dir: false,
            file_count: 1,
        });

        let cleared = state.clear();

        assert_eq!(cleared.len(), 0);
        assert_eq!(state.rows().len(), 1);
        assert_eq!(state.total_size(), 100);
        assert_eq!(state.total_files(), 1);
    }

    #[test]
    fn select_row() {
        let mut state = state();
        state.push_entry(BinEntry {
            size: 100,
            file_count: 1,
            ..Default::default()
        });
        state.push_entry(BinEntry {
            size: 200,
            file_count: 1,
            ..Default::default()
        });

        assert!(state.select_row(Some(0)));
        assert_eq!(state.table_state().selected(), Some(0));

        assert!(state.select_row(None));
        assert_eq!(state.table_state().selected(), None);
    }

    #[test]
    fn select_row_out_of_bounds() {
        let mut state = state();
        state.push_entry(BinEntry {
            size: 100,
            file_count: 1,
            ..Default::default()
        });

        assert!(!state.select_row(Some(5)));
        assert_eq!(state.table_state().selected(), Some(0));
    }

    #[test]
    fn select_row_empty() {
        let mut state = state();
        assert!(state.select_row(None));
        assert!(!state.select_row(Some(0)));
    }

    #[test]
    fn select_previous_row() {
        let mut state = state();
        state.push_entries(vec![BinEntry::default(); 3]);
        state.select_row(Some(2));

        state.select_previous_row();

        assert_eq!(state.table_state.selected(), Some(1));
    }

    #[test]
    fn select_previous_row_none() {
        let mut state = state();
        state.push_entries(vec![BinEntry::default(); 3]);
        state.select_row(None);

        state.select_previous_row();

        assert_eq!(state.table_state.selected(), Some(0));
    }

    #[test]
    fn select_next_row() {
        let mut state = state();
        state.push_entries(vec![BinEntry::default(); 3]);
        state.select_row(Some(1));

        state.select_next_row();

        assert_eq!(state.table_state.selected(), Some(2));
    }

    #[test]
    fn select_next_row_none() {
        let mut state = state();
        state.push_entries(vec![BinEntry::default(); 3]);
        state.select_row(None);

        state.select_next_row();

        assert_eq!(state.table_state.selected(), Some(1));
    }
}
