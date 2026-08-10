use crate::app::state::{AlertKind, Component, ConfirmKind, Overlay, State};
use crossterm::event::Event as CrosstermEvent;
use crossterm::event::Event::{Key, Mouse};
use crossterm::event::{
    KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::layout::Rect;
use std::time::{Duration, Instant};

const DOUBLE_CLICK_TIMEOUT: Duration = Duration::from_millis(400);

pub enum Command {
    Quit,
    Help,
    FocusNext,
    HideOverlay,
    Toggle(Component),
    Tree(TreeCommand),
    Bin(BinCommand),
    Nothing,
}

pub enum TreeCommand {
    Up,
    Down,
    Select(usize),
    ScrollTo(usize),
    Enter(Option<usize>),
    Trash,
    Restore,
}

pub enum BinCommand {
    Up,
    Down,
    Select(usize),
    ScrollTo(usize),
    Restore,
    Clear,
    ClearConfirmed,
}

impl Command {
    pub fn from_event(event: CrosstermEvent, state: &mut State) -> Self {
        match event {
            Key(e) => Self::parse_key_event(e, state),
            Mouse(e) => Self::parse_mouse_event(e, state),
            _ => Self::Nothing,
        }
    }

    fn parse_key_event(event: KeyEvent, state: &State) -> Self {
        if event.kind != KeyEventKind::Press {
            return Self::Nothing;
        }

        match state.overlay {
            Overlay::Confirm(ConfirmKind::Clear) => {
                return match event.code {
                    KeyCode::Char('y' | 'Y') => Self::Bin(BinCommand::ClearConfirmed),
                    KeyCode::Char('n' | 'N') | KeyCode::Esc => Self::HideOverlay,
                    _ => Self::Nothing,
                };
            }
            Overlay::Alert(AlertKind::IncompleteClear) => {
                return match event.code {
                    KeyCode::Enter | KeyCode::Esc => Self::HideOverlay,
                    _ => Self::Nothing,
                };
            }
            _ => {}
        };

        let focus = &state.focus();
        let ctrl = event.modifiers.contains(KeyModifiers::CONTROL);
        match event.code {
            KeyCode::Char('q') | KeyCode::Char('Q') => Self::Quit,
            KeyCode::Char('c') if ctrl => Self::Quit,
            KeyCode::F(1) | KeyCode::Char('h' | 'H') => Self::Help,
            KeyCode::Tab => Self::FocusNext,
            KeyCode::Char('1') => Self::Toggle(Component::Tree),
            KeyCode::Char('2') => Self::Toggle(Component::Pie),
            KeyCode::Char('3') => Self::Toggle(Component::Bin),
            KeyCode::Up | KeyCode::Char('w' | 'W' | 'k' | 'K') => match focus {
                Some(Component::Tree) => Self::Tree(TreeCommand::Up),
                Some(Component::Bin) => Self::Bin(BinCommand::Up),
                _ => Self::Nothing,
            },
            KeyCode::Down | KeyCode::Char('s' | 'S' | 'j' | 'J') => match focus {
                Some(Component::Tree) => Self::Tree(TreeCommand::Down),
                Some(Component::Bin) => Self::Bin(BinCommand::Down),
                _ => Self::Nothing,
            },
            KeyCode::Enter => match focus {
                Some(Component::Tree) => {
                    Self::Tree(TreeCommand::Enter(state.tree.table_state().selected()))
                }
                _ => Self::Nothing,
            },
            KeyCode::Char('t' | 'T') => match focus {
                Some(Component::Tree) => Self::Tree(TreeCommand::Trash),
                _ => Self::Nothing,
            },
            KeyCode::Char('r' | 'R') => match focus {
                Some(Component::Tree) => Self::Tree(TreeCommand::Restore),
                Some(Component::Bin) => Self::Bin(BinCommand::Restore),
                _ => Self::Nothing,
            },
            KeyCode::Char('c' | 'C') => match focus {
                Some(Component::Tree) | Some(Component::Bin) => Self::Bin(BinCommand::Clear),
                _ => Self::Nothing,
            },
            _ => Self::Nothing,
        }
    }

    fn parse_mouse_event(event: MouseEvent, state: &mut State) -> Self {
        match event.kind {
            MouseEventKind::Down(MouseButton::Left) | MouseEventKind::Drag(MouseButton::Left) => {
                if let Some(cmd) = Self::hit_tree(state, event.column, event.row) {
                    state.set_focus(Component::Tree);
                    return Self::Tree(cmd);
                }
                if let Some(cmd) = Self::hit_bin(state, event.column, event.row) {
                    state.set_focus(Component::Bin);
                    return Self::Bin(cmd);
                }
                Self::Nothing
            }
            MouseEventKind::ScrollUp => match state.focus() {
                Some(Component::Tree) => Self::Tree(TreeCommand::Up),
                Some(Component::Bin) => Self::Bin(BinCommand::Up),
                _ => Self::Nothing,
            },
            MouseEventKind::ScrollDown => match state.focus() {
                Some(Component::Tree) => Self::Tree(TreeCommand::Down),
                Some(Component::Bin) => Self::Bin(BinCommand::Down),
                _ => Self::Nothing,
            },
            _ => Self::Nothing,
        }
    }

    fn hit_tree(state: &mut State, col: u16, row: u16) -> Option<TreeCommand> {
        let len = state.tree.rows().len();

        if let Some(index) = Self::scrollbar_hit(state.tree.scrollbar_area, len, col, row) {
            return Some(TreeCommand::ScrollTo(index));
        }

        let offset = state.tree.table_state().offset();
        let idx = Self::list_item_hit(state.tree.table_area, offset, len, col, row)?;

        let is_double = state
            .tree
            .last_click
            .is_some_and(|(last_idx, t)| last_idx == idx && t.elapsed() < DOUBLE_CLICK_TIMEOUT);

        if is_double {
            state.tree.last_click = None;
            Some(TreeCommand::Enter(Some(idx)))
        } else {
            state.tree.last_click = Some((idx, Instant::now()));
            Some(TreeCommand::Select(idx))
        }
    }

    fn hit_bin(state: &State, col: u16, row: u16) -> Option<BinCommand> {
        let len = state.bin.rows().len();

        if let Some(index) = Self::scrollbar_hit(state.bin.scrollbar_area, len, col, row) {
            return Some(BinCommand::ScrollTo(index));
        }

        let offset = state.bin.table_state().offset();
        let idx = Self::list_item_hit(state.bin.table_area, offset, len, col, row)?;
        Some(BinCommand::Select(idx))
    }

    fn scrollbar_hit(area: Rect, len: usize, col: u16, row: u16) -> Option<usize> {
        if len == 0 || col != area.x || row < area.top() || row >= area.bottom() {
            return None;
        }
        Some((row - area.y) as usize * len / area.height as usize)
    }

    fn list_item_hit(area: Rect, offset: usize, len: usize, col: u16, row: u16) -> Option<usize> {
        let content_top = area.y + 1; // skip header row
        if col < area.left() || col >= area.right() || row < content_top || row >= area.bottom() {
            return None;
        }
        let idx = offset + (row - content_top) as usize;
        (idx < len).then_some(idx)
    }
}
