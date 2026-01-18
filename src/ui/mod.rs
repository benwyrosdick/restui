mod help;
mod layout;
mod request_editor;
mod request_list;
mod response;
mod url_bar;
pub mod widgets;

use crate::app::App;
use ratatui::Frame;

pub fn draw(frame: &mut Frame, app: &mut App) {
    layout::draw_layout(frame, app);

    // Draw help popup on top if showing
    if app.show_help {
        help::draw_help(frame, app);
    }
}
