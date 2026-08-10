use crate::app::state::{Component, State};
use crate::app::tui;
use crate::app::tui::{SIZE_WIDTH, percentage};
use crate::config::theme::ThemeConfig;
use crate::fs::tree::FsEntry;
use humansize::{DECIMAL, format_size};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Margin, Rect};
use ratatui::prelude::{Line, Span, Style};
use ratatui::widgets::{Block, Row, Scrollbar, ScrollbarOrientation, ScrollbarState, Table};
use std::iter::once;
use std::sync::atomic::Ordering;
use std::time::SystemTime;
use time::OffsetDateTime;

const BAR_WIDTH: u16 = 10;
const FULL_BAR_WIDTH: u16 = BAR_WIDTH + 5;
const TREE_DATE_WIDTH: u16 = 8;

pub fn render_tree(frame: &mut Frame, state: &mut State, area: Rect) {
    let focused = state.focus() == Some(Component::Tree);
    let scanning = state.fs_scanner.scanning.load(Ordering::Relaxed);

    let hint_style = state.config.theme.get_hint(focused && !scanning);
    let title_style = state.config.theme.get_title(focused && !scanning);

    let rows_len = state.tree.rows().len();
    let entries_len = state.tree.entries_len();
    let selected_entry = state.tree.table_state().selected().unwrap_or(0);

    let block = Block::bordered()
        .border_style(state.config.theme.tree_border)
        .title_top(Line::from(vec![
            Span::styled("─┐", state.config.theme.tree_border),
            Span::styled("¹", state.config.theme.hint),
            Span::styled("tree", state.config.theme.title),
            Span::styled("┌", state.config.theme.tree_border),
        ]))
        .title_top(
            Line::from(vec![
                Span::styled("┐", state.config.theme.tree_border),
                Span::styled(
                    state.tree.path.to_string_lossy().into_owned(),
                    state.config.theme.title,
                ),
                Span::styled("┌", state.config.theme.tree_border),
            ])
            .centered(),
        )
        .title_top(
            Line::from(vec![
                Span::styled("┐", state.config.theme.tree_border),
                Span::styled(
                    format_size(
                        if scanning {
                            state.fs_scanner.scanning_size.load(Ordering::Relaxed)
                        } else {
                            state.tree.total_size()
                        },
                        DECIMAL,
                    ),
                    state.config.theme.title,
                ),
                Span::styled("┌─", state.config.theme.tree_border),
            ])
            .right_aligned(),
        )
        .title_bottom(
            Line::from(vec![
                Span::styled("─┘", state.config.theme.tree_border),
                Span::styled("↑ ", hint_style),
                Span::styled("select", title_style),
                Span::styled(" ↓", hint_style),
                Span::styled("└┘", state.config.theme.tree_border),
                Span::styled("open", title_style),
                Span::styled(" ↵", hint_style),
                Span::styled("└┘", state.config.theme.tree_border),
                Span::styled("t", hint_style),
                Span::styled("rash", title_style),
                Span::styled("└┘", state.config.theme.tree_border),
                Span::styled("bin", title_style),
                Span::styled(" ⇄", hint_style),
                Span::styled("└", state.config.theme.tree_border),
            ])
            .left_aligned(),
        )
        .title_bottom(
            Line::from(vec![
                Span::styled("┘", state.config.theme.tree_border),
                Span::styled(
                    if scanning {
                        format!(
                            "0/{}",
                            state.fs_scanner.scanning_entries.load(Ordering::Relaxed)
                        )
                    } else {
                        format!("{selected_entry}/{entries_len}")
                    },
                    state.config.theme.title,
                ),
                Span::styled("└─", state.config.theme.tree_border),
            ])
            .right_aligned(),
        );

    frame.render_widget(block, area);

    let header = Row::new([
        Line::raw("name"),
        Line::raw("size"),
        Line::raw("usage"),
        Line::raw("modified"),
        Line::raw("created"),
    ])
    .style(state.config.theme.title);

    let widths = [
        Constraint::Fill(1),
        Constraint::Length(SIZE_WIDTH),
        Constraint::Length(FULL_BAR_WIDTH),
        Constraint::Length(TREE_DATE_WIDTH),
        Constraint::Length(TREE_DATE_WIDTH),
    ];

    let parent_row = Row::new([
        Line::raw(".."),
        Line::raw(""),
        Line::raw(""),
        Line::raw(""),
        Line::raw(""),
    ]);

    let rows = once(parent_row)
        .chain(state.tree.entries().map(|e| tree_row(e, state)))
        .collect::<Vec<_>>();

    let table = Table::new(rows, widths)
        .header(header)
        .style(state.config.theme.text)
        .row_highlight_style(if focused {
            state.config.theme.row_highlight
        } else {
            Style::default()
        });

    let [table_area, scrollbar_area] =
        Layout::horizontal([Constraint::Fill(1), Constraint::Length(1)]).areas(area.inner(
            Margin {
                horizontal: 1,
                vertical: 1,
            },
        ));

    frame.render_stateful_widget(table, table_area, state.tree.table_state_mut());
    render_tree_bars(frame, table_area, &state.config.theme);
    frame.render_stateful_widget(
        Scrollbar::new(ScrollbarOrientation::VerticalLeft)
            .begin_symbol(Some("↑"))
            .end_symbol(Some("↓"))
            .track_symbol(None)
            .thumb_symbol(" ")
            .thumb_style(state.config.theme.scroll_track),
        scrollbar_area,
        &mut ScrollbarState::new(rows_len)
            .position(selected_entry)
            .viewport_content_length(1),
    );

    state.tree.table_area = table_area;
    state.tree.scrollbar_area = scrollbar_area;

    if scanning {
        tui::dim_area(frame, table_area, &state.config.theme)
    }
}

