use crate::app::{App, FocusedPanel};
use crate::storage::CollectionItem;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{List, ListItem, Paragraph},
    Frame,
};

use super::layout::bordered_block_with_number;
use super::widgets::text_with_cursor;

pub fn draw(frame: &mut Frame, app: &App, area: Rect) {
    let focused = app.focused_panel == FocusedPanel::RequestList;
    let accent = app.accent_color();
    let title = if app.show_history {
        "History"
    } else {
        "Collections"
    };

    let block = bordered_block_with_number(
        title,
        focused,
        accent,
        app.theme_surface_color(),
        app.theme_muted_color(),
        Some(1),
    );
    let inner_area = block.inner(area);
    frame.render_widget(block, area);

    // Show search bar if search is active or has filter
    let show_search_bar = app.request_list_search_active || app.has_request_list_filter();

    let (list_area, search_area) = if show_search_bar {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(1)])
            .split(inner_area);
        (chunks[0], Some(chunks[1]))
    } else {
        (inner_area, None)
    };

    if app.show_history {
        draw_history(frame, app, list_area, accent);
    } else {
        draw_collections(frame, app, list_area, accent);
    }

    // Draw search bar
    if let Some(search_area) = search_area {
        draw_search_bar(frame, app, search_area, accent);
    }
}

fn draw_search_bar(frame: &mut Frame, app: &App, area: Rect, accent: Color) {
    let search_style = if app.request_list_search_active {
        Style::default().fg(accent)
    } else {
        Style::default().fg(app.theme_muted_color())
    };

    if app.request_list_search_active {
        // Show cursor when actively searching
        let mut spans = vec![Span::styled("/ ", search_style)];
        let cursor_spans = text_with_cursor(
            &app.request_list_search_query,
            app.request_list_search_cursor,
            true,
            "",
            search_style,
        );
        spans.extend(cursor_spans);
        let search_widget = Paragraph::new(Line::from(spans));
        frame.render_widget(search_widget, area);
    } else {
        // Just show the filter text without cursor
        let text = format!("/ {}", app.request_list_search_query);
        let search_widget = Paragraph::new(text).style(search_style);
        frame.render_widget(search_widget, area);
    }
}

fn draw_collections(frame: &mut Frame, app: &App, area: Rect, accent: Color) {
    let mut items: Vec<ListItem> = Vec::new();
    let has_filter = app.has_request_list_filter();

    if has_filter {
        // Filtered view: show flat list of matching requests only
        let filtered_items = app.filtered_collection_items();

        for (display_idx, &(col_idx, item_idx)) in filtered_items.iter().enumerate() {
            let is_selected = display_idx == app.request_list_filtered_selection;

            if let Some(collection) = app.collections.get(col_idx) {
                let flattened = collection.flatten();
                if let Some((_, item)) = flattened.get(item_idx) {
                    if let CollectionItem::Request(req) = item {
                        let method_color = match req.method {
                            crate::storage::HttpMethod::Get => Color::Green,
                            crate::storage::HttpMethod::Post => Color::Yellow,
                            crate::storage::HttpMethod::Put => Color::Blue,
                            crate::storage::HttpMethod::Patch => Color::Magenta,
                            crate::storage::HttpMethod::Delete => Color::Red,
                        };

                        let name_style = if is_selected {
                            Style::default()
                                .fg(app.theme_selection_fg())
                                .bg(app.theme_selection_bg())
                                .add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(app.theme_text_color())
                        };

                        // Highlight matching text
                        let name_spans = highlight_matches(
                            &req.name,
                            &app.request_list_search_query,
                            name_style,
                            accent,
                        );

                        let mut line_spans = vec![Span::styled(
                            format!("{} ", req.method.as_str()),
                            Style::default().fg(method_color),
                        )];
                        line_spans.extend(name_spans);

                        // Add collection name as context (dimmed)
                        line_spans.push(Span::styled(
                            format!("  [{}]", collection.name),
                            Style::default().fg(app.theme_muted_color()),
                        ));

                        items.push(ListItem::new(Line::from(line_spans)));
                    }
                }
            }
        }
    } else {
        // Normal tree view
        for (col_idx, collection) in app.collections.iter().enumerate() {
            let flattened = collection.flatten();

            // Collection header
            let is_header_selected =
                col_idx == app.selected_collection && app.is_collection_header_selected();
            let prefix = if collection.expanded { "▼ " } else { "▶ " };
            let style = if is_header_selected {
                Style::default()
                    .fg(app.theme_selection_fg())
                    .bg(app.theme_selection_bg())
                    .add_modifier(Modifier::BOLD)
            } else if col_idx == app.selected_collection {
                Style::default().fg(accent).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(app.theme_text_color())
            };

            items.push(ListItem::new(Line::from(vec![
                Span::styled(format!("{} ", prefix), style),
                Span::styled(&collection.name, style),
            ])));

            if collection.expanded {
                for (item_idx, (depth, item)) in flattened.iter().enumerate() {
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
                                crate::storage::HttpMethod::Patch => Color::Magenta,
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
                            .fg(app.theme_selection_fg())
                            .bg(app.theme_selection_bg())
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(app.theme_text_color())
                    };

                    items.push(ListItem::new(Line::from(vec![
                        Span::raw(indent),
                        Span::styled(icon, method_style),
                        Span::styled(name, name_style),
                    ])));
                }
            }
        }
    }

    if items.is_empty() {
        let message = if has_filter {
            "No matches found."
        } else {
            "No collections. Press 'n' to create a request."
        };
        let placeholder =
            Paragraph::new(message).style(Style::default().fg(app.theme_muted_color()));
        frame.render_widget(placeholder, area);
    } else {
        let list = List::new(items);
        frame.render_widget(list, area);
    }
}

