use crate::app::{App, EditingField, FocusedPanel, InputMode, RequestTab};
use crate::storage::AuthType;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Tabs},
    Frame,
};

use super::layout::bordered_block;
use super::widgets::text_with_cursor;

pub fn draw(frame: &mut Frame, app: &mut App, area: Rect) {
    let focused = app.focused_panel == FocusedPanel::RequestEditor;
    let accent = app.accent_color();
    let block = bordered_block(
        "Request",
        focused,
        accent,
        app.theme_surface_color(),
        app.theme_muted_color(),
    );
    let inner_area = block.inner(area);
    frame.render_widget(block, area);

    // Split into: tabs, content
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Tabs
            Constraint::Min(3),    // Content
        ])
        .split(inner_area);

    // Store layout areas for mouse click detection
    app.layout_areas.tabs_row_y = Some(chunks[0].y);
    app.layout_areas.request_content_area =
        Some((chunks[1].x, chunks[1].y, chunks[1].width, chunks[1].height));

    draw_tabs(frame, app, chunks[0], accent);

    match app.request_tab {
        RequestTab::Headers => draw_headers(frame, app, chunks[1], accent),
        RequestTab::Body => draw_body(frame, app, chunks[1]),
        RequestTab::Auth => draw_auth(frame, app, chunks[1], accent),
        RequestTab::Params => draw_params(frame, app, chunks[1], accent),
    }
}

fn draw_tabs(frame: &mut Frame, app: &mut App, area: Rect, accent: Color) {
    let tabs_list = RequestTab::all();
    let titles: Vec<Line> = tabs_list
        .iter()
        .map(|t| {
            let style = if *t == app.request_tab {
                Style::default().fg(accent).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(app.theme_muted_color())
            };
            Line::styled(t.as_str(), style)
        })
        .collect();

    // Calculate and store tab positions for mouse click detection
    // Format: "Headers | Body | Auth | Params"
    // Each tab has its text width, plus separator " | " (3 chars) between tabs
    let mut tab_positions = Vec::new();
    let mut current_x = area.x;
    for tab in tabs_list {
        let tab_width = tab.as_str().len() as u16;
        tab_positions.push((current_x, tab_width, *tab));
        // Add tab width + separator " | " (3 chars)
        current_x += tab_width + 3;
    }
    app.layout_areas.tab_positions = tab_positions;

    let tabs = Tabs::new(titles)
        .select(app.request_tab as usize)
        .divider("|");

    frame.render_widget(tabs, area);
}

fn draw_headers(frame: &mut Frame, app: &App, area: Rect, accent: Color) {
    let mut lines: Vec<Line> = Vec::new();
    let is_focused = app.focused_panel == FocusedPanel::RequestEditor
        && app.request_tab == RequestTab::Headers
        && app.input_mode == InputMode::Normal;

    for (i, header) in app.current_request.headers.iter().enumerate() {
        let is_selected = is_focused && i == app.selected_header_index;
        let enabled_indicator = if header.enabled { "●" } else { "○" };

        let is_editing_key = app.input_mode == InputMode::Editing
            && app.editing_field == Some(EditingField::HeaderKey(i));
        let is_editing_value = app.input_mode == InputMode::Editing
            && app.editing_field == Some(EditingField::HeaderValue(i));

        let mut spans = vec![];

        // Selection indicator
        if is_selected {
            spans.push(Span::styled("> ", Style::default().fg(accent)));
        } else {
            spans.push(Span::raw("  "));
        }

        spans.push(Span::styled(
            format!("{} ", enabled_indicator),
            Style::default().fg(if header.enabled {
                Color::Green
            } else {
                Color::DarkGray
            }),
        ));

        spans.extend(text_with_cursor(
            &header.key,
            app.cursor_position,
            is_editing_key,
            "key",
            Style::default().fg(accent),
        ));

        spans.push(Span::raw(": "));

        spans.extend(text_with_cursor(
            &header.value,
            app.cursor_position,
            is_editing_value,
            "value",
            Style::default(),
        ));

        lines.push(Line::from(spans));
    }

    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "No headers. Press Enter to add.",
            Style::default().fg(app.theme_muted_color()),
        )));
    }

    let para = Paragraph::new(lines);
    frame.render_widget(para, area);
}

