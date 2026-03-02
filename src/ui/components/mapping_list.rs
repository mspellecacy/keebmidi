use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState};
use ratatui::Frame;

use crate::app::state::{ActivePane, AppState};

pub fn draw_mapping_list(f: &mut Frame, area: Rect, state: &AppState) {
    let is_focused = state.active_pane == ActivePane::MappingList;

    let items: Vec<ListItem> = state
        .mappings
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let enabled_marker = if m.enabled { "✓" } else { "○" };
            let selected_marker = if state.selected_mapping == Some(i) {
                "▸"
            } else {
                " "
            };

            let line = Line::from(vec![
                Span::raw(format!("{selected_marker} ")),
                Span::styled(
                    enabled_marker,
                    Style::default().fg(if m.enabled {
                        Color::Green
                    } else {
                        Color::DarkGray
                    }),
                ),
                Span::raw(" "),
                Span::styled(
                    &m.name,
                    Style::default().fg(if m.enabled {
                        Color::White
                    } else {
                        Color::DarkGray
                    }),
                ),
            ]);

            ListItem::new(line)
        })
        .collect();

    let border_style = if is_focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let title = format!(" Mappings ({}) ", state.mappings.len());
    let list = List::new(items).block(
        Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(border_style),
    );

    let mut list_state = ListState::default();
    list_state.select(state.selected_mapping);
    f.render_stateful_widget(list, area, &mut list_state);
}
