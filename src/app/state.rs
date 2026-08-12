pub mod bin;
pub mod tree;

use crate::app::state::bin::{BinEntry, BinState};
use crate::app::state::tree::{TreeRow, TreeState};
use crate::config::Config;
use crate::fs::scanner::FsScanner;
use getset::{CopyGetters, Getters};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use strum::{Display, EnumString};
use tracing::warn;

#[derive(CopyGetters, Getters)]
pub struct State {
    pub config: Config,
    pub running: bool,
    pub fs_scanner: FsScanner,
    #[getset(get_copy = "pub")]
    focus: Option<Component>,
    #[getset(get = "pub")]
    active: HashSet<Component>,
    pub tree: TreeState,
    pub bin: BinState,
    pub overlay: Overlay,
}

impl State {
    pub fn new(config: Config, dir: PathBuf, fs_scanner: FsScanner) -> Self {
        let active = config
            .state
            .active
            .iter()
            .filter_map(|s| s.parse().ok())
            .collect();

        Self {
            config,
            running: true,
            fs_scanner,
            focus: Some(Component::Tree),
            active,
            tree: TreeState::new(dir),
            bin: BinState::default(),
            overlay: Overlay::default(),
        }
    }

    pub fn sync_config(&mut self) {
        self.config.state.active = self.active.iter().map(ToString::to_string).collect();
        self.config.save();
    }

    pub fn update_tree(&mut self) {
        let fs_tree = self.fs_scanner.read_tree();
        let entries = fs_tree
            .entries_at(&self.tree.path)
            .into_iter()
            .cloned()
            .collect();
        self.tree.set_rows(entries);
        self.tree.select_row(Some(0));
    }

    pub fn set_focus(&mut self, component: Component) {
        if self.active.contains(&component) && component.is_interactive() {
            self.focus = Some(component);
        }
    }

    pub fn focus_next(&mut self, allow_none: bool) {
        self.focus = match self.focus {
            Some(Component::Tree) if self.active.contains(&Component::Bin) => Some(Component::Bin),
            Some(Component::Bin) if self.active.contains(&Component::Tree) => Some(Component::Tree),
            _ if allow_none => None,
            _ => return,
        };
    }

    pub fn toggle(&mut self, component: Component) {
        if !self.active.remove(&component) {
            if component.is_interactive() && !self.active.iter().any(|a| a.is_interactive()) {
                self.focus = Some(component);
            }
            self.active.insert(component);
        } else if self.focus == Some(component) {
            self.focus_next(true);
        }
    }

    pub fn trash_selected(&mut self) {
        let Some(fs_entry) = self.tree.selected_entry() else {
            return;
        };
        let entry_path = self.tree.path.join(fs_entry.name());
        if self.bin.contains_path(&entry_path) {
            return;
        }
        let file_count = self.fs_scanner.read_tree().count_files(&entry_path);
        self.bin
            .push_entry(BinEntry::from_entry_at(fs_entry, entry_path, file_count));
    }

    pub fn restore_selected_tree(&mut self) {
        let Some(fs_entry) = self.tree.selected_entry() else {
            return;
        };
        let entry_path = self.tree.path.join(fs_entry.name());
        self.bin.restore_by_path(&entry_path)
    }

    pub fn confirm_clear(&mut self) {
        if self.bin.rows().is_empty() {
            self.overlay = Overlay::Alert(AlertKind::ClearEmptyBin);
        } else {
            self.overlay = Overlay::Confirm(ConfirmKind::Clear)
        }
    }

    pub fn clear_bin(&mut self) {
        let cleared = self.bin.clear();

        self.fs_scanner
            .write_tree()
            .remove_entries(cleared.iter().map(|e| e.path.as_path()));
        self.update_tree();

        if self.bin.rows().is_empty() {
            self.overlay = Overlay::None;
        } else {
            self.overlay = Overlay::Alert(AlertKind::IncompleteClear);
        }
    }

    pub fn navigate_into(&mut self, idx: usize) {
        if idx == 0 {
            self.navigate_up();
        } else {
            self.navigate_down(idx);
        }
    }

    pub fn navigate_to(&mut self, path: PathBuf) {
        self.tree.path = path.clone();

        if self.fs_scanner.read_tree().is_scanned(&path) {
            self.update_tree();
        } else {
            if !self.fs_scanner.scan(&path) {
                warn!("already scanning, ignoring navigate to {:?}", path);
            }
        }
    }

    fn navigate_up(&mut self) {
        if let Some(parent) = self.tree.path.parent().map(Path::to_path_buf) {
            self.navigate_to(parent);
        }
    }