fn tree_row(entry: &FsEntry, state: &State) -> Row<'static> {
    let name = entry.name().to_string_lossy().to_string();
    let name = if entry.is_dir() {
        format!("{name}/")
    } else {
        name
    };

    let size = entry
        .size()
        .map(|s| format_size(s, DECIMAL))
        .unwrap_or_else(|| "─".to_string());
    let total_size = state.tree.total_size();

    let entry_usage = entry.size().map_or(0.0, |s| percentage(s, total_size));
    let bar = Line::raw(format!("{entry_usage:>3.0}%"));

    let modified = entry
        .modified()
        .map(format_tree_date)
        .unwrap_or_else(|| "─".to_string());

    let created = entry
        .created()
        .map(format_tree_date)
        .unwrap_or_else(|| "─".to_string());

    let mut row = Row::new([
        Line::raw(name),
        Line::raw(size),
        bar,
        Line::raw(modified),
        Line::raw(created),
    ]);

    let entry_path = state.tree.path.join(entry.name());
    if state.bin.contains_path(&entry_path) {
        row = row.style(state.config.theme.inactive_text)
    }

    row
}

fn format_tree_date(t: SystemTime) -> String {
    let dt = OffsetDateTime::from(t);
    format!(
        "{:02}-{:02}-{:02}",
        dt.year() % 100,
        dt.month() as u8,
        dt.day()
    )
}

// ♿
fn render_tree_bars(frame: &mut Frame, area: Rect, theme_config: &ThemeConfig) {
    let buffer = frame.buffer_mut();

    let content_top = area.y + 1; // skip header row

    for y in content_top..area.bottom() {
        let Some(px) = (area.left()..area.right()).find(|&x| buffer[(x, y)].symbol() == "%") else {
            continue;
        };

        let digits: String = (area.left()..px)
            .rev()
            .map_while(|x| buffer[(x, y)].symbol().trim().parse::<char>().ok())
            .collect::<String>()
            .chars()
            .rev()
            .collect();

        let Ok(usage) = digits.trim().parse::<f64>() else {
            continue;
        };
        let filled = ((usage / 100.0) * BAR_WIDTH as f64).round() as u16;

        for dx in 0..BAR_WIDTH {
            let x = px + 2 + dx;
            if x >= area.right() {
                break;
            }
            buffer[(x, y)].set_style(if dx < filled {
                theme_config.filled_bar
            } else {
                theme_config.empty_bar
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use time::macros::datetime;

    #[rstest]
    #[case(datetime!(2026-06-01 12:00:00 UTC), "26-06-01")]
    #[case(datetime!(2000-01-01 00:00:00 UTC), "00-01-01")]
    #[case(datetime!(1999-12-31 23:59:59 UTC), "99-12-31")]
    fn format_tree_date_correctly(#[case] input: OffsetDateTime, #[case] expected: &str) {
        assert_eq!(format_tree_date(input.into()), expected);
    }
}
