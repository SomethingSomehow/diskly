use crate::app::state::bin::BinEntry;
use crate::app::state::{Component, State};
use crate::app::tui::SIZE_WIDTH;
use humansize::{DECIMAL, format_size};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Margin, Rect};
use ratatui::prelude::{Line, Span, Style};
use ratatui::widgets::{Block, Row, Scrollbar, ScrollbarOrientation, ScrollbarState, Table};
use std::path::{MAIN_SEPARATOR, Path, PathBuf};

pub fn render_bin(frame: &mut Frame, state: &mut State, area: Rect) {
    let focused = state.focus() == Some(Component::Bin);

    let hint_style = state.config.theme.get_hint(focused);
    let title_style = state.config.theme.get_title(focused);

    let total_files = state.bin.total_files();
    let entries_len = state.bin.rows().len();
    let selected_entry = state.bin.table_state().selected().unwrap_or(0);

    let block = Block::bordered()
        .border_style(state.config.theme.bin_border)
        .title_top(Line::from(vec![
            Span::styled("─┐", state.config.theme.bin_border),
            Span::styled("³", state.config.theme.hint),
            Span::styled("bin", state.config.theme.title),
            Span::styled("┌", state.config.theme.bin_border),
        ]))
        .title_top(
            Line::from(vec![
                Span::styled("┐", state.config.theme.bin_border),
                Span::styled(
                    format!(
                        "{total_files} file{}",
                        if total_files == 1 { "" } else { "s" }
                    ),
                    state.config.theme.title,
                ),
                Span::styled("┌", state.config.theme.bin_border),
                Span::styled("┐", state.config.theme.bin_border),
                Span::styled(
                    format_size(state.bin.total_size(), DECIMAL),
                    state.config.theme.title,
                ),
                Span::styled("┌─", state.config.theme.bin_border),
            ])
            .right_aligned(),
        )
        .title_bottom(
            Line::from(vec![
                Span::styled("─┘", state.config.theme.bin_border),
                Span::styled("↑ ", hint_style),
                Span::styled("select", title_style),
                Span::styled(" ↓", hint_style),
                Span::styled("└┘", state.config.theme.bin_border),
                Span::styled("r", state.config.theme.hint),
                Span::styled("estore", state.config.theme.title),
                Span::styled("└┘", state.config.theme.bin_border),
                Span::styled("c", state.config.theme.hint),
                Span::styled("lear", state.config.theme.title),
                Span::styled("└┘", state.config.theme.bin_border),
                Span::styled("tree", title_style),
                Span::styled(" ⇄", hint_style),
                Span::styled("└", state.config.theme.bin_border),
            ])
            .left_aligned(),
        )
        .title_bottom(
            Line::from(vec![
                Span::styled("┘", state.config.theme.bin_border),
                Span::styled(
                    format!(
                        "{}/{}",
                        if entries_len == 0 {
                            0
                        } else {
                            selected_entry + 1
                        },
                        entries_len
                    ),
                    state.config.theme.title,
                ),
                Span::styled("└─", state.config.theme.bin_border),
            ])
            .right_aligned(),
        );

    frame.render_widget(&block, area);

    let header = Row::new([Line::raw("name"), Line::raw("size")]).style(state.config.theme.title);
    let widths = [Constraint::Fill(1), Constraint::Length(SIZE_WIDTH)];
    let rows = state.bin.rows().iter().map(bin_row).collect::<Vec<_>>();

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

    frame.render_stateful_widget(table, table_area, state.bin.table_state_mut());
    frame.render_stateful_widget(
        Scrollbar::new(ScrollbarOrientation::VerticalLeft)
            .begin_symbol(Some("↑"))
            .end_symbol(Some("↓"))
            .track_symbol(None)
            .thumb_symbol(" ")
            .thumb_style(state.config.theme.scroll_track),
        scrollbar_area,
        &mut ScrollbarState::new(entries_len)
            .position(selected_entry)
            .viewport_content_length(1),
    );

    state.bin.table_area = table_area;
    state.bin.scrollbar_area = scrollbar_area;
}

fn bin_row(entry: &BinEntry) -> Row<'static> {
    let name = shorten_path(&entry.path, entry.is_dir);
    Row::new([Line::raw(name), Line::raw(format_size(entry.size, DECIMAL))])
}

fn shorten_path(path: &Path, is_dir: bool) -> String {
    let components: Vec<_> = path.components().collect();
    let root_len = components
        .iter()
        .take_while(|c| {
            matches!(
                c,
                std::path::Component::Prefix(_) | std::path::Component::RootDir
            )
        })
        .count();

    let mut result = if components.len() - root_len > 4 {
        let head: PathBuf = components[..root_len + 2].iter().collect();
        let tail: PathBuf = components[components.len() - 2..].iter().collect();
        format!(
            "{}{MAIN_SEPARATOR}...{MAIN_SEPARATOR}{}",
            head.display(),
            tail.display()
        )
    } else {
        path.display().to_string()
    };

    if is_dir && !result.ends_with(MAIN_SEPARATOR) {
        result.push(MAIN_SEPARATOR);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use std::path::Path;

    #[cfg(unix)]
    #[rstest]
    #[case("/a/b/c/d", false, "/a/b/c/d")]
    #[case("/a/b/c/d", true, "/a/b/c/d/")]
    #[case("/a/b/c/d/e", false, "/a/b/.../d/e")]
    #[case("/a/b/c/d/e", true, "/a/b/.../d/e/")]
    #[case("/", false, "/")]
    #[case("/", true, "/")]
    fn shorten_path_unix(#[case] input: &str, #[case] is_dir: bool, #[case] expected: &str) {
        assert_eq!(shorten_path(Path::new(input), is_dir), expected);
    }

    #[cfg(windows)]
    #[rstest]
    #[case(r"C:\a\b\c\d", false, r"C:\a\b\c\d")]
    #[case(r"C:\a\b\c\d", true, r"C:\a\b\c\d\")]
    #[case(r"C:\a\b\c\d\e", false, r"C:\a\b\...\d\e")]
    #[case(r"C:\a\b\c\d\e", true, r"C:\a\b\...\d\e\")]
    #[case(r"C:\", false, r"C:\")]
    #[case(r"C:\", true, r"C:\")]
    fn shorten_path_windows(#[case] input: &str, #[case] is_dir: bool, #[case] expected: &str) {
        assert_eq!(shorten_path(Path::new(input), is_dir), expected);
    }
}