    fn navigate_down(&mut self, index: usize) {
        if let Some(TreeRow::Entry(entry)) = self.tree.rows().get(index)
            && entry.is_dir()
        {
            self.navigate_to(self.tree.path.join(entry.name()));
        }
    }
}

#[derive(Clone, Copy, Debug, Display, EnumString, Eq, Hash, PartialEq)]
#[strum(serialize_all = "snake_case")]
pub enum Component {
    Tree,
    Pie,
    Bin,
}

impl Component {
    pub fn is_interactive(&self) -> bool {
        matches!(self, Self::Tree | Self::Bin)
    }
}

#[derive(Debug, Default, Eq, PartialEq)]
pub enum Overlay {
    #[default]
    None,
    Help,
    Confirm(ConfirmKind),
    Alert(AlertKind),
}

impl Overlay {
    pub fn toggle(&mut self, overlay: Overlay) {
        if *self == overlay {
            *self = Overlay::None;
        } else {
            *self = overlay;
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum ConfirmKind {
    Clear,
}

#[derive(Debug, Eq, PartialEq)]
pub enum AlertKind {
    IncompleteClear,
    ClearEmptyBin,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fs::tree::{FsEntry, populated_tree};
    use Component::*;
    use rstest::rstest;
    use std::fs;
    use std::sync::mpsc;
    use std::time::Duration;
    use tempfile::tempdir;

    fn state(path: PathBuf) -> State {
        let (tx, _rx) = mpsc::channel();
        State::new(Config::default(), path, FsScanner::new(tx))
    }

    macro_rules! set {
        () => { HashSet::new() };
        ($($x:expr),+ $(,)?) => { HashSet::from([$($x),+]) };
    }

    #[test]
    fn update_tree() {
        let mut state = state(PathBuf::from("/home"));
        *state.fs_scanner.write_tree() = populated_tree();

        state.update_tree();

        assert_eq!(state.tree.entries_len(), 2);
        assert_eq!(state.tree.table_state().selected(), Some(0));
    }

    #[rstest]
    #[case(set![Tree], Tree, Some(Tree))]
    #[case(set![], Tree, None)]
    #[case(set![Pie], Pie, None)]
    fn set_focus(
        #[case] active: HashSet<Component>,
        #[case] component: Component,
        #[case] expected: Option<Component>,
    ) {
        let mut state = state(PathBuf::default());
        state.focus = None;
        state.active = active;

        state.set_focus(component);

        assert_eq!(state.focus, expected);
    }

    #[rstest]
    #[case(Some(Tree), set![Bin, Pie], false, Some(Bin))]
    #[case(Some(Bin), set![Tree], false, Some(Tree))]
    #[case(Some(Tree), set![Pie], true, None)]
    #[case(Some(Tree), set![], false, Some(Tree))]
    fn focus_next(
        #[case] focus: Option<Component>,
        #[case] active: HashSet<Component>,
        #[case] allow_none: bool,
        #[case] expected: Option<Component>,
    ) {
        let mut state = state(PathBuf::default());
        state.focus = focus;
        state.active = active;

        state.focus_next(allow_none);

        assert_eq!(state.focus, expected);
    }

    #[rstest]
    #[case(None, set![], Tree, Some(Tree), set![Tree])]
    #[case(Some(Bin), set![Bin], Tree, Some(Bin), set![Bin, Tree])]
    #[case(None, set![], Pie, None, set![Pie])]
    #[case(Some(Tree), set![Tree, Bin], Bin, Some(Tree), set![Tree])]
    #[case(Some(Tree), set![Tree, Bin], Tree, Some(Bin), set![Bin])]
    #[case(Some(Tree), set![Tree], Tree, None, set![])]
    fn toggle(
        #[case] focus: Option<Component>,
        #[case] active: HashSet<Component>,
        #[case] component: Component,
        #[case] expected_focus: Option<Component>,
        #[case] expected_active: HashSet<Component>,
    ) {
        let mut state = state(PathBuf::default());
        state.focus = focus;
        state.active = active;

        state.toggle(component);

        assert_eq!(state.focus, expected_focus);
        assert_eq!(state.active, expected_active);
    }

    #[rstest]
    #[case(None, None)]
    #[case(Some(0), None)]
    #[case(Some(1), Some(PathBuf::from("/home/user/cat.png")))]
    fn trash_selected(#[case] select: Option<usize>, #[case] expected: Option<PathBuf>) {
        let mut state = state(PathBuf::from("/home/user"));
        *state.fs_scanner.write_tree() = populated_tree();
        state.update_tree();
        state.tree.select_row(select);

        state.trash_selected();

        match expected {
            Some(path) => assert_eq!(state.bin.rows()[0].path, path),
            None => assert!(state.bin.rows().is_empty()),
        }
    }

    #[test]
    fn trash_selected_skips_duplicate() {
        let mut state = state(PathBuf::from("/home/user"));
        *state.fs_scanner.write_tree() = populated_tree();
        state.update_tree();
        state.tree.select_row(Some(1));
        state.trash_selected();

        state.trash_selected();

        assert_eq!(state.bin.rows().len(), 1);
    }

    #[rstest]
    #[case(None, 1)]
    #[case(Some(0), 1)]
    #[case(Some(1), 0)]
    fn restore_selected_tree(#[case] select: Option<usize>, #[case] expected: usize) {
        let mut state = state(PathBuf::from("/home/user"));
        *state.fs_scanner.write_tree() = populated_tree();
        state.update_tree();
        state.tree.select_row(Some(1));
        state.trash_selected();
        state.tree.select_row(select);

        state.restore_selected_tree();

        assert_eq!(state.bin.rows().len(), expected);
    }

    #[rstest]
    #[case(vec![], Overlay::Alert(AlertKind::ClearEmptyBin))]
    #[case(vec![BinEntry::default()], Overlay::Confirm(ConfirmKind::Clear))]
    fn confirm_clear(#[case] bin_rows: Vec<BinEntry>, #[case] expected: Overlay) {
        let mut state = state(PathBuf::from("/"));
        state.bin.push_entries(bin_rows);

        state.confirm_clear();

        assert_eq!(state.overlay, expected);
    }

    #[test]
    fn clear_bin_success() {
        // Given
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("file.txt");
        fs::File::create(&file_path).unwrap();

        let mut state = state(dir.path().to_path_buf());
        {
            let mut tree = state.fs_scanner.write_tree();
            let root_id = tree.ensure_dir(dir.path());
            tree.mark_scanned(root_id);
            tree.add_entry(
                FsEntry::file("file.txt".into(), None, None, None),
                dir.path(),
            );
        }
        state.update_tree();
        state.tree.select_row(Some(1));
        state.trash_selected();

        // When
        state.clear_bin();

        // Then
        assert!(!file_path.exists());
        assert_eq!(state.tree.entries_len(), 0);
        assert_eq!(state.overlay, Overlay::None);
    }

    #[test]
    fn clear_bin_incomplete() {
        let mut state = state(PathBuf::from("/home/user"));
        *state.fs_scanner.write_tree() = populated_tree();
        state.update_tree();
        state.tree.select_row(Some(1)); // cat.png
        state.trash_selected();

        state.clear_bin();

        assert_eq!(state.overlay, Overlay::Alert(AlertKind::IncompleteClear));
    }

    #[rstest]
    #[case(PathBuf::from("/home/guest/empty"), 0, PathBuf::from("/home/guest"))]
    #[case(PathBuf::from("/home/guest"), 1, PathBuf::from("/home/guest"))]
    #[case(PathBuf::from("/home/guest"), 2, PathBuf::from("/home/guest/empty"))]
    #[case(PathBuf::from("/home/guest"), 99, PathBuf::from("/home/guest"))]
    fn navigate_into(#[case] start: PathBuf, #[case] idx: usize, #[case] expected: PathBuf) {
        let mut state = state(start);
        *state.fs_scanner.write_tree() = populated_tree();
        state.update_tree();

        state.navigate_into(idx);

        assert_eq!(state.tree.path, expected);
    }

    #[test]
    fn navigate_to_scanned_path() {
        let mut state = state(PathBuf::from("/home"));
        *state.fs_scanner.write_tree() = populated_tree();

        state.navigate_to(PathBuf::from("/home/guest"));

        assert_eq!(state.tree.path, PathBuf::from("/home/guest"));
        assert_eq!(state.tree.entries_len(), 2);
    }

    #[test]
    fn navigate_to_unscanned_path_triggers_scan() {
        // Given
        let dir = tempdir().unwrap();
        fs::File::create(dir.path().join("file.txt")).unwrap();

        let (tx, rx) = mpsc::channel();
        let mut state = State::new(Config::default(), PathBuf::new(), FsScanner::new(tx));

        // When
        state.navigate_to(dir.path().to_path_buf());
        rx.recv_timeout(Duration::from_secs(5))
            .expect("scan should complete");
        state.update_tree();

        // Then
        assert_eq!(state.tree.path, dir.path());
        assert_eq!(state.tree.entries_len(), 1);
    }
}
