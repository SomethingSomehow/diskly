pub mod bin;
pub mod pie;
pub mod tree;

use crate::app::state::{AlertKind, Component, ConfirmKind, Overlay, State};
use crate::app::tui::bin::render_bin;
use crate::app::tui::pie::render_pie_chart;
use crate::app::tui::tree::render_tree;
use crate::config::theme::ThemeConfig;
use humansize::{DECIMAL, format_size};
use ratatui::layout::{Flex, Margin, Rect};
use ratatui::prelude::{Color, Line, Span, Style};
use ratatui::widgets::{Clear, Paragraph, Row, Table};
use ratatui::{
    Frame,
    layout::{Constraint, Layout},
    widgets::Block,
};
use std::iter::once;

const LOGO: [&str; 6] = [
    "██████╗ ██╗███████╗██╗  ██╗██╗  ██╗   ██╗",
    "██╔══██╗██║██╔════╝██║ ██╔╝██║  ╚██╗ ██╔╝",
    "██║  ██║██║███████╗█████╔╝ ██║   ╚████╔╝",
    "██║  ██║██║╚════██║██╔═██╗ ██║    ╚██╔╝",
    "██████╔╝██║███████║██║  ██╗███████╗██║",
    "╚═════╝ ╚═╝╚══════╝╚═╝  ╚═╝╚══════╝╚═╝",
];
const LOGO_HEIGHT: u16 = 7;
const LOGO_WIDTH: u16 = 41;

const MIN_TREE: (u16, u16) = (70, 10);
const MIN_PIE: (u16, u16) = (20, 10);
const MIN_BIN: (u16, u16) = (45, 10);

const SIZE_WIDTH: u16 = 10;

const HELP_KEY_LEN: u16 = 15;
const HELP_DESC_LEN: u16 = 50;
const HELP_WIDTH: u16 = HELP_KEY_LEN + HELP_DESC_LEN + 2;
const BINDINGS: &[(&str, &str)] = &[
    ("q, ctrl + c", "Quit program"),
    ("F1, h", "Show or hide help window"),
    ("Tab", "Move focus to next component"),
    ("1", "Toggle tree component"),
    ("2", "Toggle pie component"),
    ("3", "Toggle bin componen"),
    ("Click", "Select entry under cursor"),
    ("Double-click", "Open directory under cursor"),
    ("Up, w, k", "Select previous entry in focused list"),
    ("Down, s, j", "Select next entry in focused list"),
    ("Enter", "Open selected directory"),
    ("t", "Move selected entry to bin"),
    ("r", "Restore selected entry from bin"),
    ("c", "Clear all entries from bin"),
    ("y", "Confirm pending action"),
    ("n", "Cancel pending action"),
];

pub fn render(state: &mut State, frame: &mut Frame) {
    let area = frame.area();
    frame.render_widget(Block::default().style(state.config.theme.background), area);

    if state.active().is_empty() {
        render_empty(frame, state, area);
        return;
    }

    let show_tree = state.active().contains(&Component::Tree);
    let show_pie = state.active().contains(&Component::Pie);
    let show_bin = state.active().contains(&Component::Bin);

    let (min_w, min_h) = min_size(show_tree, show_pie, show_bin);
    if area.width < min_w || area.height < min_h {
        render_too_small(frame, state, area, min_w, min_h);
        return;
    }

    let show_right = show_pie || show_bin;

    let h_constraints: Vec<Constraint> = [
        show_tree.then_some(Constraint::Fill(3)),
        show_right.then_some(Constraint::Fill(2)),
    ]
    .into_iter()
    .flatten()
    .collect();

    let mut h_areas = area
        .layout_vec(&Layout::horizontal(h_constraints))
        .into_iter();

    if show_tree {
        render_tree(frame, state, h_areas.next().unwrap());
    }

    if show_right {
        let right = h_areas.next().unwrap();

        let v_constraints: Vec<Constraint> = [
            show_pie.then_some(Constraint::Fill(1)),
            show_bin.then_some(Constraint::Fill(1)),
        ]
        .into_iter()
        .flatten()
        .collect();

        let mut v_areas = right
            .layout_vec(&Layout::vertical(v_constraints))
            .into_iter();

        if show_pie {
            render_pie_chart(frame, state, v_areas.next().unwrap());
        }
        if show_bin {
            render_bin(frame, state, v_areas.next().unwrap());
        }
    }

    if state.overlay != Overlay::None {
        dim_area(frame, area, &state.config.theme);
    }

    match &state.overlay {
        Overlay::Help => render_help(frame, state, area),
        Overlay::Confirm(k) => render_confirm(frame, state, area, k),
        Overlay::Alert(k) => render_alert(frame, state, area, k),
        Overlay::None => {}
    }
}

