use crate::app::command::{BinCommand, Command, TreeCommand};
use crate::app::state::{Overlay, State};
use crate::app::tui::render;
use crate::config::Config;
use crate::fs::scanner::FsScanner;
use color_eyre::Result;
use crossterm::event::Event as CrosstermEvent;
use ratatui::DefaultTerminal;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

pub struct App {
    terminal: DefaultTerminal,
    events: EventHandler,
    state: State,
}

impl App {
    pub fn new(terminal: DefaultTerminal, config: Config, dir: PathBuf) -> Self {
        let events = EventHandler::new();
        let fs_scanner = FsScanner::new(events.sender());
        let mut state = State::new(config, dir.clone(), fs_scanner);
        state.navigate_to(dir);
        Self {
            terminal,
            events,
            state,
        }
    }

    pub fn run(mut self) -> Result<()> {
        const SCANNING_TIMEOUT: Duration = Duration::from_millis(100);

        while self.state.running {
            self.terminal.draw(|frame| render(&mut self.state, frame))?;

            let scanning = self.state.fs_scanner.scanning.load(Ordering::Relaxed);
            let timeout = scanning.then_some(SCANNING_TIMEOUT);

            if let Some(event) = self.events.next(timeout)? {
                self.handle_event(event);

                while let Ok(event) = self.events.try_next() {
                    self.handle_event(event);
                }
            }
        }

        self.state.sync_config();

        Ok(())
    }

    fn handle_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::Crossterm(e) => {
                let command = Command::from_event(e, &mut self.state);
                Self::handle_command(&mut self.state, command)
            }
            AppEvent::ScanCompleted => {
                self.state.update_tree();
            }
        }
    }

    fn handle_command(state: &mut State, command: Command) {
        match command {
            Command::Quit => state.running = false,
            Command::Help => state.overlay.toggle(Overlay::Help),
            Command::FocusNext => state.focus_next(false),
            Command::HideOverlay => state.overlay = Overlay::None,
            Command::Toggle(c) => state.toggle(c),
            Command::Tree(c) => Self::handle_tree(state, c),
            Command::Bin(c) => Self::handle_bin(state, c),
            Command::Nothing => {}
        }
    }

    fn handle_tree(state: &mut State, command: TreeCommand) {
        if state.fs_scanner.scanning.load(Ordering::Relaxed) {
            return;
        }

        match command {
            TreeCommand::Up => {
                state.tree.select_previous_row();
            }
            TreeCommand::Down => {
                state.tree.select_next_row();
            }
            TreeCommand::Select(idx) => {
                state.tree.select_row(Some(idx));
            }
            TreeCommand::ScrollTo(idx) => {
                state.tree.select_row(Some(idx));
            }
            TreeCommand::Enter(idx) => {
                if let Some(idx) = idx {
                    state.navigate_into(idx)
                }
            }
            TreeCommand::Trash => state.trash_selected(),
            TreeCommand::Restore => state.restore_selected_tree(),
        }
    }

    fn handle_bin(state: &mut State, command: BinCommand) {
        match command {
            BinCommand::Up => {
                state.bin.select_previous_row();
            }
            BinCommand::Down => {
                state.bin.select_next_row();
            }
            BinCommand::Select(idx) => {
                state.bin.select_row(Some(idx));
            }
            BinCommand::ScrollTo(idx) => {
                state.bin.select_row(Some(idx));
            }
            BinCommand::Restore => state.bin.restore_selected(),
            BinCommand::Clear => state.confirm_clear(),
            BinCommand::ClearConfirmed => state.clear_bin(),
        }
    }
}

pub struct EventHandler {
    receiver: mpsc::Receiver<AppEvent>,
    sender: mpsc::Sender<AppEvent>,
}

impl EventHandler {
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel();
        Self::subscribe_crossterm(sender.clone());
        Self { receiver, sender }
    }

    pub fn sender(&self) -> mpsc::Sender<AppEvent> {
        self.sender.clone()
    }

    pub fn next(&self, timeout: Option<Duration>) -> Result<Option<AppEvent>> {
        match timeout {
            Some(t) => match self.receiver.recv_timeout(t) {
                Ok(event) => Ok(Some(event)),
                Err(mpsc::RecvTimeoutError::Timeout) => Ok(None),
                Err(e) => Err(e.into()),
            },
            None => Ok(Some(self.receiver.recv()?)),
        }
    }

    pub fn try_next(&self) -> Result<AppEvent> {
        Ok(self.receiver.try_recv()?)
    }

    fn subscribe_crossterm(sender: mpsc::Sender<AppEvent>) {
        thread::spawn(move || {
            while let Ok(e) = crossterm::event::read() {
                if sender.send(AppEvent::Crossterm(e)).is_err() {
                    break;
                }
            }
        });
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AppEvent {
    Crossterm(CrosstermEvent),
    ScanCompleted,
}
