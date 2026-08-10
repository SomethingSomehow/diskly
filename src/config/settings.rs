use serde::Deserialize;

#[derive(Deserialize)]
pub struct SettingsConfig {
    pub true_color: bool,
}