fn draw_history(frame: &mut Frame, app: &App, area: Rect, _accent: Color) {
    let has_filter = app.has_request_list_filter();
    let accent = app.accent_color();

    // Build list of visible entries with their display index
    let visible_entries: Vec<(usize, usize, &_)> = app
        .history
        .entries
        .iter()
        .enumerate()
        .filter(|(_, entry)| {
            if !has_filter {
                return true;
            }
            // Match against URL, method, or path
            let path = entry
                .request
                .url
                .split("://")
                .nth(1)
                .and_then(|s| s.find('/').map(|i| &s[i..]))
                .unwrap_or(&entry.request.url);
            app.matches_request_list_filter(path)
                || app.matches_request_list_filter(entry.request.method.as_str())
                || app.matches_request_list_filter(&entry.request.url)
        })
        .enumerate()
        .map(|(display_idx, (original_idx, entry))| (display_idx, original_idx, entry))
        .collect();

    let items: Vec<ListItem> = visible_entries
        .iter()
        .map(|(display_idx, original_idx, entry)| {
            // Use display_idx for selection when filtering, original_idx otherwise
            let is_selected = if has_filter {
                *display_idx == app.selected_history
            } else {
                *original_idx == app.selected_history
            };

            let method_color = match entry.request.method {
                crate::storage::HttpMethod::Get => Color::Green,
                crate::storage::HttpMethod::Post => Color::Yellow,
                crate::storage::HttpMethod::Put => Color::Blue,
                crate::storage::HttpMethod::Patch => Color::Magenta,
                crate::storage::HttpMethod::Delete => Color::Red,
            };

            let status_color = match entry.status_code {
                Some(code) if (200..300).contains(&code) => Color::Green,
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
                    .bg(app.theme_selection_bg())
                    .fg(app.theme_selection_fg())
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(app.theme_text_color())
            };

            // Extract path from URL
            let path = entry
                .request
                .url
                .split("://")
                .nth(1)
                .and_then(|s| s.find('/').map(|i| &s[i..]))
                .unwrap_or(&entry.request.url);

            // Highlight matching text in path
            let path_spans = if has_filter {
                highlight_matches(path, &app.request_list_search_query, style, accent)
            } else {
                vec![Span::styled(path.to_string(), style)]
            };

            let mut spans = vec![
                Span::styled(
                    format!("{:6} ", entry.request.method.as_str()),
                    Style::default().fg(method_color),
                ),
                Span::styled(
                    format!("{:4} ", status_str),
                    Style::default().fg(status_color),
                ),
            ];
            spans.extend(path_spans);

            ListItem::new(Line::from(spans))
        })
        .collect();

    if items.is_empty() {
        let message = if has_filter {
            "No matches found."
        } else {
            "No history yet."
        };
        let placeholder =
            Paragraph::new(message).style(Style::default().fg(app.theme_muted_color()));
        frame.render_widget(placeholder, area);
    } else {
        let list = List::new(items);
        frame.render_widget(list, area);
    }
}

/// Highlight matching parts of text with accent color
fn highlight_matches(
    text: &str,
    query: &str,
    base_style: Style,
    accent: Color,
) -> Vec<Span<'static>> {
    if query.is_empty() {
        return vec![Span::styled(text.to_string(), base_style)];
    }

    let text_lower = text.to_lowercase();
    let query_lower = query.to_lowercase();
    let mut spans = Vec::new();
    let mut last_end = 0;

    for (start, _) in text_lower.match_indices(&query_lower) {
        // Add non-matching prefix
        if start > last_end {
            spans.push(Span::styled(text[last_end..start].to_string(), base_style));
        }
        // Add matching part with highlight
        let end = start + query.len();
        spans.push(Span::styled(
            text[start..end].to_string(),
            base_style.fg(accent).add_modifier(Modifier::BOLD),
        ));
        last_end = end;
    }

    // Add remaining text
    if last_end < text.len() {
        spans.push(Span::styled(text[last_end..].to_string(), base_style));
    }

    if spans.is_empty() {
        vec![Span::styled(text.to_string(), base_style)]
    } else {
        spans
    }
}
