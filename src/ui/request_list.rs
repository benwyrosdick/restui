use crate::app::{App, FocusedPanel};
use crate::storage::CollectionItem;
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{List, ListItem, Paragraph},
    Frame,
};

use super::layout::bordered_block;

pub fn draw(frame: &mut Frame, app: &App, area: Rect) {
    let focused = app.focused_panel == FocusedPanel::RequestList;
    let accent = app.accent_color();
    let title = if app.show_history {
        "History"
    } else {
        "Collections"
    };

    let block = bordered_block(title, focused, accent);
    let inner_area = block.inner(area);
    frame.render_widget(block, area);

    if app.show_history {
        draw_history(frame, app, inner_area, accent);
    } else {
        draw_collections(frame, app, inner_area, accent);
    }
}

fn draw_collections(frame: &mut Frame, app: &App, area: Rect, accent: Color) {
    let mut items: Vec<ListItem> = Vec::new();

    for (col_idx, collection) in app.collections.iter().enumerate() {
        // Collection header - selected when this collection is selected AND header is selected (usize::MAX)
        let is_header_selected =
            col_idx == app.selected_collection && app.is_collection_header_selected();
        let prefix = if collection.expanded { "▼ " } else { "▶ " };
        let style = if is_header_selected {
            Style::default()
                .fg(Color::Black)
                .bg(accent)
                .add_modifier(Modifier::BOLD)
        } else if col_idx == app.selected_collection {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };

        items.push(ListItem::new(Line::from(vec![
            Span::styled(format!("{} ", prefix), style),
            Span::styled(&collection.name, style),
        ])));

        if collection.expanded {
            for (item_idx, (depth, item)) in collection.flatten().into_iter().enumerate() {
                let is_selected = col_idx == app.selected_collection
                    && !app.is_collection_header_selected()
                    && item_idx == app.selected_item;

                let indent = "  ".repeat(depth + 1);
                let (icon, name, method_style) = match item {
                    CollectionItem::Request(req) => {
                        let method_color = match req.method {
                            crate::storage::HttpMethod::Get => Color::Green,
                            crate::storage::HttpMethod::Post => Color::Yellow,
                            crate::storage::HttpMethod::Put => Color::Blue,
                            crate::storage::HttpMethod::Delete => Color::Red,
                        };
                        (
                            format!("{} ", req.method.as_str()),
                            req.name.clone(),
                            Style::default().fg(method_color),
                        )
                    }
                    CollectionItem::Folder { name, expanded, .. } => {
                        let icon = if *expanded { "▼ " } else { "▶ " };
                        (icon.to_string(), name.clone(), Style::default().fg(accent))
                    }
                };

                let name_style = if is_selected {
                    Style::default()
                        .fg(Color::Black)
                        .bg(accent)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };

                items.push(ListItem::new(Line::from(vec![
                    Span::raw(indent),
                    Span::styled(icon, method_style),
                    Span::styled(name, name_style),
                ])));
            }
        }
    }

    if items.is_empty() {
        let placeholder = Paragraph::new("No collections. Press 'n' to create a request.")
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(placeholder, area);
    } else {
        let list = List::new(items);
        frame.render_widget(list, area);
    }
}

fn draw_history(frame: &mut Frame, app: &App, area: Rect, accent: Color) {
    let items: Vec<ListItem> = app
        .history
        .entries
        .iter()
        .enumerate()
        .map(|(i, entry)| {
            let is_selected = i == app.selected_history;

            let method_color = match entry.request.method {
                crate::storage::HttpMethod::Get => Color::Green,
                crate::storage::HttpMethod::Post => Color::Yellow,
                crate::storage::HttpMethod::Put => Color::Blue,
                crate::storage::HttpMethod::Delete => Color::Red,
            };

            let status_color = match entry.status_code {
                Some(code) if code >= 200 && code < 300 => Color::Green,
                Some(code) if code >= 400 => Color::Red,
                Some(_) => Color::Yellow,
                None => Color::Red,
            };

            let status_str = entry
                .status_code
                .map(|c| c.to_string())
                .unwrap_or_else(|| "ERR".to_string());

            let style = if is_selected {
                Style::default()
                    .bg(accent)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            // Extract path from URL
            let path = entry
                .request
                .url
                .split("://")
                .nth(1)
                .and_then(|s| s.find('/').map(|i| &s[i..]))
                .unwrap_or(&entry.request.url);

            ListItem::new(Line::from(vec![
                Span::styled(
                    format!("{:6} ", entry.request.method.as_str()),
                    Style::default().fg(method_color),
                ),
                Span::styled(
                    format!("{:4} ", status_str),
                    Style::default().fg(status_color),
                ),
                Span::styled(path.to_string(), style),
            ]))
        })
        .collect();

    if items.is_empty() {
        let placeholder =
            Paragraph::new("No history yet.").style(Style::default().fg(Color::DarkGray));
        frame.render_widget(placeholder, area);
    } else {
        let list = List::new(items);
        frame.render_widget(list, area);
    }
}
