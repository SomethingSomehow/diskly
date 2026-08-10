pub mod settings;
pub mod state;
pub mod theme;

use crate::config::settings::SettingsConfig;
use crate::config::state::StateConfig;
use crate::config::theme::{TRUE_COLOR, ThemeConfig};
use directories::ProjectDirs;
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::fs;
use std::path::{Path, PathBuf};

const DEFAULT_APP_CONFIG: &str = include_str!("../config/settings.toml");
const DEFAULT_THEME: &str = include_str!("../config/theme.toml");
const DEFAULT_STATE: &str = include_str!("../config/state.toml");

pub struct Config {
    pub theme: ThemeConfig,
    pub settings: SettingsConfig,
    pub state: StateConfig,
}

impl Config {
    pub fn load() -> Self {
        let settings: SettingsConfig = load("settings.toml", DEFAULT_APP_CONFIG);
        TRUE_COLOR.set(settings.true_color).ok();
        let theme = load("theme.toml", DEFAULT_THEME);
        let state = load("state.toml", DEFAULT_STATE);
        Self {
            theme,
            settings,
            state,
        }
    }

    pub fn save(&self) {
        save("state.toml", &self.state);
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            theme: toml::from_str(DEFAULT_THEME).expect("embedded default theme config is invalid"),
            settings: toml::from_str(DEFAULT_APP_CONFIG)
                .expect("embedded default settings config is invalid"),
            state: toml::from_str(DEFAULT_STATE).expect("embedded default state config is invalid"),
        }
    }
}

pub fn config_dir() -> Option<PathBuf> {
    ProjectDirs::from("", "", "diskly").map(|dirs| dirs.config_dir().to_path_buf())
}

pub fn cache_dir() -> Option<PathBuf> {
    ProjectDirs::from("", "", "diskly").map(|dirs| dirs.cache_dir().to_path_buf())
}

fn load<T: DeserializeOwned>(filename: &str, default: &str) -> T {
    let path = config_dir().map(|dir| dir.join(filename));
    let content = path.as_deref().and_then(|p| {
        if !p.exists() {
            create_default(p, default);
        }
        fs::read_to_string(p).ok()
    });
    let str = content.as_deref().unwrap_or(default);
    toml::from_str(str).unwrap_or_else(|_| {
        toml::from_str(default)
            .unwrap_or_else(|e| panic!("embedded default {filename:?} config is invalid: {e}"))
    })
}

pub fn save<T: Serialize>(filename: &str, value: &T) {
    let Some(dir) = config_dir() else { return };
    if let Err(e) = fs::create_dir_all(&dir) {
        return tracing::warn!("failed to create config dir {dir:?}: {e}");
    }
    let path = dir.join(filename);
    match toml::to_string_pretty(value) {
        Ok(s) => {
            fs::write(&path, s).unwrap_or_else(|e| tracing::warn!("failed to save {filename}: {e}"))
        }
        Err(e) => tracing::warn!("failed to serialize {filename}: {e}"),
    }
}

fn create_default(path: &Path, default: &str) {
    let Some(parent) = path.parent() else { return };
    if let Err(e) = fs::create_dir_all(parent).and_then(|_| fs::write(path, default)) {
        tracing::warn!("failed to create default config at {path:?}: {e}");
    }
}
