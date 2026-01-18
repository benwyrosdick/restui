use crate::app::{App, EditingField, FocusedPanel, InputMode, RequestTab};
use crate::storage::AuthType;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Tabs},
    Frame,
};

use super::layout::bordered_block;

/// Helper to create spans with cursor for editing text fields
fn text_with_cursor<'a>(text: &str, cursor_pos: usize, is_editing: bool, placeholder: &str, base_style: Style) -> Vec<Span<'a>> {
    if is_editing {
        let pos = cursor_pos.min(text.len());
        let (before, after) = text.split_at(pos);
        vec![
            Span::styled(before.to_string(), Style::default().bg(Color::DarkGray)),
            Span::styled("│", Style::default().fg(Color::White).bg(Color::DarkGray)),
            Span::styled(after.to_string(), Style::default().bg(Color::DarkGray)),
        ]
    } else if text.is_empty() {
        vec![Span::styled(placeholder.to_string(), Style::default().fg(Color::DarkGray))]
    } else {
        vec![Span::styled(text.to_string(), base_style)]
    }
}

pub fn draw(frame: &mut Frame, app: &mut App, area: Rect) {
    let focused = app.focused_panel == FocusedPanel::RequestEditor;
    let block = bordered_block("Request", focused);
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

    draw_tabs(frame, app, chunks[0]);

    match app.request_tab {
        RequestTab::Headers => draw_headers(frame, app, chunks[1]),
        RequestTab::Body => draw_body(frame, app, chunks[1]),
        RequestTab::Auth => draw_auth(frame, app, chunks[1]),
        RequestTab::Params => draw_params(frame, app, chunks[1]),
    }
}