fn render_empty(frame: &mut Frame, state: &State, area: Rect) {
    const TEXT_HEIGHT: u16 = 7;

    let [logo_row, text_row] = Layout::vertical([
        Constraint::Length(LOGO_HEIGHT),
        Constraint::Length(TEXT_HEIGHT),
    ])
    .flex(Flex::Center)
    .areas(area);

    let [logo_area] = Layout::horizontal([Constraint::Length(LOGO_WIDTH)])
        .flex(Flex::Center)
        .areas(logo_row);
    let [text_area] = Layout::horizontal([Constraint::Length(23)])
        .flex(Flex::Center)
        .areas(text_row);

    render_logo(frame, state, logo_area);

    frame.render_widget(
        Paragraph::new(vec![
            Line::styled(" No components shown!", state.config.theme.title),
            Line::from(""),
            Line::from(vec![
                Span::styled("1", state.config.theme.hint),
                Span::styled(" - show tree component", state.config.theme.text),
            ]),
            Line::from(vec![
                Span::styled("2", state.config.theme.hint),
                Span::styled(" - show pie component", state.config.theme.text),
            ]),
            Line::from(vec![
                Span::styled("3", state.config.theme.hint),
                Span::styled(" - show bin component", state.config.theme.text),
            ]),
            Line::from(vec![
                Span::styled("q", state.config.theme.hint),
                Span::styled(" - quit", state.config.theme.text),
            ]),
        ]),
        text_area,
    );
}

fn min_size(show_tree: bool, show_pie: bool, show_bin: bool) -> (u16, u16) {
    let mut right_w = 0;
    let mut right_h = 0;

    if show_pie {
        right_w = right_w.max(MIN_PIE.0);
        right_h += MIN_PIE.1;
    }
    if show_bin {
        right_w = right_w.max(MIN_BIN.0);
        right_h += MIN_BIN.1;
    }

    let tree_w = if show_tree { MIN_TREE.0 } else { 0 };

    (tree_w + right_w, MIN_TREE.1.max(right_h))
}

fn render_too_small(frame: &mut Frame, state: &State, area: Rect, min_w: u16, min_h: u16) {
    let [text_area] = Layout::vertical([Constraint::Length(5)])
        .flex(Flex::Center)
        .areas(area);

    let width = if area.width < min_w {
        Span::styled(area.width.to_string(), state.config.theme.hint)
    } else {
        Span::styled(area.width.to_string(), state.config.theme.title)
    };
    let height = if area.height < min_h {
        Span::styled(area.height.to_string(), state.config.theme.hint)
    } else {
        Span::styled(area.height.to_string(), state.config.theme.title)
    };

    frame.render_widget(
        Paragraph::new(vec![
            Line::styled("Terminal size too small:", state.config.theme.title),
            Line::from(vec![
                Span::styled("Width - ", state.config.theme.title),
                width,
                Span::styled(" Height - ", state.config.theme.title),
                height,
            ]),
            Line::from(""),
            Line::styled("Needed for current config:", state.config.theme.title),
            Line::styled(
                format!("Width - {min_w} Height - {min_h}"),
                state.config.theme.title,
            ),
        ])
        .style(state.config.theme.text)
        .centered(),
        text_area,
    );
}

fn render_help(frame: &mut Frame, state: &State, area: Rect) {
    let [_, logo_row, help_row, _] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(LOGO_HEIGHT),
        Constraint::Fill(1),
        Constraint::Length(1),
    ])
    .flex(Flex::Center)
    .areas(area);

    let [logo_area] = Layout::horizontal([Constraint::Length(LOGO_WIDTH)])
        .flex(Flex::Center)
        .areas(logo_row);
    let [help_area] = Layout::horizontal([Constraint::Length(HELP_WIDTH)])
        .flex(Flex::Center)
        .areas(help_row);
    let help_table_area = help_area.inner(Margin {
        horizontal: 1,
        vertical: 1,
    });

    render_logo(frame, state, logo_area);

    let block = Block::bordered()
        .border_style(state.config.theme.overlay_border)
        .title_top(Line::from(vec![
            Span::styled("─┐", state.config.theme.overlay_border),
            Span::styled("help", state.config.theme.title),
            Span::styled("┌", state.config.theme.overlay_border),
        ]))
        .style(state.config.theme.background);

    frame.render_widget(Clear, help_area);
    frame.render_widget(block, help_area);

    let header = Row::new([Line::raw("Key:").centered(), Line::raw("Description:")])
        .style(state.config.theme.title);

    let rows = BINDINGS.iter().map(|(key, desc)| {
        Row::new([
            Line::styled(*key, state.config.theme.hint).centered(),
            Line::styled(*desc, state.config.theme.text),
        ])
        .style(state.config.theme.hint)
    });

    let table = Table::new(
        rows,
        [
            Constraint::Length(HELP_KEY_LEN),
            Constraint::Length(HELP_DESC_LEN),
        ],
    )
    .header(header)
    .style(state.config.theme.text);

    frame.render_widget(table, help_table_area);
}

