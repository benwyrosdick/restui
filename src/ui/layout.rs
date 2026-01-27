use crate::app::{App, FocusedPanel, InputMode, RequestTab};
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
    let version = env!("CARGO_PKG_VERSION");

    // Split header into left and right halves
    let header_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    let theme = app.theme();
    let accent = app.accent_color();

    let left_content = Line::from(vec![
        Span::styled(
            " ResTUI ",
            Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("v{}", version),
            Style::default()
                .fg(theme.muted)
                .add_modifier(Modifier::BOLD),
        ),
    ]);
    let left_header = Paragraph::new(left_content).style(Style::default().bg(theme.surface));
    frame.render_widget(left_header, header_chunks[0]);

    let right_content = Line::from(vec![
        Span::styled("Env: ", Style::default().fg(theme.text)),
        Span::styled(
            env_name,
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
    ]);
    let right_header = Paragraph::new(right_content)
        .style(Style::default().bg(theme.surface))
        .alignment(ratatui::layout::Alignment::Right);
    frame.render_widget(right_header, header_chunks[1]);
}

fn draw_footer(frame: &mut Frame, app: &App, area: Rect) {
    let mode_indicator = if app.pending_move.is_some() {
        Span::styled(
            " MOVE ",
            Style::default().bg(Color::Yellow).fg(Color::Black),
        )
    } else {
        match app.input_mode {
            InputMode::Normal => Span::styled(
                " NORMAL ",
                Style::default().bg(Color::DarkGray).fg(Color::White),
            ),
            InputMode::Editing => Span::styled(
                " EDITING ",
                Style::default().bg(Color::Green).fg(Color::Black),
            ),
        }
    };

    // Build footer: mode indicator + optional status + shortcuts
    let mut footer_spans = vec![mode_indicator, Span::raw(" ")];

    // Show status/error message if present
    if app.is_loading {
        footer_spans.push(Span::styled(
            format!("Sending request {} ", app.spinner_frame()),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ));
    } else if let Some(err) = &app.error_message {
        footer_spans.push(Span::styled(
            format!("{} ", err),
            Style::default().fg(Color::Red),
        ));
        footer_spans.push(Span::styled(
            "│ ",
            Style::default().fg(app.theme_muted_color()),
        ));
    } else if let Some(status) = &app.status_message {
        footer_spans.push(Span::styled(
            format!("{} ", status),
            Style::default().fg(app.accent_color()),
        ));
        footer_spans.push(Span::styled(
            "│ ",
            Style::default().fg(app.theme_muted_color()),
        ));
    }

    // Always show shortcuts (except when loading)
    if !app.is_loading {
        footer_spans.extend(get_panel_shortcuts(app));
    }

    let footer_content = Line::from(footer_spans);
    let footer =
        Paragraph::new(footer_content).style(Style::default().bg(app.theme_surface_color()));

    frame.render_widget(footer, area);
}

/// Build a shortcut hint span with highlighted key
fn shortcut(key: &str, desc: &str, accent: Color, muted: Color) -> Vec<Span<'static>> {
    vec![
        Span::styled(key.to_string(), Style::default().fg(accent)),
        Span::styled(format!(":{} ", desc), Style::default().fg(muted)),
    ]
}

/// Get context-sensitive keyboard shortcuts for the current panel
fn get_panel_shortcuts(app: &App) -> Vec<Span<'static>> {
    let accent = app.theme_accent_color(); // Use theme's accent for shortcut hints
    let muted = app.theme_muted_color();
    let mut spans = Vec::new();

    match app.input_mode {
        InputMode::Editing => {
            spans.extend(shortcut("Esc", "exit", accent, muted));
            spans.extend(shortcut("Tab", "next field", accent, muted));
        }
        InputMode::Normal => {
            match app.focused_panel {
                FocusedPanel::RequestList => {
                    if app.request_list_search_active {
                        spans.extend(shortcut("Enter", "confirm", accent, muted));
                        spans.extend(shortcut("Esc", "cancel", accent, muted));
                    } else if app.has_request_list_filter() {
                        spans.extend(shortcut("Enter", "select", accent, muted));
                        spans.extend(shortcut("s", "send", accent, muted));
                        spans.extend(shortcut("Esc", "clear", accent, muted));
                    } else {
                        spans.extend(shortcut("/", "search", accent, muted));
                        spans.extend(shortcut("s", "send", accent, muted));
                        spans.extend(shortcut("Space", "expand", accent, muted));
                        spans.extend(shortcut("H", "history", accent, muted));
                    }
                }
                FocusedPanel::UrlBar => {
                    spans.extend(shortcut("Enter", "edit", accent, muted));
                    spans.extend(shortcut("s", "send", accent, muted));
                    spans.extend(shortcut("m", "method", accent, muted));
                    spans.extend(shortcut("e", "env", accent, muted));
                }
                FocusedPanel::RequestEditor => {
                    spans.extend(shortcut("Enter", "edit", accent, muted));
                    spans.extend(shortcut("h/l", "tabs", accent, muted));
                    spans.extend(shortcut("s", "send", accent, muted));
                    match app.request_tab {
                        RequestTab::Body => {
                            spans.extend(shortcut("f", "format", accent, muted));
                        }
                        RequestTab::Auth => {
                            spans.extend(shortcut("a", "auth type", accent, muted));
                        }
                        RequestTab::Headers | RequestTab::Params => {
                            spans.extend(shortcut("t", "toggle", accent, muted));
                            spans.extend(shortcut("x", "delete", accent, muted));
                        }
                    }
                }
                FocusedPanel::ResponseView => {
                    spans.extend(shortcut("/", "search", accent, muted));
                    spans.extend(shortcut("f", "filter", accent, muted));
                    spans.extend(shortcut("c", "copy", accent, muted));
                    spans.extend(shortcut("S", "save", accent, muted));
                    spans.extend(shortcut("s", "send", accent, muted));
                }
            }
            // Always show these at the end
            spans.extend(shortcut("?", "help", accent, muted));
            spans.extend(shortcut("q", "quit", accent, muted));
        }
    }

    spans
}

/// Helper to create a bordered block with focus indication
pub fn bordered_block(
    title: &str,
    focused: bool,
    accent: Color,
    surface: Color,
    muted: Color,
) -> Block<'_> {
    bordered_block_with_number(title, focused, accent, surface, muted, None)
}

/// Helper to create a bordered block with focus indication and optional panel number
pub fn bordered_block_with_number(
    title: &str,
    focused: bool,
    accent: Color,
    surface: Color,
    muted: Color,
    panel_number: Option<u8>,
) -> Block<'_> {
    let border_style = if focused {
        Style::default().fg(accent)
    } else {
        Style::default().fg(muted)
    };

    let title_text = if let Some(num) = panel_number {
        let subscript = match num {
            1 => "₁",
            2 => "₂",
            3 => "₃",
            4 => "₄",
            5 => "₅",
            6 => "₆",
            7 => "₇",
            8 => "₈",
            9 => "₉",
            0 => "₀",
            _ => "",
        };
        format!(" {}{} ", title, subscript)
    } else {
        format!(" {} ", title)
    };

    Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .style(Style::default().bg(surface))
        .title(title_text)
        .title_style(if focused {
            Style::default().fg(accent).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(muted)
        })
}