fn draw_tabs(frame: &mut Frame, app: &mut App, area: Rect) {
    let tabs_list = RequestTab::all();
    let titles: Vec<Line> = tabs_list
        .iter()
        .map(|t| {
            let style = if *t == app.request_tab {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
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

fn draw_headers(frame: &mut Frame, app: &App, area: Rect) {
    let mut lines: Vec<Line> = Vec::new();

    for (i, header) in app.current_request.headers.iter().enumerate() {
        let enabled_indicator = if header.enabled { "●" } else { "○" };

        let is_editing_key = app.input_mode == InputMode::Editing
            && app.editing_field == Some(EditingField::HeaderKey(i));
        let is_editing_value = app.input_mode == InputMode::Editing
            && app.editing_field == Some(EditingField::HeaderValue(i));

        let mut spans = vec![
            Span::styled(
                format!("{} ", enabled_indicator),
                Style::default().fg(if header.enabled {
                    Color::Green
                } else {
                    Color::DarkGray
                }),
            ),
        ];

        spans.extend(text_with_cursor(
            &header.key,
            app.cursor_position,
            is_editing_key,
            "key",
            Style::default().fg(Color::Cyan),
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
            Style::default().fg(Color::DarkGray),
        )));
    }

    let para = Paragraph::new(lines);
    frame.render_widget(para, area);
}

fn draw_body(frame: &mut Frame, app: &App, area: Rect) {
    let is_editing =
        app.input_mode == InputMode::Editing && app.editing_field == Some(EditingField::Body);

    let body_spans = text_with_cursor(
        &app.current_request.body,
        app.cursor_position,
        is_editing,
        "Enter request body...",
        Style::default(),
    );

    let para = Paragraph::new(Line::from(body_spans))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(if is_editing {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default().fg(Color::DarkGray)
                })
                .title(" Body (JSON) "),
        );

    frame.render_widget(para, area);
}

fn draw_auth(frame: &mut Frame, app: &App, area: Rect) {
    let auth = &app.current_request.auth;

    let mut lines: Vec<Line> = Vec::new();

    // Auth type selector
    lines.push(Line::from(vec![
        Span::styled("Type: ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            auth.auth_type.as_str(),
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" (press 'a' to cycle)", Style::default().fg(Color::DarkGray)),
    ]));

    lines.push(Line::from(""));

    // Show relevant fields based on auth type
    match auth.auth_type {
        AuthType::None => {
            lines.push(Line::from(Span::styled(
                "No authentication configured.",
                Style::default().fg(Color::DarkGray),
            )));
        }
        AuthType::Bearer => {
            let is_editing = app.input_mode == InputMode::Editing
                && app.editing_field == Some(EditingField::AuthBearerToken);

            let mut spans = vec![Span::styled("Token: ", Style::default().fg(Color::DarkGray))];
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
                spans.push(Span::styled("Enter token...", Style::default().fg(Color::DarkGray)));
            } else {
                // Truncate when not editing
                spans.push(Span::styled(
                    format!("{}...", &auth.bearer_token.chars().take(20).collect::<String>()),
                    Style::default(),
                ));
            }
            lines.push(Line::from(spans));
        }
        AuthType::Basic => {
            let is_editing_user = app.input_mode == InputMode::Editing
                && app.editing_field == Some(EditingField::AuthBasicUsername);
            let is_editing_pass = app.input_mode == InputMode::Editing
                && app.editing_field == Some(EditingField::AuthBasicPassword);

            let mut user_spans = vec![Span::styled("Username: ", Style::default().fg(Color::DarkGray))];
            user_spans.extend(text_with_cursor(
                &auth.basic_username,
                app.cursor_position,
                is_editing_user,
                "Enter username...",
                Style::default(),
            ));
            lines.push(Line::from(user_spans));

            // Password field - show masked or with cursor
            let mut pass_spans = vec![Span::styled("Password: ", Style::default().fg(Color::DarkGray))];
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
                pass_spans.push(Span::styled("Enter password...", Style::default().fg(Color::DarkGray)));
            } else {
                pass_spans.push(Span::styled("*".repeat(auth.basic_password.len()), Style::default()));
            }
            lines.push(Line::from(pass_spans));
        }
        AuthType::ApiKey => {
            let is_editing_name = app.input_mode == InputMode::Editing
                && app.editing_field == Some(EditingField::AuthApiKeyName);
            let is_editing_value = app.input_mode == InputMode::Editing
                && app.editing_field == Some(EditingField::AuthApiKeyValue);

            let mut name_spans = vec![Span::styled("Key Name: ", Style::default().fg(Color::DarkGray))];
            name_spans.extend(text_with_cursor(
                &auth.api_key_name,
                app.cursor_position,
                is_editing_name,
                "e.g., X-API-Key",
                Style::default(),
            ));
            lines.push(Line::from(name_spans));

            let mut value_spans = vec![Span::styled("Key Value: ", Style::default().fg(Color::DarkGray))];
            if is_editing_value {
                value_spans.extend(text_with_cursor(
                    &auth.api_key_value,
                    app.cursor_position,
                    true,
                    "Enter API key...",
                    Style::default(),
                ));
            } else if auth.api_key_value.is_empty() {
                value_spans.push(Span::styled("Enter API key...", Style::default().fg(Color::DarkGray)));
            } else {
                // Truncate when not editing
                value_spans.push(Span::styled(
                    format!("{}...", &auth.api_key_value.chars().take(20).collect::<String>()),
                    Style::default(),
                ));
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
                    Style::default().fg(Color::Cyan),
                ),
            ]));
        }
    }

    let para = Paragraph::new(lines);
    frame.render_widget(para, area);
}

fn draw_params(frame: &mut Frame, app: &App, area: Rect) {
    let mut lines: Vec<Line> = Vec::new();

    for (i, param) in app.current_request.query_params.iter().enumerate() {
        let enabled_indicator = if param.enabled { "●" } else { "○" };

        let is_editing_key = app.input_mode == InputMode::Editing
            && app.editing_field == Some(EditingField::ParamKey(i));
        let is_editing_value = app.input_mode == InputMode::Editing
            && app.editing_field == Some(EditingField::ParamValue(i));

        let mut spans = vec![
            Span::styled(
                format!("{} ", enabled_indicator),
                Style::default().fg(if param.enabled {
                    Color::Green
                } else {
                    Color::DarkGray
                }),
            ),
        ];

        spans.extend(text_with_cursor(
            &param.key,
            app.cursor_position,
            is_editing_key,
            "key",
            Style::default().fg(Color::Cyan),
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
