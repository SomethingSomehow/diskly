use crate::app::state::State;
use crate::app::tui::percentage;
use crate::fs::tree::FsEntry;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::prelude::{Color, Line, Span};
use ratatui::widgets::Block;
use tui_piechart::{PieChart, PieSlice};

const PIE_SLICES: usize = 7;
const PIE_PALETTE: [Color; PIE_SLICES] = [
    Color::Red,
    Color::Yellow,
    Color::Green,
    Color::Cyan,
    Color::Blue,
    Color::Magenta,
    Color::White,
];
const PIE_OTHER_COLOR: Color = Color::DarkGray;

pub fn render_pie_chart(frame: &mut Frame, state: &State, area: Rect) {
    let block = Block::bordered()
        .border_style(state.config.theme.pie_border)
        .title_top(Line::from(vec![
            Span::styled("─┐", state.config.theme.pie_border),
            Span::styled("²", state.config.theme.hint),
            Span::styled("pie", state.config.theme.title),
            Span::styled("┌", state.config.theme.pie_border),
        ]));

    let pie = PieChart::new(pie_slices(state.tree.entries(), state.tree.total_size()))
        .block(block)
        .high_resolution(true)
        .show_legend(true)
        .show_percentages(false)
        .style(state.config.theme.text);

    frame.render_widget(pie, area);
}

fn pie_slices<'a>(
    entries: impl Iterator<Item = &'a FsEntry>,
    total_size: u64,
) -> Vec<PieSlice<'a>> {
    let mut slices = Vec::with_capacity(PIE_SLICES + 1);
    let mut other_size = 0u64;

    for (i, entry) in entries.enumerate() {
        if i < PIE_SLICES {
            slices.push(pie_slice(entry, total_size, PIE_PALETTE[i]));
        } else {
            other_size += entry.size().unwrap_or(0);
        }
    }

    if other_size > 0 {
        slices.push(PieSlice::new(
            "Other",
            percentage(other_size, total_size),
            PIE_OTHER_COLOR,
        ));
    }

    slices
}

fn pie_slice(entry: &FsEntry, total_size: u64, color: Color) -> PieSlice<'_> {
    let name = entry.name().to_str().unwrap_or("<invalid utf8>");
    let size = entry.size().unwrap_or(0);
    PieSlice::new(name, percentage(size, total_size), color)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fs_entry(size: Option<u64>) -> FsEntry {
        FsEntry::file("file".into(), size, None, None)
    }

    #[test]
    fn pie_slices_overflow() {
        // Given
        const OVERFLOW: usize = 3;
        const ENTRY_SIZE: u64 = 10;

        let entries: Vec<FsEntry> = (0..PIE_SLICES + OVERFLOW)
            .map(|_| fs_entry(Some(ENTRY_SIZE)))
            .collect();
        let total_size = entries.len() as u64 * ENTRY_SIZE;

        // When
        let slices = pie_slices(entries.iter(), total_size);

        // Then
        assert_eq!(slices.len(), PIE_SLICES + 1);

        let other = slices.last().unwrap();
        let expected_other_size = OVERFLOW as u64 * ENTRY_SIZE;
        assert_eq!(other.value(), percentage(expected_other_size, total_size));
    }
}
