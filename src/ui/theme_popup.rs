use crate::app::{App, Theme};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Flex, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

pub fn draw_theme_popup(frame: &mut Frame, app: &App) {
    let accent = app.accent_color();
    let theme = app.theme();

    let max_name_len = app.themes.iter().map(|t| t.name.len()).max().unwrap_or(8);

    // Calculate popup size - list height + preview height + borders
    let list_height = app.themes.len() as u16;
    let preview_height: u16 = 12; // Mini app preview with border and padding
    let popup_width = 50u16;
    let popup_height = list_height + preview_height + 4; // +4 for borders and footer

    let area = centered_rect(popup_width, popup_height, frame.area());
    frame.render_widget(Clear, area);

    // Split into list area and preview area
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(list_height + 2), // +2 for top border and title
            Constraint::Length(preview_height),
            Constraint::Length(1), // Footer
        ])
        .split(area);

    // Draw theme list
    let mut lines = Vec::new();
    for (idx, theme_item) in app.themes.iter().enumerate() {
        let is_selected = idx == app.theme_popup.selected_index;
        let is_active = idx == app.active_theme_index;
        let marker = if is_active { "●" } else { "○" };

        let line_style = if is_selected {
            Style::default()
                .fg(app.theme_selection_fg())
                .bg(app.theme_selection_bg())
        } else {
            Style::default().fg(app.theme_text_color())
        };

        let name_padded = format!(
            " {} {:width$}",
            marker,
            theme_item.name,
            width = max_name_len
        );
        lines.push(Line::from(vec![Span::styled(name_padded, line_style)]));
    }

    let block = Block::default()
        .title(" Themes ")
        .title_alignment(Alignment::Center)
        .borders(Borders::TOP | Borders::LEFT | Borders::RIGHT)
        .border_style(Style::default().fg(accent))
        .style(Style::default().bg(theme.surface));

    let content = Paragraph::new(lines)
        .block(block)
        .alignment(Alignment::Left);
    frame.render_widget(content, chunks[0]);

    // Draw mini app preview for the selected theme
    let selected_theme = &app.themes[app.theme_popup.selected_index];
    draw_mini_preview(frame, chunks[1], selected_theme, accent, theme);

    // Footer
    let footer = Paragraph::new(Line::from(vec![Span::styled(
        " Enter apply • Esc close ",
        Style::default().fg(app.theme_muted_color()),
    )]))
    .alignment(Alignment::Center)
    .style(Style::default().bg(theme.surface));

    // Draw side borders for footer area
    let footer_block = Block::default()
        .borders(Borders::LEFT | Borders::RIGHT | Borders::BOTTOM)
        .border_style(Style::default().fg(accent))
        .style(Style::default().bg(theme.surface));
    frame.render_widget(footer_block, chunks[2]);
    let footer_inner = Rect {
        x: chunks[2].x + 1,
        y: chunks[2].y,
        width: chunks[2].width.saturating_sub(2),
        height: 1,
    };
    frame.render_widget(footer, footer_inner);
}

