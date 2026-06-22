mod dialog;
mod env_popup;
mod filter_history;
mod help;
mod layout;
mod request_editor;
mod request_list;
mod response;
mod theme_popup;
mod url_bar;
pub mod widgets;

use crate::app::App;
use crate::storage::HttpMethod;
use ratatui::style::Color;
use ratatui::Frame;

/// Color used to render an HTTP method label, shared across all panels.
pub(crate) fn method_color(method: HttpMethod) -> Color {
    match method {
        HttpMethod::Get => Color::Green,
        HttpMethod::Post => Color::Yellow,
        HttpMethod::Put => Color::Blue,
        HttpMethod::Patch => Color::Magenta,
        HttpMethod::Delete => Color::Red,
    }
}

/// Color used to render an HTTP status code (None = request error).
pub(crate) fn status_color(code: Option<u16>) -> Color {
    match code {
        Some(c) if (200..300).contains(&c) => Color::Green,
        Some(c) if c >= 400 => Color::Red,
        Some(_) => Color::Yellow,
        None => Color::Red,
    }
}

pub fn draw(frame: &mut Frame, app: &mut App) {
    layout::draw_layout(frame, app);

    // Draw dialog popup on top if showing (higher priority than help)
    if app.dialog.dialog_type.is_some() {
        dialog::draw_dialog(frame, app);
    } else if app.show_env_popup {
        env_popup::draw_env_popup(frame, app);
    } else if app.show_theme_popup {
        theme_popup::draw_theme_popup(frame, app);
    } else if app.show_filter_history {
        filter_history::draw_filter_history(frame, app);
    } else if app.show_help {
        help::draw_help(frame, app);
    }
}
