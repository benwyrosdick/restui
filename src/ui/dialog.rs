use crate::app::{App, DialogType, ItemType};
use ratatui::{
    layout::{Alignment, Constraint, Flex, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

/// Draw the dialog popup if active
pub fn draw_dialog(frame: &mut Frame, app: &App) {
    let Some(dialog_type) = &app.dialog.dialog_type else {
        return;
    };

    let accent = app.accent_color();

    match dialog_type {
        DialogType::ConfirmDelete {
            item_type,
            item_name,
            ..
        } => {
            draw_confirm_dialog(frame, item_type, item_name, accent);
        }
        _ => {
            draw_input_dialog(frame, app, dialog_type);
        }
    }
}

fn draw_input_dialog(frame: &mut Frame, app: &App, dialog_type: &DialogType) {
    let accent = app.accent_color();
    let title = match dialog_type {
        DialogType::CreateCollection => "New Collection",
        DialogType::CreateFolder { .. } => "New Folder",
        DialogType::CreateRequest { .. } => "New Request",
        DialogType::RenameItem { item_type, .. } => match item_type {
            ItemType::Collection => "Rename Collection",
            ItemType::Folder => "Rename Folder",
            ItemType::Request => "Rename Request",
        },
        DialogType::ConfirmDelete { .. } => unreachable!(),
    };

    let popup_width = 50;
    let popup_height = 7;
    let area = centered_rect(popup_width, popup_height, frame.area());

    // Clear area behind popup
    frame.render_widget(Clear, area);

    let block = Block::default()
        .title(format!(" {} ", title))
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(accent))
        .style(Style::default().bg(Color::Black));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Input label and field
    let cursor = if (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis()
        / 500)
        % 2
        == 0
    {
        "_"
    } else {
        " "
    };

    let prompt = Paragraph::new(Line::from(vec![
        Span::styled("Name: ", Style::default().fg(Color::Yellow)),
        Span::styled(&app.dialog.input_buffer, Style::default().fg(Color::White)),
        Span::styled(cursor, Style::default().fg(Color::Gray)),
    ]));

    let prompt_area = Rect {
        x: inner.x + 1,
        y: inner.y + 1,
        width: inner.width.saturating_sub(2),
        height: 1,
    };
    frame.render_widget(prompt, prompt_area);

    // Footer hints
    let footer = Paragraph::new(Line::from(vec![
        Span::styled("Enter", Style::default().fg(accent)),
        Span::raw(": confirm  "),
        Span::styled("Esc", Style::default().fg(accent)),
        Span::raw(": cancel"),
    ]))
    .alignment(Alignment::Center);

    let footer_area = Rect {
        x: inner.x,
        y: inner.y + inner.height - 2,
        width: inner.width,
        height: 1,
    };
    frame.render_widget(footer, footer_area);
}

fn draw_confirm_dialog(frame: &mut Frame, item_type: &ItemType, item_name: &str, accent: Color) {
    let type_str = match item_type {
        ItemType::Collection => "collection",
        ItemType::Folder => "folder (and all contents)",
        ItemType::Request => "request",
    };

    let popup_width = 50;
    let popup_height = 9;
    let area = centered_rect(popup_width, popup_height, frame.area());

    frame.render_widget(Clear, area);

    let block = Block::default()
        .title(" Confirm Delete ")
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Red))
        .style(Style::default().bg(Color::Black));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Warning message
    let message = Paragraph::new(vec![
        Line::from(""),
        Line::from(Span::styled(
            format!("Delete this {}?", type_str),
            Style::default().fg(Color::White),
        )),
        Line::from(""),
        Line::from(Span::styled(
            format!("\"{}\"", item_name),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
    ])
    .alignment(Alignment::Center);

    frame.render_widget(
        message,
        Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: 5,
        },
    );

    // Footer hints
    let footer = Paragraph::new(Line::from(vec![
        Span::styled(
            "y",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ),
        Span::raw(": delete  "),
        Span::styled("n/Esc", Style::default().fg(accent)),
        Span::raw(": cancel"),
    ]))
    .alignment(Alignment::Center);

    frame.render_widget(
        footer,
        Rect {
            x: inner.x,
            y: inner.y + inner.height - 1,
            width: inner.width,
            height: 1,
        },
    );
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let horizontal = Layout::horizontal([Constraint::Length(width)]).flex(Flex::Center);
    let vertical = Layout::vertical([Constraint::Length(height)]).flex(Flex::Center);
    let [area] = vertical.areas(area);
    let [area] = horizontal.areas(area);
    area
}
