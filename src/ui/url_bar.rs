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
        crate::storage::HttpMethod::Patch => Color::Magenta,
        crate::storage::HttpMethod::Delete => Color::Red,
    };

    // URL display with cursor and selection if editing
    let url_text = &app.current_request.url;
    let url_spans: Vec<Span> = if is_editing {
        let editing_style = Style::default().bg(Color::DarkGray);
        let cursor_style = Style::default().bg(Color::White).fg(Color::Black);
        let selection_style = Style::default().bg(Color::Blue).fg(Color::White);

        let char_count = url_text.chars().count();
        let cursor_pos = app.cursor_position.min(char_count);
        let selection = app.get_selection_range();

        if url_text.is_empty() {
            // Empty text, show block cursor as a space
            vec![Span::styled(" ", cursor_style)]
        } else if let Some((sel_start, sel_end)) = selection {
            // Clamp selection bounds to valid range
            let sel_start = sel_start.min(char_count);
            let sel_end = sel_end.min(char_count);

            if sel_start != sel_end {
                // We have a selection
                let chars: Vec<char> = url_text.chars().collect();
                let mut spans = Vec::new();

                if sel_start > 0 {
                    let before: String = chars[..sel_start].iter().collect();
                    spans.push(Span::styled(before, editing_style));
                }

                let selected: String = chars[sel_start..sel_end].iter().collect();
                spans.push(Span::styled(selected, selection_style));

                if sel_end < char_count {
                    let after: String = chars[sel_end..].iter().collect();
                    spans.push(Span::styled(after, editing_style));
                }

                // Cursor at end if past text
                if cursor_pos >= char_count {
                    spans.push(Span::styled(" ", cursor_style));
                }

                spans
            } else {
                // Selection collapsed - show just cursor
                render_url_with_cursor(
                    url_text,
                    cursor_pos,
                    char_count,
                    editing_style,
                    cursor_style,
                )
            }
        } else {
            // No selection, just cursor
            render_url_with_cursor(
                url_text,
                cursor_pos,
                char_count,
                editing_style,
                cursor_style,
            )
        }
    } else if url_text.is_empty() {
        vec![Span::styled(
            "Enter URL... (press Enter or 'i' to edit)",
            Style::default().fg(app.theme_muted_color()),
        )]
    } else {
        vec![Span::styled(
            url_text.clone(),
            Style::default().fg(app.theme_text_color()),
        )]
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
        Style::default().fg(app.theme_muted_color())
    };

    let title_style = if focused {
        Style::default().fg(accent).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(app.theme_muted_color())
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .style(Style::default().bg(app.theme_surface_color()))
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

fn render_url_with_cursor<'a>(
    url_text: &str,
    cursor_pos: usize,
    char_count: usize,
    editing_style: Style,
    cursor_style: Style,
) -> Vec<Span<'a>> {
    if cursor_pos >= char_count {
        // Cursor at end, show block cursor after text
        vec![
            Span::styled(url_text.to_string(), editing_style),
            Span::styled(" ", cursor_style),
        ]
    } else {
        // Cursor in middle, highlight character under cursor
        let chars: Vec<char> = url_text.chars().collect();
        let mut spans = Vec::new();

        if cursor_pos > 0 {
            let before: String = chars[..cursor_pos].iter().collect();
            spans.push(Span::styled(before, editing_style));
        }

        spans.push(Span::styled(chars[cursor_pos].to_string(), cursor_style));

        if cursor_pos + 1 < char_count {
            let after: String = chars[cursor_pos + 1..].iter().collect();
            spans.push(Span::styled(after, editing_style));
        }

        spans
    }
}
