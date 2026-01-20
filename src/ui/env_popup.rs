use crate::app::{App, EditingField, EnvPopupSection, InputMode};
use crate::storage::KeyValue;
use ratatui::{
    layout::{Alignment, Constraint, Flex, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

pub fn draw_env_popup(frame: &mut Frame, app: &mut App) {
    let accent = app.accent_color();
    let active_name = app.environments.active_name();

    let sections = [
        EnvSection {
            title: "Shared".to_string(),
            placeholder: "No shared variables. Press 'a' or Enter to add.",
            items: &app.env_popup_shared,
            section: EnvPopupSection::Shared,
        },
        EnvSection {
            title: format!("Env: {}", active_name),
            placeholder: "No env variables. Press 'a' or Enter to add.",
            items: &app.env_popup_active,
            section: EnvPopupSection::Active,
        },
    ];

    let mut max_key_len = 0usize;
    let mut max_val_len = 0usize;
    for section in &sections {
        for item in section.items {
            max_key_len = max_key_len.max(item.key.len());
            max_val_len = max_val_len.max(item.value.len());
        }
    }

    let mut lines: Vec<Line> = Vec::new();
    let key_width = max_key_len.max(8);

    let max_width = frame.area().width.saturating_sub(4).min(110).max(40) as usize;
    let popup_width = (key_width + max_val_len + 12).min(max_width).max(40) as u16;
    let content_width = popup_width.saturating_sub(2) as usize;

    for (idx, section) in sections.iter().enumerate() {
        if idx > 0 {
            lines.push(Line::from(""));
        }

        lines.push(Line::from(Span::styled(
            truncate_with_ellipsis(&format!("-- {} --", section.title), content_width),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )));

        if section.items.is_empty() {
            let is_selected = app.input_mode == InputMode::Normal
                && app.env_popup_selected_section == section.section;
            let prefix = if is_selected { "> " } else { "  " };
            lines.push(Line::from(vec![
                Span::styled(prefix, Style::default().fg(accent)),
                Span::styled(
                    truncate_with_ellipsis(section.placeholder, content_width.saturating_sub(2)),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
        } else {
            for (item_index, item) in section.items.iter().enumerate() {
                let is_selected = app.input_mode == InputMode::Normal
                    && app.env_popup_selected_section == section.section
                    && app.env_popup_selected_index == item_index;

                let editing_key = match section.section {
                    EnvPopupSection::Shared => EditingField::EnvSharedKey(item_index),
                    EnvPopupSection::Active => EditingField::EnvActiveKey(item_index),
                };
                let editing_value = match section.section {
                    EnvPopupSection::Shared => EditingField::EnvSharedValue(item_index),
                    EnvPopupSection::Active => EditingField::EnvActiveValue(item_index),
                };
                let is_editing_key =
                    app.input_mode == InputMode::Editing && app.editing_field == Some(editing_key);
                let is_editing_value = app.input_mode == InputMode::Editing
                    && app.editing_field == Some(editing_value);

                let mut spans = Vec::new();
                spans.push(Span::styled(
                    if is_selected { "> " } else { "  " },
                    Style::default().fg(accent),
                ));
                spans.extend(text_with_cursor(
                    &item.key,
                    app.cursor_position,
                    is_editing_key,
                    "key",
                    Style::default().fg(accent).add_modifier(Modifier::BOLD),
                ));
                spans.push(Span::raw(" = "));
                spans.extend(text_with_cursor(
                    &item.value,
                    app.cursor_position,
                    is_editing_value,
                    "value",
                    Style::default().fg(Color::White),
                ));

                lines.push(Line::from(spans));
            }
        }
    }

    let popup_height = (lines.len() + 6).min(40).max(10) as u16;
    let visible_height = popup_height.saturating_sub(3) as usize;
    let max_scroll = lines.len().saturating_sub(visible_height) as u16;
    let scroll = app.env_popup_scroll.min(max_scroll);
    app.env_popup_scroll = scroll;
    app.env_popup_visible_height = visible_height;

    let area = centered_rect(popup_width, popup_height, frame.area());
    frame.render_widget(Clear, area);

    let env_block = Block::default()
        .title(" Env Variables ")
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
    let footer_text = " Enter edit • Tab next • a add • x delete • Esc close ";
    let footer = Paragraph::new(Line::from(vec![Span::styled(
        truncate_with_ellipsis(footer_text, content_width),
        Style::default().fg(Color::DarkGray),
    )]))
    .alignment(Alignment::Center);
    frame.render_widget(footer, footer_area);
}

struct EnvSection<'a> {
    title: String,
    placeholder: &'a str,
    items: &'a Vec<KeyValue>,
    section: EnvPopupSection,
}

fn text_with_cursor<'a>(
    text: &str,
    cursor_pos: usize,
    is_editing: bool,
    placeholder: &str,
    base_style: Style,
) -> Vec<Span<'a>> {
    if is_editing {
        let pos = cursor_pos.min(text.len());
        if text.is_empty() {
            vec![Span::styled(
                " ",
                Style::default().bg(Color::White).fg(Color::Black),
            )]
        } else if pos >= text.len() {
            vec![
                Span::styled(text.to_string(), Style::default().bg(Color::DarkGray)),
                Span::styled(" ", Style::default().bg(Color::White).fg(Color::Black)),
            ]
        } else {
            let (before, rest) = text.split_at(pos);
            let mut chars = rest.chars();
            let cursor_char = chars.next().unwrap_or(' ');
            let after: String = chars.collect();
            vec![
                Span::styled(before.to_string(), Style::default().bg(Color::DarkGray)),
                Span::styled(
                    cursor_char.to_string(),
                    Style::default().bg(Color::White).fg(Color::Black),
                ),
                Span::styled(after, Style::default().bg(Color::DarkGray)),
            ]
        }
    } else if text.is_empty() {
        vec![Span::styled(
            placeholder.to_string(),
            Style::default().fg(Color::DarkGray),
        )]
    } else {
        vec![Span::styled(text.to_string(), base_style)]
    }
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
