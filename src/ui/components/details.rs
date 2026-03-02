use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::state::{ActivePane, AppState};
use crate::config::model::OutputAction;

pub fn draw_details(f: &mut Frame, area: Rect, state: &AppState) {
    let is_focused = state.active_pane == ActivePane::Details;

    let border_style = if is_focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let content = if let Some(mapping) = state.selected_mapping_ref() {
        let mut lines = vec![
            Line::from(vec![
                Span::styled("Name: ", Style::default().fg(Color::Yellow)),
                Span::raw(&mapping.name),
            ]),
            Line::from(vec![
                Span::styled("ID: ", Style::default().fg(Color::DarkGray)),
                Span::styled(&mapping.id, Style::default().fg(Color::DarkGray)),
            ]),
            Line::from(vec![
                Span::styled("Enabled: ", Style::default().fg(Color::Yellow)),
                Span::styled(
                    if mapping.enabled { "Yes" } else { "No" },
                    Style::default().fg(if mapping.enabled {
                        Color::Green
                    } else {
                        Color::Red
                    }),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Trigger: ", Style::default().fg(Color::Yellow)),
                Span::raw(format!("{}", mapping.trigger)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Action: ", Style::default().fg(Color::Yellow)),
                Span::raw(format!("{}", mapping.action)),
            ]),
        ];

        // Show macro steps if applicable
        if let OutputAction::Macro { ref spec } = mapping.action {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Macro Steps:",
                Style::default().fg(Color::Yellow),
            )));
            for (i, step) in spec.steps.iter().enumerate() {
                lines.push(Line::from(format!("  {}: {step}", i + 1)));
            }
        }

        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("Options: ", Style::default().fg(Color::Yellow)),
        ]));
        lines.push(Line::from(format!(
            "  Debounce: {}ms",
            mapping.options.debounce_ms
        )));
        lines.push(Line::from(format!(
            "  Suppress retrigger: {}",
            mapping.options.suppress_retrigger_while_held
        )));
        lines.push(Line::from(format!(
            "  Value change only: {}",
            mapping.options.trigger_on_value_change_only
        )));
        lines.push(Line::from(format!(
            "  Allow overlap: {}",
            mapping.options.allow_overlap
        )));

        lines
    } else {
        vec![
            Line::from(""),
            Line::from(Span::styled(
                "  No mapping selected",
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "  Press 'a' to add a new mapping",
                Style::default().fg(Color::DarkGray),
            )),
        ]
    };

    let details = Paragraph::new(content)
        .block(
            Block::default()
                .title(" Details ")
                .borders(Borders::ALL)
                .border_style(border_style),
        )
        .wrap(Wrap { trim: false });

    f.render_widget(details, area);
}
