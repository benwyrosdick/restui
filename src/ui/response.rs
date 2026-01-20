use crate::app::{App, FocusedPanel};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap},
    Frame,
};

use super::layout::bordered_block;

pub fn draw(frame: &mut Frame, app: &App, area: Rect) {
    let focused = app.focused_panel == FocusedPanel::ResponseView;
    let accent = app.accent_color();
    let block = bordered_block(
        "Response",
        focused,
        accent,
        app.theme_surface_color(),
        app.theme_muted_color(),
    );
    let inner_area = block.inner(area);
    frame.render_widget(block, area);

    if app.is_loading {
        draw_loading(frame, app, inner_area);
        return;
    }

    match &app.response {
        Some(response) => {
            // Split into status line and body
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(2), Constraint::Min(3)])
                .split(inner_area);

            // Status line
            draw_status(frame, app, response, chunks[0], accent);

            // Response body with syntax highlighting
            draw_body(frame, app, response, chunks[1], accent);
        }
        None => {
            let placeholder = Paragraph::new("No response yet. Send a request with 's'.")
                .style(Style::default().fg(app.theme_muted_color()));
            frame.render_widget(placeholder, inner_area);
        }
    }
}

fn draw_loading(frame: &mut Frame, app: &App, area: Rect) {
    let loading = Paragraph::new(format!("Sending request {}", app.spinner_frame())).style(
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    );
    frame.render_widget(loading, area);
}

fn draw_status(
    frame: &mut Frame,
    app: &App,
    response: &crate::http::HttpResponse,
    area: Rect,
    accent: Color,
) {
    let status_color = if response.is_success() {
        Color::Green
    } else if response.status >= 400 {
        Color::Red
    } else {
        Color::Yellow
    };

    let status_line = Line::from(vec![
        Span::styled(
            format!(" {} {} ", response.status, response.status_text),
            Style::default()
                .fg(Color::Black)
                .bg(status_color)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(
            format!("{}ms", response.duration_ms),
            Style::default().fg(accent),
        ),
        Span::raw("  "),
        Span::styled(
            format_size(response.size_bytes),
            Style::default().fg(app.theme_muted_color()),
        ),
    ]);

    let para = Paragraph::new(status_line);
    frame.render_widget(para, area);
}

fn draw_body(
    frame: &mut Frame,
    app: &App,
    response: &crate::http::HttpResponse,
    area: Rect,
    _accent: Color,
) {
    let pretty_body = response.pretty_body();
    let lines: Vec<Line> = pretty_body
        .lines()
        .enumerate()
        .map(|(_, line)| {
            // Basic JSON syntax highlighting
            let styled_line = highlight_json_line(line);
            Line::from(styled_line)
        })
        .collect();

    let total_lines = lines.len() as u16;

    let para = Paragraph::new(lines)
        .scroll((app.response_scroll, 0))
        .wrap(Wrap { trim: false })
        .block(
            Block::default()
                .borders(Borders::TOP)
                .border_style(Style::default().fg(app.theme_muted_color()))
                .style(Style::default().bg(app.theme_surface_color())),
        );

    frame.render_widget(para, area);

    // Render scrollbar if content is larger than area
    if total_lines > area.height {
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("↑"))
            .end_symbol(Some("↓"));

        let mut scrollbar_state =
            ScrollbarState::new(total_lines as usize).position(app.response_scroll as usize);

        frame.render_stateful_widget(scrollbar, area, &mut scrollbar_state);
    }
}

/// Basic JSON syntax highlighting
fn highlight_json_line(line: &str) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let trimmed = line.trim_start();
    let indent = line.len() - trimmed.len();

    // Add indentation
    if indent > 0 {
        spans.push(Span::raw(" ".repeat(indent)));
    }

    // Very basic highlighting - could be improved with a proper lexer
    let mut chars = trimmed.chars().peekable();
    let mut current = String::new();
    let mut in_string = false;
    let mut is_key = false;

    while let Some(c) = chars.next() {
        match c {
            '"' => {
                if in_string {
                    current.push(c);
                    let style = if is_key {
                        Style::default().fg(Color::Cyan)
                    } else {
                        Style::default().fg(Color::Green)
                    };
                    spans.push(Span::styled(current.clone(), style));
                    current.clear();
                    in_string = false;
                    is_key = false;
                } else {
                    if !current.is_empty() {
                        spans.push(Span::styled(current.clone(), Style::default()));
                        current.clear();
                    }
                    current.push(c);
                    in_string = true;
                    // Check if this might be a key (followed by :)
                    is_key = trimmed.contains(':');
                }
            }
            ':' if !in_string => {
                if !current.is_empty() {
                    spans.push(Span::styled(current.clone(), Style::default()));
                    current.clear();
                }
                spans.push(Span::styled(
                    ":".to_string(),
                    Style::default().fg(Color::White),
                ));
            }
            ',' | '{' | '}' | '[' | ']' if !in_string => {
                if !current.is_empty() {
                    spans.push(Span::styled(current.clone(), Style::default()));
                    current.clear();
                }
                spans.push(Span::styled(
                    c.to_string(),
                    Style::default().fg(Color::White),
                ));
            }
            _ if !in_string && (c.is_numeric() || c == '-' || c == '.') => {
                current.push(c);
                // Peek to see if this is a complete number
                if chars.peek().map_or(true, |next| {
                    !next.is_numeric() && *next != '.' && *next != 'e' && *next != 'E'
                }) {
                    spans.push(Span::styled(
                        current.clone(),
                        Style::default().fg(Color::Yellow),
                    ));
                    current.clear();
                }
            }
            _ if !in_string && trimmed.starts_with("true") => {
                spans.push(Span::styled(
                    "true".to_string(),
                    Style::default().fg(Color::Yellow),
                ));
                // Skip remaining chars of "true"
                for _ in 0..3 {
                    chars.next();
                }
            }
            _ if !in_string && trimmed.starts_with("false") => {
                spans.push(Span::styled(
                    "false".to_string(),
                    Style::default().fg(Color::Yellow),
                ));
                // Skip remaining chars of "false"
                for _ in 0..4 {
                    chars.next();
                }
            }
            _ if !in_string && trimmed.starts_with("null") => {
                spans.push(Span::styled(
                    "null".to_string(),
                    Style::default().fg(Color::Magenta),
                ));
                // Skip remaining chars of "null"
                for _ in 0..3 {
                    chars.next();
                }
            }
            _ => {
                current.push(c);
            }
        }
    }

    // Push any remaining content
    if !current.is_empty() {
        let style = if in_string {
            Style::default().fg(Color::Green)
        } else {
            Style::default()
        };
        spans.push(Span::styled(current, style));
    }

    if spans.is_empty() {
        spans.push(Span::raw(line.to_string()));
    }

    spans
}

fn format_size(bytes: usize) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}
