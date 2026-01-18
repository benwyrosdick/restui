use crate::app::{App, InputMode};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use super::{request_editor, request_list, response, url_bar};

/// Helper to convert Rect to tuple for storage
fn rect_to_tuple(r: Rect) -> (u16, u16, u16, u16) {
    (r.x, r.y, r.width, r.height)
}

/// Parse a color string into a ratatui Color, returning (background, foreground) for good contrast
fn parse_color_with_contrast(color_str: &str) -> (Color, Color) {
    let bg = match color_str.to_lowercase().as_str() {
        "red" => Color::Red,
        "green" => Color::Green,
        "blue" => Color::Blue,
        "yellow" => Color::Yellow,
        "magenta" | "purple" => Color::Magenta,
        "cyan" => Color::Cyan,
        "white" => Color::White,
        "black" => Color::Black,
        "gray" | "grey" => Color::Gray,
        "darkgray" | "darkgrey" => Color::DarkGray,
        "lightred" => Color::LightRed,
        "lightgreen" => Color::LightGreen,
        "lightblue" => Color::LightBlue,
        "lightyellow" => Color::LightYellow,
        "lightmagenta" => Color::LightMagenta,
        "lightcyan" => Color::LightCyan,
        s if s.starts_with('#') && s.len() == 7 => {
            // Parse hex color like "#FF5733"
            let r = u8::from_str_radix(&s[1..3], 16).unwrap_or(0);
            let g = u8::from_str_radix(&s[3..5], 16).unwrap_or(0);
            let b = u8::from_str_radix(&s[5..7], 16).unwrap_or(0);
            Color::Rgb(r, g, b)
        }
        _ => Color::Blue, // Default fallback
    };

    // Determine foreground color based on background luminance
    let fg = get_contrast_color(bg);
    (bg, fg)
}

/// Get a contrasting foreground color (black or white) based on background luminance
fn get_contrast_color(bg: Color) -> Color {
    let (r, g, b) = match bg {
        Color::Rgb(r, g, b) => (r, g, b),
        // Approximate RGB values for named colors
        Color::Black => (0, 0, 0),
        Color::Red => (205, 0, 0),
        Color::Green => (0, 205, 0),
        Color::Yellow => (205, 205, 0),
        Color::Blue => (0, 0, 238),
        Color::Magenta => (205, 0, 205),
        Color::Cyan => (0, 205, 205),
        Color::Gray => (128, 128, 128),
        Color::DarkGray => (85, 85, 85),
        Color::LightRed => (255, 85, 85),
        Color::LightGreen => (85, 255, 85),
        Color::LightYellow => (255, 255, 85),
        Color::LightBlue => (85, 85, 255),
        Color::LightMagenta => (255, 85, 255),
        Color::LightCyan => (85, 255, 255),
        Color::White => (255, 255, 255),
        _ => (128, 128, 128), // Default to mid-gray for unknown
    };

    // Calculate relative luminance using sRGB formula
    // https://www.w3.org/TR/WCAG20/#relativeluminancedef
    let luminance = 0.299 * (r as f64) + 0.587 * (g as f64) + 0.114 * (b as f64);

    // Use black text for light backgrounds, white text for dark backgrounds
    if luminance > 150.0 {
        Color::Black
    } else {
        Color::White
    }
}

/// Main application layout
pub fn draw_layout(frame: &mut Frame, app: &mut App) {
    let size = frame.area();

    // Main vertical layout: header, main content, footer
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Header
            Constraint::Min(10),   // Main content
            Constraint::Length(1), // Footer
        ])
        .split(size);

    // Draw header
    draw_header(frame, app, chunks[0]);

    // Main horizontal layout: left panel (30%), right panel (70%)
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(chunks[1]);

    // Store layout areas for mouse click detection
    app.layout_areas.request_list = Some(rect_to_tuple(main_chunks[0]));

    // Left panel: Request list / History
    request_list::draw(frame, app, main_chunks[0]);

    // Right panel: URL bar + Request editor + Response viewer
    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),      // URL bar
            Constraint::Percentage(40), // Request editor
            Constraint::Min(5),         // Response viewer (fills remaining space)
        ])
        .split(main_chunks[1]);

    // Store more layout areas
    app.layout_areas.url_bar = Some(rect_to_tuple(right_chunks[0]));
    app.layout_areas.request_editor = Some(rect_to_tuple(right_chunks[1]));
    app.layout_areas.response_view = Some(rect_to_tuple(right_chunks[2]));

    // URL bar
    url_bar::draw(frame, app, right_chunks[0]);

    // Request editor (also stores tab positions)
    request_editor::draw(frame, app, right_chunks[1]);

    // Response viewer
    response::draw(frame, app, right_chunks[2]);

    // Draw footer
    draw_footer(frame, app, chunks[2]);
}

fn draw_header(frame: &mut Frame, app: &App, area: Rect) {
    let env_name = app.environments.active_name();
    let version = env!("CARGO_PKG_VERSION");

    // Split header into left and right halves
    let header_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    // Left half: Dark gray background (matching footer) with "ResTUI" in white and version in purple
    let left_content = Line::from(vec![
        Span::styled(" ResTUI ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        Span::styled(format!("v{}", version), Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
    ]);
    let left_header = Paragraph::new(left_content)
        .style(Style::default().bg(Color::DarkGray));
    frame.render_widget(left_header, header_chunks[0]);

    // Right half: Environment-colored background with environment name
    let (bg_color, fg_color) = app.environments.active_color()
        .map(parse_color_with_contrast)
        .unwrap_or((Color::Blue, Color::White));

    let right_title = format!("[Env: {}] ", env_name);
    let right_header = Paragraph::new(right_title)
        .style(Style::default().bg(bg_color).fg(fg_color).add_modifier(Modifier::BOLD))
        .alignment(ratatui::layout::Alignment::Right);
    frame.render_widget(right_header, header_chunks[1]);
}

fn draw_footer(frame: &mut Frame, app: &App, area: Rect) {
    let mode_indicator = match app.input_mode {
        InputMode::Normal => Span::styled(
            " NORMAL ",
            Style::default().bg(Color::DarkGray).fg(Color::White),
        ),
        InputMode::Editing => Span::styled(
            " EDITING ",
            Style::default().bg(Color::Green).fg(Color::Black),
        ),
    };

    // Show status or error message
    let message = if let Some(err) = &app.error_message {
        Span::styled(
            format!(" {} ", err),
            Style::default().fg(Color::Red),
        )
    } else if let Some(status) = &app.status_message {
        Span::styled(
            format!(" {} ", status),
            Style::default().fg(Color::Cyan),
        )
    } else {
        Span::raw(" [S]end [N]ew [E]nv [H]istory [?]help | Tab:switch | q:quit ")
    };

    let footer_content = Line::from(vec![mode_indicator, message]);
    let footer = Paragraph::new(footer_content)
        .style(Style::default().bg(Color::DarkGray));

    frame.render_widget(footer, area);
}

/// Helper to create a bordered block with focus indication
pub fn bordered_block(title: &str, focused: bool) -> Block<'_> {
    let border_style = if focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title(format!(" {} ", title))
        .title_style(if focused {
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        })
}