fn draw_body(frame: &mut Frame, app: &mut App, area: Rect) {
    let is_editing =
        app.input_mode == InputMode::Editing && app.editing_field == Some(EditingField::Body);

    let body = &app.current_request.body;

    let lines: Vec<Line> = if body.is_empty() && !is_editing {
        vec![Line::from(Span::styled(
            "Enter request body...",
            Style::default().fg(app.theme_muted_color()),
        ))]
    } else if is_editing {
        // When editing, we need to show cursor at the right position across lines
        let cursor_pos = app.cursor_position.min(body.len());
        let mut result_lines = Vec::new();
        let mut char_count = 0;
        let mut cursor_rendered = false;

        for line_text in body.split('\n') {
            let line_start = char_count;
            let line_end = char_count + line_text.len();

            if !cursor_rendered && cursor_pos >= line_start && cursor_pos <= line_end {
                // Cursor is on this line
                let pos_in_line = cursor_pos - line_start;
                cursor_rendered = true;

                if pos_in_line >= line_text.len() {
                    // Cursor at end of line, show block cursor after text
                    result_lines.push(Line::from(vec![
                        Span::styled(line_text.to_string(), Style::default().bg(Color::DarkGray)),
                        Span::styled(" ", Style::default().bg(Color::White).fg(Color::Black)),
                    ]));
                } else {
                    // Cursor in middle, highlight character under cursor
                    let (before, rest) = line_text.split_at(pos_in_line);
                    let mut chars = rest.chars();
                    let cursor_char = chars.next().unwrap_or(' ');
                    let after: String = chars.collect();
                    result_lines.push(Line::from(vec![
                        Span::styled(before.to_string(), Style::default().bg(Color::DarkGray)),
                        Span::styled(
                            cursor_char.to_string(),
                            Style::default().bg(Color::White).fg(Color::Black),
                        ),
                        Span::styled(after, Style::default().bg(Color::DarkGray)),
                    ]));
                }
            } else {
                result_lines.push(Line::from(Span::styled(
                    line_text.to_string(),
                    Style::default().bg(Color::DarkGray),
                )));
            }

            // Account for the newline character (except for the last line)
            char_count = line_end + 1;
        }

        // Handle empty body with cursor
        if result_lines.is_empty() {
            result_lines.push(Line::from(Span::styled(
                " ",
                Style::default().bg(Color::White).fg(Color::Black),
            )));
        }

        result_lines
    } else {
        // Not editing, just display lines normally
        body.split('\n')
            .map(|line| Line::from(Span::raw(line.to_string())))
            .collect()
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(if is_editing {
            Style::default().fg(Color::Green)
        } else {
            Style::default().fg(app.theme_muted_color())
        })
        .style(Style::default().bg(app.theme_surface_color()))
        .title(format!(" Body ({}) ", app.body_format_label()));

    // Store inner area for click-to-cursor positioning
    let inner_area = block.inner(area);
    app.layout_areas.body_area = Some((
        inner_area.x,
        inner_area.y,
        inner_area.width,
        inner_area.height,
    ));

    let total_lines = lines.len() as u16;

    let para = Paragraph::new(lines)
        .block(block)
        .scroll((app.body_scroll, 0));
    frame.render_widget(para, area);

    // Render scrollbar if content is larger than area
    if total_lines > inner_area.height {
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("↑"))
            .end_symbol(Some("↓"));

        let mut scrollbar_state =
            ScrollbarState::new(total_lines as usize).position(app.body_scroll as usize);

        frame.render_stateful_widget(scrollbar, area, &mut scrollbar_state);
    }
}

