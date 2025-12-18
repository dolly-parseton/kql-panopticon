use crate::app::{ConnectionStatus, CurrentScreen};
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Padding, Paragraph};
use ratatui::Frame;

pub fn render_title(frame: &mut Frame, app: &crate::app::App, chunk: Rect) {
    // Create inner area with horizontal margin
    let inner_area = Layout::default()
        .direction(Direction::Horizontal)
        .horizontal_margin(1)
        .constraints([Constraint::Min(0)])
        .split(chunk)[0];

    // Calculate status indicator width
    let status_text = get_connection_status_text(&app.connection_status, &app.theme, app.animation_frame());
    let status_width = status_text.chars().count() as u16 + 4; // Add padding

    // Split into title (left) and status (right)
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Min(0),            // Left: title takes remaining space
            Constraint::Length(status_width), // Right: status indicator
        ])
        .split(inner_area);

    let title_area = chunks[0];
    let status_area = chunks[1];

    // Render title on the left
    render_title_text(frame, app, title_area);

    // Render status indicator on the right
    render_connection_status(frame, app, status_area);
}

fn render_title_text(frame: &mut Frame, app: &crate::app::App, area: Rect) {
    let view_name = match app.current_screen {
        CurrentScreen::Interpreter => "Interpreter",
        CurrentScreen::Settings => "Settings",
        _ => "",
    };

    // Get styled title spans
    let title_spans = app.theme.format_title_styled(view_name);
    let title_line = Line::from(title_spans);

    frame.render_widget(
        Paragraph::new(title_line)
            .block(
                Block::default()
                    .padding(Padding::horizontal(1))
                    .borders(Borders::TOP | Borders::LEFT | Borders::BOTTOM)
                    .border_type(app.theme.border_type())
                    .border_style(app.theme.border_style())
                    .style(app.theme.background_style()),
            ),
        area,
    );
}

fn render_connection_status(frame: &mut Frame, app: &crate::app::App, area: Rect) {
    let frame_idx = app.animation_frame();
    let (icon, text, style) = get_status_display(&app.connection_status, &app.theme, frame_idx);

    let status_line = Line::from(vec![
        Span::styled(icon, style),
        Span::raw(" "),
        Span::styled(text, style),
    ]);

    frame.render_widget(
        Paragraph::new(status_line)
            .alignment(Alignment::Right)
            .block(
                Block::default()
                    .padding(Padding::horizontal(1))
                    .borders(Borders::TOP | Borders::RIGHT | Borders::BOTTOM)
                    .border_type(app.theme.border_type())
                    .border_style(app.theme.border_style())
                    .style(app.theme.background_style()),
            ),
        area,
    );
}

/// Get the text representation of connection status (for width calculation)
fn get_connection_status_text(
    status: &ConnectionStatus,
    theme: &crate::theme::Theme,
    frame_idx: usize,
) -> String {
    match status {
        ConnectionStatus::Authenticating => {
            let spinner = theme.get_spinner_frame(frame_idx);
            format!("{} Authenticating...", spinner)
        }
        ConnectionStatus::Discovering => {
            let spinner = theme.get_spinner_frame(frame_idx);
            format!("{} Discovering...", spinner)
        }
        ConnectionStatus::Ready { workspace_count } => {
            format!("{} {} workspaces", theme.completed_icon(), workspace_count)
        }
        ConnectionStatus::Error { message } => {
            // Truncate long error messages
            let truncated = if message.len() > 25 {
                format!("{}...", &message[..22])
            } else {
                message.clone()
            };
            format!("{} {}", theme.failed_icon(), truncated)
        }
    }
}

/// Get the display components for connection status
fn get_status_display(
    status: &ConnectionStatus,
    theme: &crate::theme::Theme,
    frame_idx: usize,
) -> (String, String, Style) {
    match status {
        ConnectionStatus::Authenticating => {
            let spinner = theme.get_spinner_frame(frame_idx);
            let style = theme.running_status_style(frame_idx);
            (spinner.to_string(), "Authenticating...".to_string(), style)
        }
        ConnectionStatus::Discovering => {
            let spinner = theme.get_spinner_frame(frame_idx);
            let style = theme.running_status_style(frame_idx);
            (spinner.to_string(), "Discovering...".to_string(), style)
        }
        ConnectionStatus::Ready { workspace_count } => {
            let style = theme.success_status_style();
            (
                theme.completed_icon().to_string(),
                format!("{} workspaces", workspace_count),
                style,
            )
        }
        ConnectionStatus::Error { message } => {
            let style = theme.error_status_style();
            // Truncate long error messages
            let truncated = if message.len() > 25 {
                format!("{}...", &message[..22])
            } else {
                message.clone()
            };
            (theme.failed_icon().to_string(), truncated, style)
        }
    }
}
