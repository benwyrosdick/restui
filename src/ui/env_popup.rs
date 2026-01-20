use crate::app::App;
use ratatui::{
    layout::{Alignment, Constraint, Flex, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

pub fn draw_env_popup(frame: &mut Frame, app: &App) {
    let accent = app.accent_color();
    let active_name = app.environments.active_name();

    let mut sections: Vec<(&str, Vec<(String, String)>)> = Vec::new();
    if !app.environments.shared.is_empty() {
        let mut items: Vec<(String, String)> = app
            .environments
            .shared
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        items.sort_by(|a, b| a.0.cmp(&b.0));
        sections.push(("Shared", items));
    }
    if let Some(env) = app.environments.active() {
        if !env.variables.is_empty() {
            let mut items: Vec<(String, String)> = env
                .variables
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            items.sort_by(|a, b| a.0.cmp(&b.0));
            sections.push(("Active", items));
        }
    }

    let mut max_key_len = 0usize;
    let mut max_val_len = 0usize;
    for (_, items) in &sections {
        for (key, value) in items {
            max_key_len = max_key_len.max(key.len());
            max_val_len = max_val_len.max(value.len());
        }
    }
    if sections.is_empty() {
        max_val_len = max_val_len.max("No variables set".len());
    }

    let mut lines: Vec<Line> = Vec::new();
    let key_width = max_key_len.max(8);

    let max_width = frame
        .area()
        .width
        .saturating_sub(4)
        .min(90)
        .max(30) as usize;
    let popup_width = (key_width + max_val_len + 6)
        .min(max_width)
        .max(30) as u16;
    let content_width = popup_width.saturating_sub(2) as usize;
    let value_width = content_width.saturating_sub(key_width + 2);

    if sections.is_empty() {
        lines.push(Line::from(Span::styled(
            truncate_with_ellipsis("No variables set", content_width),
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        for (idx, (title, items)) in sections.iter().enumerate() {
            if idx > 0 {
                lines.push(Line::from(""));
            }
            lines.push(Line::from(Span::styled(
                truncate_with_ellipsis(&format!("-- {} --", title), content_width),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )));
            for (key, value) in items {
                lines.push(Line::from(vec![
                    Span::styled(
                        format!("{:>width$}", key, width = key_width),
                        Style::default().fg(accent).add_modifier(Modifier::BOLD),
                    ),
                    Span::raw("  "),
                    Span::styled(
                        truncate_with_ellipsis(value, value_width),
                        Style::default().fg(Color::White),
                    ),
                ]));
            }
        }
    }

    let popup_height = (lines.len() + 4).min(30).max(7) as u16;
    let visible_height = popup_height.saturating_sub(3) as usize;
    let max_scroll = lines.len().saturating_sub(visible_height) as u16;
    let scroll = app.env_popup_scroll.min(max_scroll);

    let area = centered_rect(popup_width, popup_height, frame.area());
    frame.render_widget(Clear, area);

    let env_block = Block::default()
        .title(format!(" Env: {} ", active_name))
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(accent))
        .style(Style::default().bg(Color::Black));

    let env_text = Paragraph::new(lines)
        .block(env_block)
        .scroll((scroll, 0))
        .alignment(Alignment::Left);
    frame.render_widget(env_text, area);

    let footer_area = Rect {
        x: area.x,
        y: area.y + area.height - 1,
        width: area.width,
        height: 1,
    };
    let footer = Paragraph::new(Line::from(vec![Span::styled(
        " Press any key to close ",
        Style::default().fg(Color::DarkGray),
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

fn truncate_with_ellipsis(input: &str, max_width: usize) -> String {
    if max_width == 0 {
        return String::new();
    }
    let input_len = input.chars().count();
    if input_len <= max_width {
        return input.to_string();
    }
    if max_width <= 3 {
        return ".".repeat(max_width);
    }
    let take_len = max_width.saturating_sub(3);
    let mut result = String::new();
    for ch in input.chars().take(take_len) {
        result.push(ch);
    }
    result.push_str("...");
    result
}
