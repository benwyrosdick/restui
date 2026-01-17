mod layout;
mod request_editor;
mod request_list;
mod response;
pub mod widgets;

use crate::app::App;
use ratatui::Frame;

pub fn draw(frame: &mut Frame, app: &App) {
    layout::draw_layout(frame, app);
}
