use ratatui::{
    style::{Color, Style},
    text::Span,
};

pub fn text_with_cursor<'a>(
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
