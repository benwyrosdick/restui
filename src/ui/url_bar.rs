use crate::app::{App, EditingField, FocusedPanel, InputMode};
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

pub fn draw(frame: &mut Frame, app: &App, area: Rect) {
    let focused = app.focused_panel == FocusedPanel::UrlBar;
    let is_editing = app.input_mode == InputMode::Editing
        && app.editing_field == Some(EditingField::Url);

    // Method color
    let method_color = match app.current_request.method {
        crate::storage::HttpMethod::Get => Color::Green,
        crate::storage::HttpMethod::Post => Color::Yellow,
        crate::storage::HttpMethod::Put => Color::Blue,
        crate::storage::HttpMethod::Delete => Color::Red,
    };

    // URL display with cursor if editing
    let url_text = &app.current_request.url;
    let url_spans: Vec<Span> = if is_editing {
        // Insert cursor at the cursor position
        let cursor_pos = app.cursor_position.min(url_text.len());
        let (before, after) = url_text.split_at(cursor_pos);
        vec![
            Span::styled(before.to_string(), Style::default().bg(Color::DarkGray)),
            Span::styled("│", Style::default().fg(Color::White).bg(Color::DarkGray)),
            Span::styled(after.to_string(), Style::default().bg(Color::DarkGray)),
        ]
    } else if url_text.is_empty() {
        vec![Span::styled(
            "Enter URL... (press Enter or 'i' to edit)",
            Style::default().fg(Color::DarkGray),
        )]
    } else {
        vec![Span::styled(url_text.clone(), Style::default())]
    };

    // Build the URL line
    let mut spans = vec![
        Span::styled(
            format!(" {} ", app.current_request.method.as_str()),
            Style::default()
                .fg(Color::Black)
                .bg(method_color)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
    ];
    spans.extend(url_spans);
    let url_line = Line::from(spans);

    // Border style based on focus
    let border_style = if is_editing {
        Style::default().fg(Color::Green)
    } else if focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let title_style = if focused {
        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title(" URL ")
        .title_style(title_style);

    let url_bar = Paragraph::new(url_line).block(block);

    frame.render_widget(url_bar, area);
}
