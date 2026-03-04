use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::state::{AppMode, AppState, LearnState};

/// Draw a centered modal overlay.
pub fn draw_modal(f: &mut Frame, state: &AppState) {
    match &state.mode {
        AppMode::LearnMidi => draw_learn_midi_modal(f, state),
        AppMode::LearnAction => {
            if state.action_menu_open {
                draw_action_menu_modal(f, state);
            } else {
                draw_learn_key_modal(f, state);
            }
        }
        AppMode::RecordMacro => draw_record_macro_modal(f, state),
        AppMode::TextInput => draw_text_input_modal(f, state),
        AppMode::SelectDevice => draw_device_select_modal(f, state),
        AppMode::EditMenu => draw_edit_menu_modal(f, state),
        AppMode::ConfirmDialog(msg) => draw_dialog_modal(f, msg, Color::Yellow),
        AppMode::ErrorDialog(msg) => draw_dialog_modal(f, msg, Color::Red),
        _ => {}
    }
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let [_, vert, _] = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(height),
        Constraint::Fill(1),
    ])
    .areas(area);

    let [_, horiz, _] = Layout::horizontal([
        Constraint::Fill(1),
        Constraint::Length(width),
        Constraint::Fill(1),
    ])
    .areas(vert);

    horiz
}

fn draw_learn_midi_modal(f: &mut Frame, state: &AppState) {
    let area = centered_rect(50, 7, f.area());
    f.render_widget(Clear, area);

    let mut lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "  Waiting for MIDI input…",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("  Press a pad, key, knob, or pedal"),
        Line::from("  on your MIDI device."),
    ];

    if let Some(ref trigger) = state.learned_trigger {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!("  Captured: {trigger}"),
            Style::default().fg(Color::Green),
        )));
    }

    // Knob detection states
    match &state.learn_state {
        Some(LearnState::KnobDetected { channel, controller, .. }) => {
            lines.clear();
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                format!("  Knob detected on CC {controller} (ch={channel})."),
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from("  Configure as rotary knob?"));
            lines.push(Line::from(""));
            lines.push(Line::from("  [Y] Yes  [N] No  [Esc] Cancel"));
        }
        Some(LearnState::KnobLearnCW { .. }) => {
            lines.clear();
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "  Turn the knob CLOCKWISE",
                Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from("  and press Enter…"));
            lines.push(Line::from(""));
            lines.push(Line::from("  [Enter] Confirm  [Esc] Cancel"));
        }
        Some(LearnState::KnobLearnCCW { .. }) => {
            lines.clear();
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "  Turn the knob COUNTER-CLOCKWISE",
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from("  and press Enter…"));
            lines.push(Line::from(""));
            lines.push(Line::from("  [Enter] Confirm  [Esc] Cancel"));
        }
        _ => {}
    }

    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .title(" Learn MIDI ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow)),
        )
        .wrap(Wrap { trim: false });

    f.render_widget(paragraph, area);
}

