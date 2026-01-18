use crate::app::{App, InputMode};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use super::{request_editor, request_list, response, url_bar};

/// Helper to convert Rect to tuple for storage
fn rect_to_tuple(r: Rect) -> (u16, u16, u16, u16) {
    (r.x, r.y, r.width, r.height)
}

/// Main application layout
pub fn draw_layout(frame: &mut Frame, app: &mut App) {
    let size = frame.area();

    // Main vertical layout: header, main content, footer
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Header
            Constraint::Min(10),   // Main content
            Constraint::Length(1), // Footer
        ])
        .split(size);

    // Draw header
    draw_header(frame, app, chunks[0]);

    // Main horizontal layout: left panel (30%), right panel (70%)
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(chunks[1]);

    // Store layout areas for mouse click detection
    app.layout_areas.request_list = Some(rect_to_tuple(main_chunks[0]));

    // Left panel: Request list / History
    request_list::draw(frame, app, main_chunks[0]);

    // Right panel: URL bar + Request editor + Response viewer
    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),      // URL bar
            Constraint::Percentage(40), // Request editor
            Constraint::Min(5),         // Response viewer (fills remaining space)
        ])
        .split(main_chunks[1]);

    // Store more layout areas
    app.layout_areas.url_bar = Some(rect_to_tuple(right_chunks[0]));
    app.layout_areas.request_editor = Some(rect_to_tuple(right_chunks[1]));
    app.layout_areas.response_view = Some(rect_to_tuple(right_chunks[2]));

    // URL bar
    url_bar::draw(frame, app, right_chunks[0]);

    // Request editor (also stores tab positions)
    request_editor::draw(frame, app, right_chunks[1]);

    // Response viewer
    response::draw(frame, app, right_chunks[2]);

    // Draw footer
    draw_footer(frame, app, chunks[2]);
}

fn draw_header(frame: &mut Frame, app: &App, area: Rect) {
    let env_name = app.environments.active_name();
    let title = format!(" restui                                              [Env: {}] ", env_name);

    let header = Paragraph::new(title)
        .style(Style::default().bg(Color::Blue).fg(Color::White));

    frame.render_widget(header, area);
}

fn draw_footer(frame: &mut Frame, app: &App, area: Rect) {
    let mode_indicator = match app.input_mode {
        InputMode::Normal => Span::styled(
            " NORMAL ",
            Style::default().bg(Color::DarkGray).fg(Color::White),
        ),
        InputMode::Editing => Span::styled(
            " EDITING ",
            Style::default().bg(Color::Green).fg(Color::Black),
        ),
    };

    // Show status or error message
    let message = if let Some(err) = &app.error_message {
        Span::styled(
            format!(" {} ", err),
            Style::default().fg(Color::Red),
        )
    } else if let Some(status) = &app.status_message {
        Span::styled(
            format!(" {} ", status),
            Style::default().fg(Color::Cyan),
        )
    } else {
        Span::raw(" [S]end [N]ew [E]nv [H]istory [?]help | Tab:switch | q:quit ")
    };

    let footer_content = Line::from(vec![mode_indicator, message]);
    let footer = Paragraph::new(footer_content)
        .style(Style::default().bg(Color::DarkGray));

    frame.render_widget(footer, area);
}

/// Helper to create a bordered block with focus indication
pub fn bordered_block(title: &str, focused: bool) -> Block<'_> {
    let border_style = if focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title(format!(" {} ", title))
        .title_style(if focused {
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        })
}
