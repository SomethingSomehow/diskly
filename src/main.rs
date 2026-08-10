#![allow(clippy::new_without_default)]

use crate::app::core::App;
use crate::config::{Config, cache_dir};
use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::execute;
use std::env;
use std::fs::File;
use std::io::stdout;
use std::path::PathBuf;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::time::ChronoLocal;

pub mod app;
pub mod config;
pub mod fs;

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    init_logging()?;

    let config = Config::load();
    let dir = env::args()
        .nth(1)
        .map(PathBuf::from)
        .map(dunce::canonicalize)
        .unwrap_or_else(env::current_dir)?;
    let terminal = ratatui::init();
    let app = App::new(terminal, config, dir);

    if let Err(e) = execute!(stdout(), EnableMouseCapture) {
        tracing::warn!("failed to enable mouse capture: {e}");
    }
    let result = app.run();
    if let Err(e) = execute!(stdout(), DisableMouseCapture) {
        tracing::warn!("failed to disable mouse capture: {e}");
    }
    ratatui::restore();

    result
}

fn init_logging() -> color_eyre::Result<()> {
    let log_path = cache_dir().map(|dir| dir.join("diskly.log"));

    let log_file = match &log_path {
        Some(path) => {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            File::create(path)?
        }
        None => File::create("diskly.log")?,
    };

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_timer(ChronoLocal::new("%H:%M:%S".into()))
        .with_writer(log_file)
        .with_ansi(false)
        .init();

    Ok(())
}
