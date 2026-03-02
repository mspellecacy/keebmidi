use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::app::state::{AppMode, AppState};

pub fn draw_status_bar(f: &mut Frame, area: Rect, state: &AppState) {
    let mode_str = match &state.mode {
        AppMode::Setup => "SETUP",
        AppMode::Run => "▶ RUN",
        AppMode::LearnMidi => "LEARN MIDI",
        AppMode::LearnAction => "LEARN ACTION",
        AppMode::RecordMacro => "REC MACRO",
        AppMode::TextInput => "TEXT INPUT",
        AppMode::SelectDevice => "SELECT DEVICE",
        AppMode::EditMenu => "EDIT MAPPING",
        AppMode::ConfirmDialog(_) => "CONFIRM",
        AppMode::ErrorDialog(_) => "ERROR",
    };

    let mode_color = match &state.mode {
        AppMode::Run => Color::Green,
        AppMode::LearnMidi | AppMode::LearnAction => Color::Yellow,
        AppMode::RecordMacro => Color::Red,
        AppMode::ErrorDialog(_) => Color::Red,
        _ => Color::Cyan,
    };

    let device_str = state
        .selected_device_name
        .as_deref()
        .unwrap_or("No device");

    let conn_str = if state.midi_connected {
        Span::styled(" ● Connected", Style::default().fg(Color::Green))
    } else {
        Span::styled(" ○ Disconnected", Style::default().fg(Color::Red))
    };

    let panic_str = if state.panic_stop {
        Span::styled(
            " ⚠ STOPPED",
            Style::default()
                .fg(Color::Red)
                .add_modifier(Modifier::BOLD | Modifier::SLOW_BLINK),
        )
    } else {
        Span::raw("")
    };

    let dirty_str = if state.dirty {
        Span::styled(" [modified]", Style::default().fg(Color::Yellow))
    } else {
        Span::raw("")
    };

    let line = Line::from(vec![
        Span::styled(
            " keebmidi ",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("│ "),
        Span::styled(
            mode_str,
            Style::default()
                .fg(mode_color)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" │ "),
        Span::styled(device_str, Style::default().fg(Color::White)),
        conn_str,
        panic_str,
        dirty_str,
    ]);

    let bar = Paragraph::new(line)
        .block(Block::default().borders(Borders::BOTTOM));

    f.render_widget(bar, area);
}
