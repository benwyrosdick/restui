mod dialog;
mod env_popup;
mod help;
mod layout;
mod request_editor;
mod request_list;
mod response;
mod theme_popup;
mod url_bar;
pub mod widgets;

use crate::app::App;
use ratatui::Frame;

pub fn draw(frame: &mut Frame, app: &mut App) {
    layout::draw_layout(frame, app);

    // Draw dialog popup on top if showing (higher priority than help)
    if app.dialog.dialog_type.is_some() {
        dialog::draw_dialog(frame, app);
    } else if app.show_env_popup {
        env_popup::draw_env_popup(frame, app);
    } else if app.show_theme_popup {
        theme_popup::draw_theme_popup(frame, app);
    } else if app.show_help {
        help::draw_help(frame, app);
    }
}
