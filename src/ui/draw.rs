use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::state::{ActivePane, AppMode, AppState};
use crate::ui::components::details::draw_details;
use crate::ui::components::mapping_list::draw_mapping_list;
use crate::ui::components::modal::draw_modal;
use crate::ui::components::status_bar::draw_status_bar;

/// Main draw function. Renders the full UI from AppState.
pub fn draw(f: &mut Frame, state: &AppState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),  // top status bar
            Constraint::Min(10),   // main content
            Constraint::Length(8), // log pane
            Constraint::Length(1), // footer keybinds
        ])
        .split(f.area());

    // Top bar
    draw_status_bar(f, chunks[0], state);

    // Main content: left (mappings) + right (details)
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(chunks[1]);

    draw_mapping_list(f, main_chunks[0], state);
    draw_details(f, main_chunks[1], state);

    // Log pane
    draw_log_pane(f, chunks[2], state);

    // Footer keybinds
    draw_footer(f, chunks[3], state);

    // Modal overlay (if any)
    match &state.mode {
        AppMode::Setup | AppMode::Run => {}
        _ => draw_modal(f, state),
    }
}

fn draw_log_pane(f: &mut Frame, area: Rect, state: &AppState) {

    let is_focused = state.active_pane == ActivePane::Log;
    let border_style = if is_focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let visible_lines = (area.height as usize).saturating_sub(2);
    let start = state.log_lines.len().saturating_sub(visible_lines);
    let lines: Vec<Line> = state.log_lines[start..]
        .iter()
        .map(|l| {
            let color = if l.starts_with("▶") {
                Color::Green
            } else if l.starts_with("⚠") || l.starts_with("MIDI:") {
                Color::Yellow
            } else if l.contains("error") || l.contains("Error") || l.contains("failed") {
                Color::Red
            } else {
                Color::DarkGray
            };
            Line::from(Span::styled(l.as_str(), Style::default().fg(color)))
        })
        .collect();

    let mut title_parts = vec![Span::raw(" Log ")];
    if let Some(ref midi) = state.last_midi_event {
        title_parts.push(Span::styled(
            format!("│ MIDI: {midi} "),
            Style::default().fg(Color::Yellow),
        ));
    }
    if let Some(ref action) = state.last_action {
        title_parts.push(Span::styled(
            format!("│ Last: {action} "),
            Style::default().fg(Color::Green),
        ));
    }

    let log = Paragraph::new(lines)
        .block(
            Block::default()
                .title(Line::from(title_parts))
                .borders(Borders::ALL)
                .border_style(border_style),
        )
        .wrap(Wrap { trim: false });

    f.render_widget(log, area);
}

fn draw_footer(f: &mut Frame, area: Rect, state: &AppState) {

    let binds = match &state.mode {
        AppMode::Setup => {
            "q:Quit  s:Save  a:Add  e:Edit  d:Delete  l:LearnMIDI  r:Run  Space:Toggle  D:Device  Tab:Pane  F12:Stop"
        }
        AppMode::Run => "r/Esc:Stop  q:Quit  F12:PanicStop",
        AppMode::LearnMidi => "Esc:Cancel",
        AppMode::LearnAction => "↑↓:Select  Enter:Confirm  Esc:Cancel",
        AppMode::RecordMacro => "Enter:Stop  Esc:Cancel",
        AppMode::TextInput => "Enter:Confirm  Esc:Cancel  Backspace:Delete",
        AppMode::SelectDevice => "↑↓:Select  Enter:Connect  r:Refresh  Esc:Cancel",
        AppMode::EditMenu => "↑↓:Select  Enter:Confirm/Toggle  Esc:Cancel",
        AppMode::ConfirmDialog(_) => "y:Yes  n:No  Esc:Cancel",
        AppMode::ErrorDialog(_) => "Enter/Esc:Dismiss",
    };

    let footer = Paragraph::new(Line::from(Span::styled(
        format!(" {binds}"),
        Style::default().fg(Color::DarkGray),
    )));

    f.render_widget(footer, area);
}
