use crate::fs::tree::FsEntry;
use getset::{CopyGetters, Getters};
use ratatui::layout::Rect;
use ratatui::widgets::TableState;
use std::cmp::Reverse;
use std::iter::once;
use std::path::PathBuf;
use std::time::Instant;

#[derive(CopyGetters, Default, Getters)]
pub struct TreeState {
    pub path: PathBuf,
    #[getset(get = "pub")]
    rows: Vec<TreeRow>,
    #[getset(get_copy = "pub")]
    total_size: u64,
    #[getset(get = "pub")]
    table_state: TableState,
    pub table_area: Rect,
    pub scrollbar_area: Rect,
    pub last_click: Option<(usize, Instant)>,
}

impl TreeState {
    pub fn new(path: PathBuf) -> Self {
        TreeState {
            path,
            rows: vec![TreeRow::Parent],
            ..Default::default()
        }
    }

    pub fn entries_len(&self) -> usize {
        self.rows.len() - 1
    }

    /// Exposes table state mutably for rendering.
    ///
    /// # Warning
    /// Do not modify this state manually outside of rendering.
    pub fn table_state_mut(&mut self) -> &mut TableState {
        &mut self.table_state
    }

    pub fn entries(&self) -> impl Iterator<Item = &FsEntry> {
        self.rows.iter().filter_map(|row| match row {
            TreeRow::Entry(e) => Some(e),
            TreeRow::Parent => None,
        })
    }

    pub fn selected_entry(&self) -> Option<&FsEntry> {
        match self.rows.get(self.table_state.selected()?)? {
            TreeRow::Parent => None,
            TreeRow::Entry(e) => Some(e),
        }
    }

    pub fn set_rows(&mut self, mut entries: Vec<FsEntry>) {
        entries.sort_unstable_by_key(|e| Reverse(e.size()));
        self.total_size = entries.iter().filter_map(|e| e.size()).sum();
        self.rows = once(TreeRow::Parent)
            .chain(entries.into_iter().map(TreeRow::Entry))
            .collect();
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

#[derive(Debug, Eq, PartialEq)]
pub enum TreeRow {
    Parent,
    Entry(FsEntry),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> TreeState {
        TreeState::new(PathBuf::new())
    }

    fn fs_entry(size: Option<u64>) -> FsEntry {
        FsEntry::file("file".into(), size, None, None)
    }

    #[test]
    fn entries_len() {
        let mut state = state();
        state.set_rows(vec![fs_entry(None); 3]);

        let entries_len = state.entries_len();

        assert_eq!(entries_len, 3);
    }

    #[test]
    fn selected_entry() {
        let mut state = state();
        state.set_rows(vec![fs_entry(Some(1)), fs_entry(Some(2))]);
        state.select_row(Some(1));

        let entry = state.selected_entry();

        assert_eq!(entry.map(|e| e.size()), Some(Some(2)));
    }

    #[test]
    fn selected_entry_without_selected() {
        let state = state();
        let entry = state.selected_entry();
        assert!(entry.is_none());
    }

    #[test]
    fn set_rows() {
        let mut state = state();
        let entries = vec![fs_entry(Some(1)), fs_entry(None), fs_entry(Some(2))];

        state.set_rows(entries);

        assert_eq!(state.total_size(), 3);
        assert_eq!(state.rows().first(), Some(&TreeRow::Parent));
        assert_eq!(
            state.entries().map(|e| e.size()).collect::<Vec<_>>(),
            vec![Some(2), Some(1), None]
        );
    }

    #[test]
    fn select_row() {
        let mut state = state();
        state.set_rows(vec![fs_entry(None); 2]);

        assert!(state.select_row(Some(1)));
        assert_eq!(state.table_state().selected(), Some(1));

        assert!(state.select_row(None));
        assert_eq!(state.table_state().selected(), None);
    }

    #[test]
    fn select_row_out_of_bounds() {
        let mut state = state();
        state.set_rows(vec![fs_entry(None)]);

        assert!(!state.select_row(Some(5)));
        assert_eq!(state.table_state().selected(), None);
    }

    #[test]
    fn select_row_empty() {
        let mut state = state();

        assert!(state.select_row(Some(0)));
        assert_eq!(state.table_state().selected(), Some(0));

        assert!(!state.select_row(Some(1)));
        assert_eq!(state.table_state().selected(), Some(0));

        assert!(state.select_row(None));
        assert_eq!(state.table_state().selected(), None);
    }

    #[test]
    fn select_previous_row() {
        let mut state = state();
        state.set_rows(vec![fs_entry(None); 3]);
        state.select_row(Some(2));

        state.select_previous_row();

        assert_eq!(state.table_state.selected(), Some(1));
    }

    #[test]
    fn select_previous_row_none() {
        let mut state = state();
        state.set_rows(vec![fs_entry(None); 3]);
        state.select_row(None);

        state.select_previous_row();

        assert_eq!(state.table_state.selected(), Some(0));
    }

    #[test]
    fn select_next_row() {
        let mut state = state();
        state.set_rows(vec![fs_entry(None); 3]);
        state.select_row(Some(1));

        state.select_next_row();

        assert_eq!(state.table_state.selected(), Some(2));
    }

    #[test]
    fn select_next_row_none() {
        let mut state = state();
        state.set_rows(vec![fs_entry(None); 3]);
        state.select_row(None);

        state.select_next_row();

        assert_eq!(state.table_state.selected(), Some(1));
    }
}
