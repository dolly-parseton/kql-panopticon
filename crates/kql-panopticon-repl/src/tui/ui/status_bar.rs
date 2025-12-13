//! Status bar rendering
//!
//! Shows application title and context information.

use crate::context::SharedContext;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

/// Render the status bar
pub fn render(frame: &mut Frame, area: Rect, ctx: &SharedContext) {
    // Try to get context info, but don't block
    let (workspace_info, step_count, input_count) = if let Ok(ctx) = ctx.try_read() {
        let ws = ctx.selected_workspaces()
            .first()
            .map(|w| w.name.clone())
            .unwrap_or_else(|| "(none)".to_string());
        let steps = ctx.pack_session().steps.len();
        let inputs = ctx.pack_session().inputs.len();
        (ws, steps, inputs)
    } else {
        ("...".to_string(), 0, 0)
    };

    // Build status line
    let left = Span::styled(
        " KQL Panopticon ",
        Style::default().fg(Color::Black).bg(Color::Cyan),
    );

    let right_text = format!(
        " {} steps │ {} inputs │ ws: {} ",
        step_count, input_count, workspace_info
    );
    let right = Span::styled(
        right_text.clone(),
        Style::default().fg(Color::DarkGray),
    );

    // Calculate spacing
    let left_width = 16; // " KQL Panopticon "
    let right_width = right_text.len();
    let spacer_width = area.width as usize - left_width - right_width;
    let spacer = " ".repeat(spacer_width.max(1));

    let line = Line::from(vec![
        left,
        Span::raw(spacer),
        right,
    ]);

    let paragraph = Paragraph::new(line)
        .style(Style::default().bg(Color::DarkGray));

    frame.render_widget(paragraph, area);
}
