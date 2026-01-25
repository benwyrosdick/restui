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
    text_with_cursor_and_selection(text, cursor_pos, is_editing, placeholder, base_style, None)
}

pub fn text_with_cursor_and_selection<'a>(
    text: &str,
    cursor_pos: usize,
    is_editing: bool,
    placeholder: &str,
    base_style: Style,
    selection: Option<(usize, usize)>,
) -> Vec<Span<'a>> {
    if is_editing {
        let char_count = text.chars().count();
        let pos = cursor_pos.min(char_count);

        // Selection highlighting style
        let selection_style = Style::default().bg(Color::Blue).fg(Color::White);
        let editing_style = Style::default().bg(Color::DarkGray);
        let cursor_style = Style::default().bg(Color::White).fg(Color::Black);

        if text.is_empty() {
            return vec![Span::styled(" ", cursor_style)];
        }

        // Check if we have a selection
        if let Some((sel_start, sel_end)) = selection {
            // Clamp selection bounds to valid range
            let sel_start = sel_start.min(char_count);
            let sel_end = sel_end.min(char_count);

            if sel_start != sel_end {
                // Build spans with selection highlighting
                let mut spans = Vec::new();
                let chars: Vec<char> = text.chars().collect();

                // Before selection
                if sel_start > 0 {
                    let before: String = chars[..sel_start].iter().collect();
                    spans.push(Span::styled(before, editing_style));
                }

                // Selected text
                let selected: String = chars[sel_start..sel_end].iter().collect();
                spans.push(Span::styled(selected, selection_style));

                // After selection
                if sel_end < chars.len() {
                    let after: String = chars[sel_end..].iter().collect();
                    spans.push(Span::styled(after, editing_style));
                }

                // Cursor at end if past text
                if pos >= char_count {
                    spans.push(Span::styled(" ", cursor_style));
                }

                return spans;
            }
        }

        // No selection - show regular cursor
        if pos >= char_count {
            vec![
                Span::styled(text.to_string(), editing_style),
                Span::styled(" ", cursor_style),
            ]
        } else {
            // Convert char position to byte position for split
            let byte_pos = text
                .char_indices()
                .nth(pos)
                .map(|(i, _)| i)
                .unwrap_or(text.len());
            let (before, rest) = text.split_at(byte_pos);
            let mut chars = rest.chars();
            let cursor_char = chars.next().unwrap_or(' ');
            let after: String = chars.collect();
            vec![
                Span::styled(before.to_string(), editing_style),
                Span::styled(cursor_char.to_string(), cursor_style),
                Span::styled(after, editing_style),
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