/// Draw a mini preview of what the app looks like with the given theme
fn draw_mini_preview(
    frame: &mut Frame,
    area: Rect,
    preview_theme: &Theme,
    current_accent: Color,
    current_theme: &Theme,
) {
    // Draw outer container with current theme's border (continues from theme list)
    let outer_container = Block::default()
        .borders(Borders::LEFT | Borders::RIGHT)
        .border_style(Style::default().fg(current_accent))
        .style(Style::default().bg(current_theme.surface));
    let outer_inner = outer_container.inner(area);
    frame.render_widget(outer_container, area);

    // Add padding around the preview pane
    let padded_area = Rect {
        x: outer_inner.x + 1,
        y: outer_inner.y,
        width: outer_inner.width.saturating_sub(2),
        height: outer_inner.height.saturating_sub(1),
    };

    // Draw the preview pane with its own border using the preview theme's accent
    let preview_block = Block::default()
        .title(" Preview ")
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(preview_theme.muted))
        .style(Style::default().bg(preview_theme.surface));
    let inner = preview_block.inner(padded_area);
    frame.render_widget(preview_block, padded_area);

    // The preview area uses the selected theme's colors
    let bg_style = Style::default().bg(preview_theme.surface);

    // Split preview into mini panels
    let preview_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Header bar
            Constraint::Length(1), // URL bar
            Constraint::Min(1),    // Body area
        ])
        .split(inner);

    // Mini header bar
    let header = Line::from(vec![
        Span::styled(
            "─",
            Style::default()
                .fg(preview_theme.muted)
                .bg(preview_theme.surface),
        ),
        Span::styled(
            " restui ",
            Style::default()
                .fg(preview_theme.text)
                .bg(preview_theme.surface)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "─".repeat((inner.width as usize).saturating_sub(9)),
            Style::default()
                .fg(preview_theme.muted)
                .bg(preview_theme.surface),
        ),
    ]);
    frame.render_widget(
        Paragraph::new(header).style(bg_style),
        preview_chunks[0],
    );

    // Mini URL bar with method badge
    let url_line = Line::from(vec![
        Span::styled(
            " ",
            Style::default()
        ),
        Span::styled(
            " GET ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            " https://api.example.com/data",
            Style::default()
                .fg(preview_theme.text)
                .bg(preview_theme.surface),
        ),
    ]);
    frame.render_widget(
        Paragraph::new(url_line).style(bg_style),
        preview_chunks[1],
    );

    // Mini response preview showing JSON with syntax highlighting
    let body_lines = vec![
        Line::from(vec![
            Span::styled("{", Style::default().fg(Color::White).bg(preview_theme.surface)),
        ]),
        Line::from(vec![
            Span::styled(
                "  \"status\"",
                Style::default().fg(Color::Cyan).bg(preview_theme.surface),
            ),
            Span::styled(
                ": ",
                Style::default().fg(Color::White).bg(preview_theme.surface),
            ),
            Span::styled(
                "\"ok\"",
                Style::default().fg(Color::Green).bg(preview_theme.surface),
            ),
            Span::styled(
                ",",
                Style::default().fg(Color::White).bg(preview_theme.surface),
            ),
        ]),
        Line::from(vec![
            Span::styled(
                "  \"count\"",
                Style::default().fg(Color::Cyan).bg(preview_theme.surface),
            ),
            Span::styled(
                ": ",
                Style::default().fg(Color::White).bg(preview_theme.surface),
            ),
            Span::styled(
                "42",
                Style::default().fg(Color::Yellow).bg(preview_theme.surface),
            ),
            Span::styled(
                ",",
                Style::default().fg(Color::White).bg(preview_theme.surface),
            ),
        ]),
        Line::from(vec![
            Span::styled(
                "  \"accent\"",
                Style::default().fg(Color::Cyan).bg(preview_theme.surface),
            ),
            Span::styled(
                ": ",
                Style::default().fg(Color::White).bg(preview_theme.surface),
            ),
            Span::styled(
                "████",
                Style::default()
                    .fg(preview_theme.accent)
                    .bg(preview_theme.surface),
            ),
        ]),
        Line::from(vec![
            Span::styled("}", Style::default().fg(Color::White).bg(preview_theme.surface)),
        ]),
    ];

    let body_block = Block::default()
        .borders(Borders::TOP)
        .border_style(Style::default().fg(preview_theme.muted))
        .style(bg_style);

    frame.render_widget(
        Paragraph::new(body_lines).block(body_block),
        preview_chunks[2],
    );
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let horizontal = Layout::horizontal([Constraint::Length(width)]).flex(Flex::Center);
    let vertical = Layout::vertical([Constraint::Length(height)]).flex(Flex::Center);

    let [area] = vertical.areas(area);
    let [area] = horizontal.areas(area);
    area
}
