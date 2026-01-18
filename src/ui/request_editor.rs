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

        let key_display = if is_editing_key {
            format!("{}|", &header.key)
        } else if header.key.is_empty() {
            "key".to_string()
        } else {
            header.key.clone()
        };

        let value_display = if is_editing_value {
            format!("{}|", &header.value)
        } else if header.value.is_empty() {
            "value".to_string()
        } else {
            header.value.clone()
        };

        lines.push(Line::from(vec![
            Span::styled(
                format!("{} ", enabled_indicator),
                Style::default().fg(if header.enabled {
                    Color::Green
                } else {
                    Color::DarkGray
                }),
            ),
            Span::styled(
                key_display,
                if is_editing_key {
                    Style::default().bg(Color::DarkGray)
                } else if header.key.is_empty() {
                    Style::default().fg(Color::DarkGray)
                } else {
                    Style::default().fg(Color::Cyan)
                },
            ),
            Span::raw(": "),
            Span::styled(
                value_display,
                if is_editing_value {
                    Style::default().bg(Color::DarkGray)
                } else if header.value.is_empty() {
                    Style::default().fg(Color::DarkGray)
                } else {
                    Style::default()
                },
            ),
        ]));
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

    let body_display = if is_editing {
        format!("{}|", &app.current_request.body)
    } else if app.current_request.body.is_empty() {
        "Enter request body...".to_string()
    } else {
        app.current_request.body.clone()
    };

    let style = if app.current_request.body.is_empty() && !is_editing {
        Style::default().fg(Color::DarkGray)
    } else if is_editing {
        Style::default().bg(Color::DarkGray)
    } else {
        Style::default()
    };

    let para = Paragraph::new(body_display)
        .style(style)
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

            let token_display = if is_editing {
                format!("{}|", &auth.bearer_token)
            } else if auth.bearer_token.is_empty() {
                "Enter token...".to_string()
            } else {
                // Mask the token for display
                format!("{}...", &auth.bearer_token.chars().take(20).collect::<String>())
            };

            lines.push(Line::from(vec![
                Span::styled("Token: ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    token_display,
                    if is_editing {
                        Style::default().bg(Color::DarkGray)
                    } else if auth.bearer_token.is_empty() {
                        Style::default().fg(Color::DarkGray)
                    } else {
                        Style::default()
                    },
                ),
            ]));
        }
        AuthType::Basic => {
            let is_editing_user = app.input_mode == InputMode::Editing
                && app.editing_field == Some(EditingField::AuthBasicUsername);
            let is_editing_pass = app.input_mode == InputMode::Editing
                && app.editing_field == Some(EditingField::AuthBasicPassword);

            lines.push(Line::from(vec![
                Span::styled("Username: ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    if is_editing_user {
                        format!("{}|", &auth.basic_username)
                    } else if auth.basic_username.is_empty() {
                        "Enter username...".to_string()
                    } else {
                        auth.basic_username.clone()
                    },
                    if is_editing_user {
                        Style::default().bg(Color::DarkGray)
                    } else {
                        Style::default()
                    },
                ),
            ]));

            lines.push(Line::from(vec![
                Span::styled("Password: ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    if is_editing_pass {
                        format!("{}|", "*".repeat(auth.basic_password.len()))
                    } else if auth.basic_password.is_empty() {
                        "Enter password...".to_string()
                    } else {
                        "*".repeat(auth.basic_password.len())
                    },
                    if is_editing_pass {
                        Style::default().bg(Color::DarkGray)
                    } else {
                        Style::default()
                    },
                ),
            ]));
        }
        AuthType::ApiKey => {
            let is_editing_name = app.input_mode == InputMode::Editing
                && app.editing_field == Some(EditingField::AuthApiKeyName);
            let is_editing_value = app.input_mode == InputMode::Editing
                && app.editing_field == Some(EditingField::AuthApiKeyValue);

            lines.push(Line::from(vec![
                Span::styled("Key Name: ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    if is_editing_name {
                        format!("{}|", &auth.api_key_name)
                    } else if auth.api_key_name.is_empty() {
                        "e.g., X-API-Key".to_string()
                    } else {
                        auth.api_key_name.clone()
                    },
                    if is_editing_name {
                        Style::default().bg(Color::DarkGray)
                    } else {
                        Style::default()
                    },
                ),
            ]));

            lines.push(Line::from(vec![
                Span::styled("Key Value: ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    if is_editing_value {
                        format!("{}|", &auth.api_key_value)
                    } else if auth.api_key_value.is_empty() {
                        "Enter API key...".to_string()
                    } else {
                        format!("{}...", &auth.api_key_value.chars().take(20).collect::<String>())
                    },
                    if is_editing_value {
                        Style::default().bg(Color::DarkGray)
                    } else {
                        Style::default()
                    },
                ),
            ]));

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

        let key_display = if is_editing_key {
            format!("{}|", &param.key)
        } else if param.key.is_empty() {
            "key".to_string()
        } else {
            param.key.clone()
        };

        let value_display = if is_editing_value {
            format!("{}|", &param.value)
        } else if param.value.is_empty() {
            "value".to_string()
        } else {
            param.value.clone()
        };

        lines.push(Line::from(vec![
            Span::styled(
                format!("{} ", enabled_indicator),
                Style::default().fg(if param.enabled {
                    Color::Green
                } else {
                    Color::DarkGray
                }),
            ),
            Span::styled(
                key_display,
                if is_editing_key {
                    Style::default().bg(Color::DarkGray)
                } else if param.key.is_empty() {
                    Style::default().fg(Color::DarkGray)
                } else {
                    Style::default().fg(Color::Cyan)
                },
            ),
            Span::raw("="),
            Span::styled(
                value_display,
                if is_editing_value {
                    Style::default().bg(Color::DarkGray)
                } else if param.value.is_empty() {
                    Style::default().fg(Color::DarkGray)
                } else {
                    Style::default()
                },
            ),
        ]));
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
