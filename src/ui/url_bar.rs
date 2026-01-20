use crate::app::{App, EditingField, FocusedPanel, InputMode};
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

pub fn draw(frame: &mut Frame, app: &mut App, area: Rect) {
    let focused = app.focused_panel == FocusedPanel::UrlBar;
    let is_editing =
        app.input_mode == InputMode::Editing && app.editing_field == Some(EditingField::Url);

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
        // Block cursor style (like body editor)
        let cursor_pos = app.cursor_position.min(url_text.len());
        if url_text.is_empty() {
            // Empty text, show block cursor as a space
            vec![Span::styled(
                " ",
                Style::default().bg(Color::White).fg(Color::Black),
            )]
        } else if cursor_pos >= url_text.len() {
            // Cursor at end, show block cursor after text
            vec![
                Span::styled(url_text.to_string(), Style::default().bg(Color::DarkGray)),
                Span::styled(" ", Style::default().bg(Color::White).fg(Color::Black)),
            ]
        } else {
            // Cursor in middle, highlight character under cursor
            let (before, rest) = url_text.split_at(cursor_pos);
            let mut chars = rest.chars();
            let cursor_char = chars.next().unwrap_or(' ');
            let after: String = chars.collect();
            vec![
                Span::styled(before.to_string(), Style::default().bg(Color::DarkGray)),
                Span::styled(
                    cursor_char.to_string(),
                    Style::default().bg(Color::White).fg(Color::Black),
                ),
                Span::styled(after, Style::default().bg(Color::DarkGray)),
            ]
        }
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
    let accent = app.accent_color();
    let border_style = if is_editing {
        Style::default().fg(Color::Green)
    } else if focused {
        Style::default().fg(accent)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let title_style = if focused {
        Style::default().fg(accent).add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title(" URL ")
        .title_style(title_style);

    let url_bar = Paragraph::new(url_line).block(block);

    // Calculate where URL text starts for click-to-cursor positioning
    // Format: [border] [space] [METHOD] [space] [URL text...]
    // border = 1, method badge = method.len() + 2, space = 1
    let method_width = app.current_request.method.as_str().len() as u16 + 2; // " GET "
    let url_text_start = area.x + 1 + method_width + 1; // border + method + space
    app.layout_areas.url_text_start = Some(url_text_start);

    frame.render_widget(url_bar, area);
}
