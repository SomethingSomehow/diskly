use ansi_colours::ansi256_from_rgb;
use ratatui::prelude::Modifier;
use ratatui::style::{Color, Style};
use serde::{Deserialize, Deserializer};
use std::sync::OnceLock;

pub static TRUE_COLOR: OnceLock<bool> = OnceLock::new();

#[derive(Deserialize)]
pub struct ThemeConfig {
    #[serde(deserialize_with = "de_style")]
    pub background: Style,
    #[serde(deserialize_with = "de_style")]
    pub tree_border: Style,
    #[serde(deserialize_with = "de_style")]
    pub pie_border: Style,
    #[serde(deserialize_with = "de_style")]
    pub bin_border: Style,
    #[serde(deserialize_with = "de_style")]
    pub overlay_border: Style,
    #[serde(deserialize_with = "de_style")]
    pub hint: Style,
    #[serde(deserialize_with = "de_style")]
    pub inactive_hint: Style,
    #[serde(deserialize_with = "de_style")]
    pub title: Style,
    #[serde(deserialize_with = "de_style")]
    pub inactive_title: Style,
    #[serde(deserialize_with = "de_style")]
    pub text: Style,
    #[serde(deserialize_with = "de_style")]
    pub inactive_text: Style,
    #[serde(deserialize_with = "de_style")]
    pub row_highlight: Style,
    #[serde(deserialize_with = "de_style")]
    pub filled_bar: Style,
    #[serde(deserialize_with = "de_style")]
    pub empty_bar: Style,
    #[serde(deserialize_with = "de_style")]
    pub scroll_track: Style,
    #[serde(deserialize_with = "de_style")]
    pub logo_from: Style,
    #[serde(deserialize_with = "de_style")]
    pub logo_to: Style,
    #[serde(deserialize_with = "de_style")]
    pub version: Style,
    #[serde(deserialize_with = "de_style")]
    pub dim: Style,
}

impl ThemeConfig {
    pub fn get_hint(&self, focused: bool) -> Style {
        if focused {
            self.hint
        } else {
            self.inactive_hint
        }
    }
    pub fn get_title(&self, focused: bool) -> Style {
        if focused {
            self.title
        } else {
            self.inactive_title
        }
    }
    pub fn get_text(&self, focused: bool) -> Style {
        if focused {
            self.text
        } else {
            self.inactive_text
        }
    }
}
#[derive(Deserialize)]
struct StyleRaw {
    fg: Option<[u8; 3]>,
    bg: Option<[u8; 3]>,
    underline_color: Option<[u8; 3]>,
    #[serde(default)]
    modifiers: Vec<String>,
}

fn de_style<'de, D>(d: D) -> Result<Style, D::Error>
where
    D: Deserializer<'de>,
{
    let raw = StyleRaw::deserialize(d)?;
    let true_color = *TRUE_COLOR.get().unwrap_or(&true);
    let color = |c: [u8; 3]| {
        if true_color {
            Color::from(c)
        } else {
            Color::Indexed(ansi256_from_rgb(c))
        }
    };

    let mut style = Style::default();

    if let Some(c) = raw.fg {
        style = style.fg(color(c));
    }
    if let Some(c) = raw.bg {
        style = style.bg(color(c));
    }
    if let Some(c) = raw.underline_color {
        style = style.underline_color(color(c));
    }

    for modifier_name in &raw.modifiers {
        if let Some(modifier) = Modifier::from_name(&modifier_name.to_uppercase()) {
            style = style.add_modifier(modifier)
        }
    }

    Ok(style)
}
