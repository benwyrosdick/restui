use crate::app::{App, FocusedPanel, ResponseMode};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap},
    Frame,
};

use super::layout::bordered_block;
use super::widgets::text_with_cursor_and_selection;

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
            // Show status bar when: in input mode, have active filter, or have search matches
            let show_status_bar = app.response_mode != ResponseMode::Normal
                || app.response_filtered_content.is_some()
                || !app.response_search_matches.is_empty();

            let constraints = if show_status_bar {
                vec![
                    Constraint::Length(2),
                    Constraint::Min(3),
                    Constraint::Length(1),
                ]
            } else {
                vec![Constraint::Length(2), Constraint::Min(3)]
            };

            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints(constraints)
                .split(inner_area);

            // Status line
            draw_status(frame, app, response, chunks[0], accent);

            // Response body with syntax highlighting
            draw_body(frame, app, chunks[1], accent);

            // Search/filter status bar
            if show_status_bar {
                draw_search_bar(frame, app, chunks[2], accent);
            }
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

fn draw_body(frame: &mut Frame, app: &App, area: Rect, accent: Color) {
    // Get content source - use filtered if available, otherwise cached lines
    let (content_lines, total_lines): (Vec<&str>, usize) =
        if let Some(filtered) = &app.response_filtered_content {
            let lines: Vec<&str> = filtered.lines().collect();
            let count = lines.len();
            (lines, count)
        } else {
            let lines: Vec<&str> = app.response_lines.iter().map(|s| s.as_str()).collect();
            let count = lines.len();
            (lines, count)
        };

    // Calculate visible viewport for efficient rendering
    // Only process lines that will actually be displayed
    let visible_height = area.height.saturating_sub(1) as usize; // -1 for border
    let scroll_pos = app.response_scroll as usize;
    let start_line = scroll_pos.min(total_lines);
    let end_line = (scroll_pos + visible_height + 1).min(total_lines); // +1 for partial lines

    let search_query = app.response_search_query.to_lowercase();

    // Only process visible lines - this is the key optimization
    let lines: Vec<Line> = content_lines
        .iter()
        .enumerate()
        .skip(start_line)
        .take(end_line.saturating_sub(start_line))
        .map(|(line_num, line)| {
            let is_match = app.response_search_matches.contains(&line_num);
            let is_current_match = is_match
                && app.response_search_matches.get(app.response_current_match) == Some(&line_num);

            // Basic JSON syntax highlighting - only for visible lines
            let styled_line = if is_match && !search_query.is_empty() {
                highlight_json_line_with_search(line, &search_query, accent)
            } else {
                highlight_json_line(line)
            };

            let line = Line::from(styled_line);

            // Add background for current match
            if is_current_match {
                line.style(Style::default().bg(app.theme_selection_bg()))
            } else if is_match && !search_query.is_empty() {
                line.style(Style::default().bg(Color::DarkGray))
            } else {
                line
            }
        })
        .collect();

    // Use a Paragraph that doesn't need to scroll since we've already sliced the content
    let para = Paragraph::new(lines).wrap(Wrap { trim: false }).block(
        Block::default()
            .borders(Borders::TOP)
            .border_style(Style::default().fg(app.theme_muted_color()))
            .style(Style::default().bg(app.theme_surface_color())),
    );

    frame.render_widget(para, area);

    // Render scrollbar if content is larger than area
    let total_lines_u16 = total_lines as u16;
    if total_lines_u16 > area.height {
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("↑"))
            .end_symbol(Some("↓"));

        let mut scrollbar_state =
            ScrollbarState::new(total_lines).position(app.response_scroll as usize);

        frame.render_stateful_widget(scrollbar, area, &mut scrollbar_state);
    }
}

fn draw_search_bar(frame: &mut Frame, app: &App, area: Rect, accent: Color) {
    let is_input_mode = app.response_mode != ResponseMode::Normal;

    let mut spans = Vec::new();

    if is_input_mode {
        // Active input mode - show editable query with cursor
        let (prefix, query, cursor_pos) = match app.response_mode {
            ResponseMode::Search => (
                "/",
                &app.response_search_query,
                app.response_cursor_position,
            ),
            ResponseMode::Filter => (
                "jq: ",
                &app.response_filter_query,
                app.response_cursor_position,
            ),
            ResponseMode::Normal => unreachable!(),
        };

        spans.push(Span::styled(
            prefix,
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        ));

        spans.extend(text_with_cursor_and_selection(
            query,
            cursor_pos,
            true,
            "",
            Style::default().fg(Color::White),
            None, // Response search/filter doesn't support selection
        ));
    } else {
        // Normal mode - show applied filter/search info
        if app.response_filtered_content.is_some() {
            spans.push(Span::styled(
                "jq: ",
                Style::default().fg(accent).add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::styled(
                app.response_filter_query.clone(),
                Style::default().fg(Color::White),
            ));
        } else if !app.response_search_matches.is_empty() {
            spans.push(Span::styled(
                "/",
                Style::default().fg(accent).add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::styled(
                app.response_search_query.clone(),
                Style::default().fg(Color::White),
            ));
        }
    }

    // Add match count for search
    if !app.response_search_matches.is_empty() {
        spans.push(Span::styled(
            format!(
                " [{}/{}]",
                app.response_current_match + 1,
                app.response_search_matches.len()
            ),
            Style::default().fg(app.theme_muted_color()),
        ));
    } else if app.response_mode == ResponseMode::Search && !app.response_search_query.is_empty() {
        spans.push(Span::styled(
            " [0/0]",
            Style::default().fg(app.theme_muted_color()),
        ));
    }

    // Add hint for clearing
    if !is_input_mode
        && (app.response_filtered_content.is_some() || !app.response_search_matches.is_empty())
    {
        spans.push(Span::styled(
            " (Esc to clear)",
            Style::default().fg(app.theme_muted_color()),
        ));
    }

    let line = Line::from(spans);
    let para = Paragraph::new(line).style(Style::default().bg(app.theme_surface_color()));
    frame.render_widget(para, area);
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

/// JSON line highlighting with search term highlighting
fn highlight_json_line_with_search(line: &str, search: &str, accent: Color) -> Vec<Span<'static>> {
    if search.is_empty() {
        return highlight_json_line(line);
    }

    let line_lower = line.to_lowercase();
    let mut spans = Vec::new();
    let mut last_end = 0;

    for (start, _) in line_lower.match_indices(search) {
        // Add everything before the match with normal highlighting
        if start > last_end {
            let before = &line[last_end..start];
            spans.extend(highlight_json_line(before));
        }

        // Add the match with highlight
        let end = start + search.len();
        let matched = &line[start..end];
        spans.push(Span::styled(
            matched.to_string(),
            Style::default()
                .fg(Color::Black)
                .bg(accent)
                .add_modifier(Modifier::BOLD),
        ));

        last_end = end;
    }

    // Add remaining text
    if last_end < line.len() {
        let remaining = &line[last_end..];
        spans.extend(highlight_json_line(remaining));
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