fn render_confirm(frame: &mut Frame, state: &State, area: Rect, kind: &ConfirmKind) {
    const CONFIRM_WIDTH: u16 = 50;

    let msg = match kind {
        ConfirmKind::Clear => Line::from(vec![
            Span::styled("Permanently delete ", state.config.theme.text),
            Span::styled(
                state.bin.total_files().to_string(),
                state.config.theme.title,
            ),
            Span::styled(" file(s) ", state.config.theme.text),
            Span::styled(
                format!("({})", format_size(state.bin.total_size(), DECIMAL)),
                state.config.theme.title,
            ),
            Span::styled("?", state.config.theme.text),
        ]),
    };

    let buttons = Line::from(vec![
        Span::styled("y", state.config.theme.hint),
        Span::styled("es ", state.config.theme.title),
        Span::raw("    "),
        Span::styled(" n", state.config.theme.hint),
        Span::styled("o", state.config.theme.title),
    ]);

    render_popup(
        frame,
        area,
        &state.config.theme,
        "clear bin",
        CONFIRM_WIDTH,
        &[msg, Line::from(""), buttons],
    );
}

fn render_alert(frame: &mut Frame, state: &State, area: Rect, kind: &AlertKind) {
    const ALERT_WIDTH: u16 = 50;

    let msg = match kind {
        AlertKind::IncompleteClear => {
            Line::styled("Some entries could not be deleted", state.config.theme.text)
        }
        AlertKind::ClearEmptyBin => Line::styled("Nothing to clear", state.config.theme.text),
    };

    let buttons = Line::from(vec![
        Span::styled("ok", state.config.theme.title),
        Span::styled(" ↵", state.config.theme.hint),
    ]);

    render_popup(
        frame,
        area,
        &state.config.theme,
        "alert",
        ALERT_WIDTH,
        &[msg, Line::from(""), buttons],
    );
}

fn render_popup(
    frame: &mut Frame,
    area: Rect,
    theme: &ThemeConfig,
    title: &str,
    width: u16,
    lines: &[Line],
) {
    let height = lines.len() as u16 + 4; // +2 for border, +2 for paddings
    let popup_area = area.centered(Constraint::Length(width), Constraint::Length(height));
    frame.render_widget(Clear, popup_area);

    let block = Block::bordered()
        .border_style(theme.bin_border)
        .title_top(Line::from(vec![
            Span::styled("─┐", theme.bin_border),
            Span::styled(title, theme.title),
            Span::styled("┌", theme.bin_border),
        ]))
        .style(theme.background);

    let inner = block.inner(popup_area);
    frame.render_widget(&block, popup_area);

    let row_constraints = vec![Constraint::Length(1); lines.len()];
    let rows = inner.layout_vec(&Layout::vertical(row_constraints).flex(Flex::Center));

    for (line, row) in lines.iter().zip(rows) {
        frame.render_widget(Paragraph::new(line.clone()).centered(), row);
    }
}

fn render_logo(frame: &mut Frame, state: &State, area: Rect) {
    let logo_from = state.config.theme.logo_from.fg.unwrap_or(Color::LightBlue);
    let logo_to = state.config.theme.logo_to.fg.unwrap_or(Color::LightBlue);
    let logo_colors = gradient(logo_from, logo_to, LOGO.len());

    let banner_lines = LOGO
        .iter()
        .zip(logo_colors)
        .map(|(row, color)| Line::styled(*row, Style::default().fg(color)))
        .chain(once(
            Line::styled(
                format!("v{}", env!("CARGO_PKG_VERSION")),
                state.config.theme.version,
            )
            .centered(),
        ))
        .collect::<Vec<_>>();

    frame.render_widget(Paragraph::new(banner_lines), area);
}

fn gradient(from: Color, to: Color, steps: usize) -> Vec<Color> {
    let (Color::Rgb(r1, g1, b1), Color::Rgb(r2, g2, b2)) = (from, to) else {
        return vec![to; steps];
    };

    (0..steps)
        .map(|i| {
            let t = i as f32 / (steps.saturating_sub(1).max(1) as f32);
            let lerp = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t) as u8;
            Color::Rgb(lerp(r1, r2), lerp(g1, g2), lerp(b1, b2))
        })
        .collect()
}

fn dim_area(frame: &mut Frame, area: Rect, theme_config: &ThemeConfig) {
    let buffer = frame.buffer_mut();

    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            let cell = &mut buffer[(x, y)];
            cell.set_style(theme_config.dim);
        }
    }
}

fn percentage(part: u64, total: u64) -> f64 {
    if total == 0 {
        0.0
    } else {
        part as f64 / total as f64 * 100.0
    }
}
