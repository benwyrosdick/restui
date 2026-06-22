use crate::app::{App, DialogType, ItemType};
use ratatui::{
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use super::widgets::text_with_cursor_and_selection;

/// Draw the dialog popup if active
pub fn draw_dialog(frame: &mut Frame, app: &mut App) {
    let Some(dialog_type) = &app.dialog.dialog_type.clone() else {
        return;
    };

    let accent = app.accent_color();

    match dialog_type {
        DialogType::ConfirmDelete {
            item_type,
            item_name,
            ..
        } => {
            draw_confirm_delete_dialog(frame, app, item_type, item_name, accent);
            app.layout_areas.dialog_input_area = None;
        }
        DialogType::ConfirmOverwrite { path } => {
            draw_confirm_overwrite_dialog(frame, app, path, accent);
            app.layout_areas.dialog_input_area = None;
        }
        _ => {
            draw_input_dialog(frame, app, dialog_type);
        }
    }
}

fn draw_input_dialog(frame: &mut Frame, app: &mut App, dialog_type: &DialogType) {
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
        DialogType::SaveResponseAs => "Save Response As",
        DialogType::ImportPostman => "Import from Postman",
        DialogType::ConfirmDelete { .. } | DialogType::ConfirmOverwrite { .. } => unreachable!(),
    };

    let prompt_label = match dialog_type {
        DialogType::SaveResponseAs => "Path: ",
        DialogType::ImportPostman => "File path: ",
        _ => "Name: ",
    };
    let prompt_label_len = prompt_label.chars().count() as u16;

    // Path-input dialogs show a live directory preview and support Tab-completion.
    let is_path = matches!(
        dialog_type,
        DialogType::SaveResponseAs | DialogType::ImportPostman
    );
    let preview_lines: Vec<Line> = if is_path {
        build_path_preview(app, accent)
    } else {
        Vec::new()
    };

    let popup_width = 50;
    let popup_height = 7 + preview_lines.len() as u16;
    let area = super::layout::centered_rect(popup_width, popup_height, frame.area());

    // Clear area behind popup
    frame.render_widget(Clear, area);

    let block = Block::default()
        .title(format!(" {} ", title))
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(accent))
        .style(Style::default().bg(app.theme_surface_color()));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Compute selection range
    let selection = app.dialog.selection_anchor.map(|anchor| {
        let cursor = app.dialog.cursor_position;
        if anchor < cursor {
            (anchor, cursor)
        } else {
            (cursor, anchor)
        }
    });

    // Input label and field with proper cursor
    let base_style = Style::default().fg(app.theme_text_color());
    let mut spans = vec![Span::styled(prompt_label, Style::default().fg(accent))];
    let text_spans = text_with_cursor_and_selection(
        &app.dialog.input_buffer,
        app.dialog.cursor_position,
        true, // always editing in dialog
        "",
        base_style,
        selection,
    );
    spans.extend(text_spans);

    let prompt = Paragraph::new(Line::from(spans));

    let prompt_area = Rect {
        x: inner.x + 1,
        y: inner.y + 1,
        width: inner.width.saturating_sub(2),
        height: 1,
    };
    frame.render_widget(prompt, prompt_area);

    // Store the input area for mouse handling (text starts after prompt label)
    let text_start_x = prompt_area.x + prompt_label_len;
    let text_width = prompt_area.width.saturating_sub(prompt_label_len);
    app.layout_areas.dialog_input_area = Some((text_start_x, prompt_area.y, text_width));

    // Directory preview (path dialogs only)
    if !preview_lines.is_empty() {
        let preview_area = Rect {
            x: inner.x + 1,
            y: prompt_area.y + 1,
            width: inner.width.saturating_sub(2),
            height: preview_lines.len() as u16,
        };
        frame.render_widget(Paragraph::new(preview_lines), preview_area);
    }

    // Footer hints
    let mut footer_spans = Vec::new();
    if is_path {
        footer_spans.push(Span::styled("Tab", Style::default().fg(accent)));
        footer_spans.push(Span::raw(": complete  "));
    }
    footer_spans.extend([
        Span::styled("Enter", Style::default().fg(accent)),
        Span::raw(": confirm  "),
        Span::styled("Esc", Style::default().fg(accent)),
        Span::raw(": cancel"),
    ]);
    let footer = Paragraph::new(Line::from(footer_spans)).alignment(Alignment::Center);

    let footer_area = Rect {
        x: inner.x,
        y: inner.y + inner.height - 2,
        width: inner.width,
        height: 1,
    };
    frame.render_widget(footer, footer_area);
}

/// Build the directory-preview lines for a path-input dialog (capped, with a
/// "+N more" indicator when the listing is truncated).
fn build_path_preview(app: &App, accent: Color) -> Vec<Line<'static>> {
    const MAX_ROWS: usize = 8;

    let candidates = app.path_completion_candidates();
    if candidates.is_empty() {
        return Vec::new();
    }

    let muted = app.theme_muted_color();
    let total = candidates.len();
    let visible = if total > MAX_ROWS {
        MAX_ROWS - 1
    } else {
        total
    };

    let mut lines: Vec<Line<'static>> = candidates
        .iter()
        .take(visible)
        .map(|(name, is_dir)| {
            let (text, color) = if *is_dir {
                (format!("  {}/", name), accent)
            } else {
                (format!("  {}", name), muted)
            };
            Line::from(Span::styled(text, Style::default().fg(color)))
        })
        .collect();

    if total > MAX_ROWS {
        lines.push(Line::from(Span::styled(
            format!("  … (+{} more)", total - visible),
            Style::default().fg(muted),
        )));
    }

    lines
}

fn draw_confirm_delete_dialog(
    frame: &mut Frame,
    app: &App,
    item_type: &ItemType,
    item_name: &str,
    accent: Color,
) {
    let type_str = match item_type {
        ItemType::Collection => "collection",
        ItemType::Folder => "folder (and all contents)",
        ItemType::Request => "request",
    };

    let popup_width = 50;
    let popup_height = 9;
    let area = super::layout::centered_rect(popup_width, popup_height, frame.area());

    frame.render_widget(Clear, area);

    let block = Block::default()
        .title(" Confirm Delete ")
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Red))
        .style(Style::default().bg(app.theme_surface_color()));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Warning message
    let message = Paragraph::new(vec![
        Line::from(""),
        Line::from(Span::styled(
            format!("Delete this {}?", type_str),
            Style::default().fg(app.theme_text_color()),
        )),
        Line::from(""),
        Line::from(Span::styled(
            format!("\"{}\"", item_name),
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
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

fn draw_confirm_overwrite_dialog(
    frame: &mut Frame,
    app: &App,
    path: &std::path::Path,
    accent: Color,
) {
    let filename = path.file_name().and_then(|s| s.to_str()).unwrap_or("file");

    let popup_width = 55;
    let popup_height = 10;
    let area = super::layout::centered_rect(popup_width, popup_height, frame.area());

    frame.render_widget(Clear, area);

    let block = Block::default()
        .title(" File Exists ")
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .style(Style::default().bg(app.theme_surface_color()));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Warning message
    let message = Paragraph::new(vec![
        Line::from(""),
        Line::from(Span::styled(
            format!("\"{}\"", filename),
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "File already exists. Would you like to overwrite it?",
            Style::default().fg(app.theme_text_color()),
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
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(": overwrite  "),
        Span::styled(
            "n",
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        ),
        Span::raw(": add (n)  "),
        Span::styled("Esc", Style::default().fg(app.theme_muted_color())),
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
