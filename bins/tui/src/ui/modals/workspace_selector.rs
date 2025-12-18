//! Workspace selector modal rendering

use crate::app::WorkspaceSelectorState;
use crate::theme::Theme;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;
use tui_tree_widget::{Tree, TreeState};

/// Render the workspace selector modal as a centered popup
pub fn render_workspace_selector(frame: &mut Frame, state: &WorkspaceSelectorState, theme: &Theme) {
    let area = centered_rect(70, 80, frame.area());

    // Clear the area behind the modal
    frame.render_widget(Clear, area);

    // Build the tree items
    let items = state.build_tree_items();

    // Create tree state with expanded items
    let mut tree_state = TreeState::default();
    for id in state.expanded_identifiers() {
        tree_state.open(vec![id]);
    }
    // Select current item (full path from root)
    tree_state.select(state.current_path());

    // Header with instructions
    let header = Line::from(vec![
        Span::styled("[a]", theme.text_dim_style()),
        Span::raw(" All  "),
        Span::styled("[n]", theme.text_dim_style()),
        Span::raw(" None  "),
        Span::styled("[Space]", theme.text_dim_style()),
        Span::raw(" Toggle  "),
        Span::styled(
            "[Enter]",
            Style::default().fg(theme.colors.success.clone().into()),
        ),
        Span::raw(" Confirm  "),
        Span::styled(
            "[Esc]",
            Style::default().fg(theme.colors.error.clone().into()),
        ),
        Span::raw(" Cancel"),
    ]);

    // Footer with selection count
    let footer = Line::from(vec![
        Span::raw("Selected: "),
        Span::styled(
            format!("{}/{}", state.selected_count(), state.workspace_count()),
            Style::default()
                .fg(theme.colors.accent.clone().into())
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" workspaces"),
    ]);

    // Split area into header, tree, footer
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Min(5),    // Tree
            Constraint::Length(3), // Footer
        ])
        .split(area);

    // Render header
    let header_block = Block::default()
        .borders(Borders::TOP | Borders::LEFT | Borders::RIGHT)
        .border_type(theme.border_type())
        .border_style(theme.border_style())
        .style(theme.modal_overlay_style())
        .title(" Select Workspaces ");
    let header_para = Paragraph::new(header)
        .block(header_block)
        .style(theme.text_style())
        .alignment(ratatui::layout::Alignment::Center);
    frame.render_widget(header_para, chunks[0]);

    // Render tree
    let tree_block = Block::default()
        .borders(Borders::LEFT | Borders::RIGHT)
        .border_type(theme.border_type())
        .border_style(theme.border_style())
        .style(theme.modal_overlay_style());
    let tree = Tree::new(&items)
        .expect("valid tree")
        .block(tree_block)
        .style(theme.text_style())
        .highlight_style(theme.highlight_style());
    frame.render_stateful_widget(tree, chunks[1], &mut tree_state);

    // Render footer
    let footer_block = Block::default()
        .borders(Borders::BOTTOM | Borders::LEFT | Borders::RIGHT)
        .border_type(theme.border_type())
        .border_style(theme.border_style())
        .style(theme.modal_overlay_style());
    let footer_para = Paragraph::new(footer)
        .block(footer_block)
        .style(theme.text_style())
        .alignment(ratatui::layout::Alignment::Center);
    frame.render_widget(footer_para, chunks[2]);
}

/// Create a centered rectangle of given percentage width and height
fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
