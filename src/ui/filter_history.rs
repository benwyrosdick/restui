use crate::app::App;
use ratatui::{
    layout::{Alignment, Constraint, Flex, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

pub fn draw_filter_history(frame: &mut Frame, app: &App) {
    let accent = app.accent_color();
    let theme = app.theme();

    let max_filter_len = app
        .filter_history
        .iter()
        .map(|f| f.len())
        .max()
        .unwrap_or(10);

    let popup_width = (max_filter_len + 8).min(60).max(30) as u16;
    let popup_height = (app.filter_history.len() + 4).min(15).max(7) as u16;
    let area = centered_rect(popup_width, popup_height, frame.area());
    frame.render_widget(Clear, area);

    let mut lines = Vec::new();
    for (idx, filter) in app.filter_history.iter().enumerate() {
        let is_selected = idx == app.filter_history_selected;
        let line_style = if is_selected {
            Style::default()
                .fg(app.theme_selection_fg())
                .bg(app.theme_selection_bg())
        } else {
            Style::default().fg(app.theme_text_color())
        };

        // Truncate long filters for display
        let display_filter = if filter.len() > popup_width as usize - 6 {
            format!("{}...", &filter[..popup_width as usize - 9])
        } else {
            filter.clone()
        };

        lines.push(Line::from(vec![Span::styled(
            format!(" {} ", display_filter),
            line_style,
        )]));
    }

    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "No filter history",
            Style::default().fg(app.theme_muted_color()),
        )));
    }

    let block = Block::default()
        .title(" Filter History ")
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
        " Enter apply • d delete • Esc close ",
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