fn draw_action_menu_modal(f: &mut Frame, state: &AppState) {
    let area = centered_rect(40, 9, f.area());
    f.render_widget(Clear, area);

    let options = ["Key / Chord", "Type Text", "Record Macro"];
    let items: Vec<ListItem> = options
        .iter()
        .enumerate()
        .map(|(i, opt)| {
            let marker = if i == state.action_menu_index {
                "▸ "
            } else {
                "  "
            };
            let style = if i == state.action_menu_index {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(Line::from(Span::styled(
                format!("{marker}{opt}"),
                style,
            )))
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .title(" Choose Action Type ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow)),
    );

    f.render_widget(list, area);
}

fn draw_learn_key_modal(f: &mut Frame, state: &AppState) {
    let area = centered_rect(50, 7, f.area());
    f.render_widget(Clear, area);

    let msg = match &state.learn_state {
        Some(LearnState::WaitingForSingleKey) => "Press a key or chord…",
        Some(LearnState::WaitingForChord { pressed: _ }) => {
            // Can't easily format dynamic content here, use a simpler message
            "Hold modifiers, then press a key…"
        }
        _ => "Waiting…",
    };

    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            format!("  {msg}"),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("  Esc to cancel"),
    ];

    let paragraph = Paragraph::new(lines).block(
        Block::default()
            .title(" Learn Key/Chord ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow)),
    );

    f.render_widget(paragraph, area);
}

fn draw_record_macro_modal(f: &mut Frame, state: &AppState) {
    let area = centered_rect(50, 10, f.area());
    f.render_widget(Clear, area);

    let step_count = if let Some(LearnState::RecordingMacro { steps, .. }) = &state.learn_state {
        steps.len()
    } else {
        0
    };

    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "  ● Recording Macro…",
            Style::default()
                .fg(Color::Red)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(format!("  Steps recorded: {step_count}")),
        Line::from(""),
        Line::from("  Press keys to record them."),
        Line::from("  Enter = stop recording"),
        Line::from("  Esc = cancel"),
    ];

    let paragraph = Paragraph::new(lines).block(
        Block::default()
            .title(" Record Macro ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Red)),
    );

    f.render_widget(paragraph, area);
}

fn draw_text_input_modal(f: &mut Frame, state: &AppState) {
    let area = centered_rect(50, 7, f.area());
    f.render_widget(Clear, area);

    let lines = vec![
        Line::from(""),
        Line::from("  Type text, then press Enter:"),
        Line::from(""),
        Line::from(Span::styled(
            format!("  > {}_", state.text_input_buffer),
            Style::default().fg(Color::Cyan),
        )),
        Line::from(""),
    ];

    let paragraph = Paragraph::new(lines).block(
        Block::default()
            .title(" Text Input ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow)),
    );

    f.render_widget(paragraph, area);
}

fn draw_edit_menu_modal(f: &mut Frame, state: &AppState) {
    let area = centered_rect(44, 12, f.area());
    f.render_widget(Clear, area);

    let mapping = state.selected_mapping_ref();

    let labels: Vec<String> = if let Some(m) = mapping {
        vec![
            "Rename".to_string(),
            "Change Action".to_string(),
            format!("Debounce: {}ms", m.options.debounce_ms),
            format!(
                "Suppress Retrigger: {}",
                if m.options.suppress_retrigger_while_held { "ON" } else { "OFF" }
            ),
            format!(
                "Value Change Only: {}",
                if m.options.trigger_on_value_change_only { "ON" } else { "OFF" }
            ),
            format!(
                "Allow Overlap: {}",
                if m.options.allow_overlap { "ON" } else { "OFF" }
            ),
        ]
    } else {
        vec!["No mapping selected".to_string()]
    };

    let items: Vec<ListItem> = labels
        .iter()
        .enumerate()
        .map(|(i, label)| {
            let marker = if i == state.edit_menu_index {
                "▸ "
            } else {
                "  "
            };
            let style = if i == state.edit_menu_index {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(Line::from(Span::styled(
                format!("{marker}{label}"),
                style,
            )))
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .title(" Edit Mapping ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow)),
    );

    f.render_widget(list, area);
}

fn draw_device_select_modal(f: &mut Frame, state: &AppState) {
    let height = (state.midi_devices.len() as u16 + 5).min(20);
    let area = centered_rect(50, height, f.area());
    f.render_widget(Clear, area);

    if state.midi_devices.is_empty() {
        let lines = vec![
            Line::from(""),
            Line::from(Span::styled(
                "  No MIDI devices found.",
                Style::default().fg(Color::Red),
            )),
            Line::from(""),
            Line::from("  Press 'r' to refresh, Esc to cancel"),
        ];
        let paragraph = Paragraph::new(lines).block(
            Block::default()
                .title(" Select MIDI Device ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow)),
        );
        f.render_widget(paragraph, area);
        return;
    }

    let items: Vec<ListItem> = state
        .midi_devices
        .iter()
        .enumerate()
        .map(|(i, dev)| {
            let marker = if i == state.device_list_index {
                "▸ "
            } else {
                "  "
            };
            let style = if i == state.device_list_index {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(Line::from(Span::styled(
                format!("{marker}{}", dev.name),
                style,
            )))
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .title(" Select MIDI Device ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow)),
    );

    f.render_widget(list, area);
}

fn draw_dialog_modal(f: &mut Frame, message: &str, color: Color) {
    let area = centered_rect(50, 5, f.area());
    f.render_widget(Clear, area);

    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            format!("  {message}"),
            Style::default().fg(color),
        )),
        Line::from(""),
    ];

    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(color)),
        )
        .wrap(Wrap { trim: false });

    f.render_widget(paragraph, area);
}
