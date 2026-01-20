use crate::app::App;
use ratatui::{
    layout::{Alignment, Constraint, Flex, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

pub fn draw_theme_popup(frame: &mut Frame, app: &App) {
    let accent = app.accent_color();
    let theme = app.theme();

    let max_name_len = app.themes.iter().map(|t| t.name.len()).max().unwrap_or(8);

    let popup_width = (max_name_len + 12).min(50).max(24) as u16;
    let popup_height = (app.themes.len() + 4).min(20).max(7) as u16;
    let area = centered_rect(popup_width, popup_height, frame.area());
    frame.render_widget(Clear, area);

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

        lines.push(Line::from(vec![Span::styled(
            format!(" {} {}", marker, theme_item.name),
            line_style,
        )]));
    }

    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "No themes available",
            Style::default().fg(app.theme_muted_color()),
        )));
    }

    let block = Block::default()
        .title(" Themes ")
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(accent))
        .style(Style::default().bg(theme.surface));

    let content = Paragraph::new(lines)
        .block(block)
        .alignment(Alignment::Left);
    frame.render_widget(content, area);

    let footer_area = Rect {
        x: area.x,
        y: area.y + area.height - 1,
        width: area.width,
        height: 1,
    };
    let footer = Paragraph::new(Line::from(vec![Span::styled(
        " Enter apply • Esc close ",
        Style::default().fg(app.theme_muted_color()),
    )]))
    .alignment(Alignment::Center);
    frame.render_widget(footer, footer_area);
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let horizontal = Layout::horizontal([Constraint::Length(width)]).flex(Flex::Center);
    let vertical = Layout::vertical([Constraint::Length(height)]).flex(Flex::Center);

    let [area] = vertical.areas(area);
    let [area] = horizontal.areas(area);
    area
}
