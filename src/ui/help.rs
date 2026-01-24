use crate::app::App;
use ratatui::{
    layout::{Alignment, Constraint, Flex, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

/// Draw the help popup centered on screen
pub fn draw_help(frame: &mut Frame, app: &App) {
    let help_content = app.get_help_content();
    let accent = app.theme_accent_color(); // Use theme's accent for help popup

    // Calculate popup size
    let max_key_len = help_content
        .iter()
        .map(|(k, _)| k.len())
        .max()
        .unwrap_or(10);
    let max_desc_len = help_content
        .iter()
        .map(|(_, d)| d.len())
        .max()
        .unwrap_or(20);

    let popup_width = (max_key_len + max_desc_len + 6).min(60) as u16;
    let popup_height = (help_content.len() + 4).min(30) as u16;

    // Center the popup
    let area = centered_rect(popup_width, popup_height, frame.area());

    // Build help lines
    let lines: Vec<Line> = help_content
        .iter()
        .map(|(key, desc)| {
            if key.is_empty() {
                // Section header
                Line::from(Span::styled(
                    *desc,
                    Style::default().fg(accent).add_modifier(Modifier::BOLD),
                ))
            } else {
                // Key-value pair
                Line::from(vec![
                    Span::styled(
                        format!("{:>12}", key),
                        Style::default().fg(accent).add_modifier(Modifier::BOLD),
                    ),
                    Span::raw("  "),
                    Span::styled(*desc, Style::default().fg(app.theme_text_color())),
                ])
            }
        })
        .collect();

    // Clear the area behind the popup
    frame.render_widget(Clear, area);

    // Draw the popup
    let help_block = Block::default()
        .title(" Help ")
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(accent))
        .style(Style::default().bg(app.theme_surface_color()));

    let help_text = Paragraph::new(lines)
        .block(help_block)
        .wrap(Wrap { trim: false })
        .alignment(Alignment::Left);

    frame.render_widget(help_text, area);

    // Footer hint
    let footer_area = Rect {
        x: area.x,
        y: area.y + area.height - 1,
        width: area.width,
        height: 1,
    };
    let footer = Paragraph::new(Line::from(vec![Span::styled(
        " Press any key to close ",
        Style::default().fg(app.theme_muted_color()),
    )]))
    .alignment(Alignment::Center);
    frame.render_widget(footer, footer_area);
}

/// Helper function to create a centered rect
fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let horizontal = Layout::horizontal([Constraint::Length(width)]).flex(Flex::Center);
    let vertical = Layout::vertical([Constraint::Length(height)]).flex(Flex::Center);

    let [area] = vertical.areas(area);
    let [area] = horizontal.areas(area);
    area
}