fn draw_auth(frame: &mut Frame, app: &App, area: Rect, accent: Color) {
    let auth = &app.current_request.auth;

    let mut lines: Vec<Line> = Vec::new();

    // Auth type selector
    lines.push(Line::from(vec![
        Span::styled("Type: ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            auth.auth_type.as_str(),
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            " (press 'a' to cycle)",
            Style::default().fg(Color::DarkGray),
        ),
    ]));

    lines.push(Line::from(""));

    // Show relevant fields based on auth type
    match auth.auth_type {
        AuthType::None => {
            lines.push(Line::from(Span::styled(
                "No authentication configured.",
                Style::default().fg(app.theme_muted_color()),
            )));
        }
        AuthType::Bearer => {
            let is_editing = app.input_mode == InputMode::Editing
                && app.editing_field == Some(EditingField::AuthBearerToken);

            let mut spans = vec![Span::styled(
                "Token: ",
                Style::default().fg(Color::DarkGray),
            )];
            if is_editing {
                // Show full token with cursor when editing
                spans.extend(text_with_cursor(
                    &auth.bearer_token,
                    app.cursor_position,
                    true,
                    "Enter token...",
                    Style::default(),
                ));
            } else if auth.bearer_token.is_empty() {
                spans.push(Span::styled(
                    "Enter token...",
                    Style::default().fg(Color::DarkGray),
                ));
            } else {
                spans.push(Span::styled(&auth.bearer_token, Style::default()));
            }
            lines.push(Line::from(spans));
        }
        AuthType::Basic => {
            let is_editing_user = app.input_mode == InputMode::Editing
                && app.editing_field == Some(EditingField::AuthBasicUsername);
            let is_editing_pass = app.input_mode == InputMode::Editing
                && app.editing_field == Some(EditingField::AuthBasicPassword);

            let mut user_spans = vec![Span::styled(
                "Username: ",
                Style::default().fg(Color::DarkGray),
            )];
            user_spans.extend(text_with_cursor(
                &auth.basic_username,
                app.cursor_position,
                is_editing_user,
                "Enter username...",
                Style::default(),
            ));
            lines.push(Line::from(user_spans));

            // Password field - show masked or with cursor
            let mut pass_spans = vec![Span::styled(
                "Password: ",
                Style::default().fg(Color::DarkGray),
            )];
            if is_editing_pass {
                // Show masked password with cursor at correct position
                let masked = "*".repeat(auth.basic_password.len());
                pass_spans.extend(text_with_cursor(
                    &masked,
                    app.cursor_position,
                    true,
                    "Enter password...",
                    Style::default(),
                ));
            } else if auth.basic_password.is_empty() {
                pass_spans.push(Span::styled(
                    "Enter password...",
                    Style::default().fg(Color::DarkGray),
                ));
            } else {
                pass_spans.push(Span::styled(
                    "*".repeat(auth.basic_password.len()),
                    Style::default(),
                ));
            }
            lines.push(Line::from(pass_spans));
        }
        AuthType::ApiKey => {
            let is_editing_name = app.input_mode == InputMode::Editing
                && app.editing_field == Some(EditingField::AuthApiKeyName);
            let is_editing_value = app.input_mode == InputMode::Editing
                && app.editing_field == Some(EditingField::AuthApiKeyValue);

            let mut name_spans = vec![Span::styled(
                "Key Name: ",
                Style::default().fg(Color::DarkGray),
            )];
            name_spans.extend(text_with_cursor(
                &auth.api_key_name,
                app.cursor_position,
                is_editing_name,
                "e.g., X-API-Key",
                Style::default(),
            ));
            lines.push(Line::from(name_spans));

            let mut value_spans = vec![Span::styled(
                "Key Value: ",
                Style::default().fg(Color::DarkGray),
            )];
            if is_editing_value {
                value_spans.extend(text_with_cursor(
                    &auth.api_key_value,
                    app.cursor_position,
                    true,
                    "Enter API key...",
                    Style::default(),
                ));
            } else if auth.api_key_value.is_empty() {
                value_spans.push(Span::styled(
                    "Enter API key...",
                    Style::default().fg(Color::DarkGray),
                ));
            } else {
                value_spans.push(Span::styled(&auth.api_key_value, Style::default()));
            }
            lines.push(Line::from(value_spans));

            lines.push(Line::from(vec![
                Span::styled("Location: ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    if auth.api_key_location == "query" {
                        "Query Parameter"
                    } else {
                        "Header"
                    },
                    Style::default().fg(accent),
                ),
            ]));
        }
    }

    let para = Paragraph::new(lines);
    frame.render_widget(para, area);
}

fn draw_params(frame: &mut Frame, app: &App, area: Rect, accent: Color) {
    let mut lines: Vec<Line> = Vec::new();
    let is_focused = app.focused_panel == FocusedPanel::RequestEditor
        && app.request_tab == RequestTab::Params
        && app.input_mode == InputMode::Normal;

    for (i, param) in app.current_request.query_params.iter().enumerate() {
        let is_selected = is_focused && i == app.selected_param_index;
        let enabled_indicator = if param.enabled { "●" } else { "○" };

        let is_editing_key = app.input_mode == InputMode::Editing
            && app.editing_field == Some(EditingField::ParamKey(i));
        let is_editing_value = app.input_mode == InputMode::Editing
            && app.editing_field == Some(EditingField::ParamValue(i));

        let mut spans = vec![];

        // Selection indicator
        if is_selected {
            spans.push(Span::styled("> ", Style::default().fg(accent)));
        } else {
            spans.push(Span::raw("  "));
        }

        spans.push(Span::styled(
            format!("{} ", enabled_indicator),
            Style::default().fg(if param.enabled {
                Color::Green
            } else {
                Color::DarkGray
            }),
        ));

        spans.extend(text_with_cursor(
            &param.key,
            app.cursor_position,
            is_editing_key,
            "key",
            Style::default().fg(accent),
        ));

        spans.push(Span::raw("="));

        spans.extend(text_with_cursor(
            &param.value,
            app.cursor_position,
            is_editing_value,
            "value",
            Style::default(),
        ));

        lines.push(Line::from(spans));
    }

    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "No query parameters. Press Enter to add.",
            Style::default().fg(Color::DarkGray),
        )));
    }

    let para = Paragraph::new(lines);
    frame.render_widget(para, area);
}
