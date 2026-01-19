use crate::config::Config;
use crate::http::{HttpClient, HttpResponse};
use crate::storage::{
    ApiRequest, Collection, CollectionItem, EnvironmentManager, HistoryEntry, HistoryManager,
    HttpMethod,
};
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::style::Color;
use std::path::PathBuf;

/// Which panel is currently focused
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FocusedPanel {
    #[default]
    RequestList,
    UrlBar,
    RequestEditor,
    ResponseView,
}

impl FocusedPanel {
    pub fn next(&self) -> Self {
        match self {
            FocusedPanel::RequestList => FocusedPanel::UrlBar,
            FocusedPanel::UrlBar => FocusedPanel::RequestEditor,
            FocusedPanel::RequestEditor => FocusedPanel::ResponseView,
            FocusedPanel::ResponseView => FocusedPanel::RequestList,
        }
    }

    pub fn prev(&self) -> Self {
        match self {
            FocusedPanel::RequestList => FocusedPanel::ResponseView,
            FocusedPanel::UrlBar => FocusedPanel::RequestList,
            FocusedPanel::RequestEditor => FocusedPanel::UrlBar,
            FocusedPanel::ResponseView => FocusedPanel::RequestEditor,
        }
    }
}

/// Which tab is active in the request editor
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RequestTab {
    #[default]
    Headers,
    Body,
    Auth,
    Params,
}

impl RequestTab {
    pub fn all() -> &'static [RequestTab] {
        &[
            RequestTab::Headers,
            RequestTab::Body,
            RequestTab::Auth,
            RequestTab::Params,
        ]
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            RequestTab::Headers => "Headers",
            RequestTab::Body => "Body",
            RequestTab::Auth => "Auth",
            RequestTab::Params => "Params",
        }
    }

    pub fn next(&self) -> Self {
        match self {
            RequestTab::Headers => RequestTab::Body,
            RequestTab::Body => RequestTab::Auth,
            RequestTab::Auth => RequestTab::Params,
            RequestTab::Params => RequestTab::Headers,
        }
    }

    pub fn prev(&self) -> Self {
        match self {
            RequestTab::Headers => RequestTab::Params,
            RequestTab::Body => RequestTab::Headers,
            RequestTab::Auth => RequestTab::Body,
            RequestTab::Params => RequestTab::Auth,
        }
    }
}

/// Input mode for text editing
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InputMode {
    #[default]
    Normal,
    Editing,
}

/// Which field is being edited
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditingField {
    Url,
    HeaderKey(usize),
    HeaderValue(usize),
    Body,
    ParamKey(usize),
    ParamValue(usize),
    AuthBearerToken,
    AuthBasicUsername,
    AuthBasicPassword,
    AuthApiKeyName,
    AuthApiKeyValue,
}

/// Type of item being operated on
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ItemType {
    Collection,
    Folder,
    Request,
}

/// Type of dialog currently being shown
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DialogType {
    CreateCollection,
    CreateFolder { parent_collection: usize, parent_folder_id: Option<String> },
    CreateRequest { parent_collection: usize, parent_folder_id: Option<String> },
    RenameItem { item_type: ItemType, item_id: String, collection_index: usize },
    ConfirmDelete { item_type: ItemType, item_id: String, item_name: String, collection_index: usize },
}

/// Dialog state for input dialogs
#[derive(Debug, Clone, Default)]
pub struct DialogState {
    pub dialog_type: Option<DialogType>,
    pub input_buffer: String,
}

/// State for a pending move operation
#[derive(Debug, Clone)]
pub struct PendingMove {
    pub item_id: String,
    pub item_type: ItemType,
    pub item_name: String,
    pub source_collection_index: usize,
}

/// Application state
pub struct App {
    pub config: Config,
    pub collections: Vec<Collection>,
    pub history: HistoryManager,
    pub environments: EnvironmentManager,
    pub http_client: HttpClient,

    // UI state
    pub focused_panel: FocusedPanel,
    pub request_tab: RequestTab,
    pub input_mode: InputMode,
    pub editing_field: Option<EditingField>,
    pub cursor_position: usize,

    // Selection state
    pub selected_collection: usize,
    pub selected_item: usize,
    pub selected_history: usize,
    pub show_history: bool,

    // Current request being edited
    pub current_request: ApiRequest,
    // Source of current request: (collection_index, request_id)
    pub current_request_source: Option<(usize, String)>,

    // Response state
    pub response: Option<HttpResponse>,
    pub is_loading: bool,

    // Status/error message
    pub status_message: Option<String>,
    pub error_message: Option<String>,

    // Response scroll
    pub response_scroll: u16,

    // Body scroll (for request body editor)
    pub body_scroll: u16,

    // Help popup
    pub show_help: bool,

    // Selected param index for navigation in Params tab
    pub selected_param_index: usize,
    // Selected header index for navigation in Headers tab
    pub selected_header_index: usize,

    // Dialog state
    pub dialog: DialogState,

    // Layout areas for mouse click detection
    pub layout_areas: LayoutAreas,

    // Pending move operation (cut/paste mode)
    pub pending_move: Option<PendingMove>,
}

/// Stores the layout areas for mouse click detection
#[derive(Debug, Clone, Default)]
pub struct LayoutAreas {
    pub request_list: Option<(u16, u16, u16, u16)>,  // x, y, width, height
    pub url_bar: Option<(u16, u16, u16, u16)>,
    pub request_editor: Option<(u16, u16, u16, u16)>,
    pub response_view: Option<(u16, u16, u16, u16)>,
    pub tabs_row_y: Option<u16>,  // y-coordinate of the tabs row
    pub tab_positions: Vec<(u16, u16, RequestTab)>,  // x, width, tab
    // Text field positions for click-to-cursor (x where text starts, y, width)
    pub url_text_start: Option<u16>,
    pub body_area: Option<(u16, u16, u16, u16)>,  // x, y, width, height for body text area
    pub request_content_area: Option<(u16, u16, u16, u16)>,  // content area below tabs
}

impl App {
    pub async fn new() -> Result<Self> {
        let config = Config::new()?;
        config.ensure_dirs()?;

        // Load existing data or create defaults
        let history = HistoryManager::load(&config.history_file).unwrap_or_default();
        let environments =
            EnvironmentManager::load(&config.environments_file).unwrap_or_else(|_| EnvironmentManager::new());

        // Load collections from the collections directory
        let collections = Self::load_collections(&config.collections_dir)?;

        let http_client = HttpClient::new()?;

        Ok(Self {
            config,
            collections,
            history,
            environments,
            http_client,
            focused_panel: FocusedPanel::default(),
            request_tab: RequestTab::default(),
            input_mode: InputMode::Normal,
            editing_field: None,
            cursor_position: 0,
            selected_collection: 0,
            selected_item: usize::MAX, // usize::MAX means collection header is selected
            selected_history: 0,
            show_history: false,
            current_request: ApiRequest::default(),
            current_request_source: None,
            response: None,
            is_loading: false,
            status_message: None,
            error_message: None,
            response_scroll: 0,
            body_scroll: 0,
            show_help: false,
            selected_param_index: 0,
            selected_header_index: 0,
            dialog: DialogState::default(),
            layout_areas: LayoutAreas::default(),
            pending_move: None,
        })
    }

    fn load_collections(dir: &PathBuf) -> Result<Vec<Collection>> {
        let mut collections = Vec::new();

        if dir.exists() {
            for entry in std::fs::read_dir(dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.extension().map_or(false, |ext| ext == "json") {
                    if let Ok(collection) = Collection::load(&path) {
                        collections.push(collection);
                    }
                }
            }
        }

        // If no collections, create a sample one
        if collections.is_empty() {
            let mut sample = Collection::new("Sample Collection");
            let mut req = ApiRequest::new("Get Users");
            req.url = "https://jsonplaceholder.typicode.com/users".to_string();
            sample.add_request(req);

            let mut req2 = ApiRequest::new("Create User");
            req2.method = HttpMethod::Post;
            req2.url = "https://jsonplaceholder.typicode.com/users".to_string();
            req2.body = r#"{"name": "John Doe", "email": "john@example.com"}"#.to_string();
            sample.add_request(req2);

            collections.push(sample);
        }

        Ok(collections)
    }

    /// Handle a key press event. Returns true if the app should quit.
    pub async fn handle_key(&mut self, key: KeyEvent) -> Result<bool> {
        // Clear any previous error on new input
        self.error_message = None;

        // Handle dialog input first if dialog is showing
        if self.dialog.dialog_type.is_some() {
            return self.handle_dialog_input(key);
        }

        // If help is showing, any key closes it
        if self.show_help {
            match key.code {
                KeyCode::Esc | KeyCode::Char('?') | KeyCode::Char('q') => {
                    self.show_help = false;
                }
                _ => {
                    self.show_help = false;
                }
            }
            return Ok(false);
        }

        // Global shortcuts
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            match key.code {
                KeyCode::Char('c') if self.input_mode == InputMode::Normal => {
                    return Ok(true);
                }
                KeyCode::Char('s') => {
                    self.save_current_request();
                    return Ok(false);
                }
                _ => {}
            }
        }

        // Quit shortcut
        if key.code == KeyCode::Char('q') && self.input_mode == InputMode::Normal {
            return Ok(true);
        }

        // Mode-specific handling
        match self.input_mode {
            InputMode::Normal => self.handle_normal_mode(key).await,
            InputMode::Editing => self.handle_editing_mode(key),
        }
    }

    /// Handle mouse click events
    pub fn handle_mouse_click(&mut self, x: u16, y: u16) {
        // Close help popup if showing
        if self.show_help {
            self.show_help = false;
            return;
        }

        // Check which panel was clicked
        if let Some((px, py, pw, ph)) = self.layout_areas.request_list {
            if x >= px && x < px + pw && y >= py && y < py + ph {
                self.focused_panel = FocusedPanel::RequestList;
                self.input_mode = InputMode::Normal;
                self.editing_field = None;

                // Calculate which item was clicked (accounting for border)
                let relative_y = y.saturating_sub(py + 1) as usize; // +1 for border
                if self.show_history {
                    let max = self.history.entries.len().saturating_sub(1);
                    self.selected_history = relative_y.min(max);
                } else {
                    // Map visual row to (collection_index, item_index)
                    // Visual rows: collection headers + their items
                    let mut visual_row = 0;
                    for (col_idx, collection) in self.collections.iter().enumerate() {
                        // Collection header row
                        if visual_row == relative_y {
                            // Clicked on collection header - select it
                            self.selected_collection = col_idx;
                            self.selected_item = usize::MAX; // Header selected
                            return;
                        }
                        visual_row += 1;

                        if collection.expanded {
                            let item_count = collection.flatten().len();
                            if relative_y < visual_row + item_count {
                                // Clicked on an item in this collection
                                self.selected_collection = col_idx;
                                self.selected_item = relative_y - visual_row;
                                return;
                            }
                            visual_row += item_count;
                        }
                    }
                }
                return;
            }
        }

        if let Some((px, py, pw, ph)) = self.layout_areas.url_bar {
            if x >= px && x < px + pw && y >= py && y < py + ph {
                self.focused_panel = FocusedPanel::UrlBar;
                // Start editing URL on click
                self.input_mode = InputMode::Editing;
                self.editing_field = Some(EditingField::Url);

                // Position cursor based on click position
                if let Some(text_start) = self.layout_areas.url_text_start {
                    let url_len = self.current_request.url.chars().count();
                    if x >= text_start {
                        let click_offset = (x - text_start) as usize;
                        self.cursor_position = click_offset.min(url_len);
                    } else {
                        self.cursor_position = 0;
                    }
                } else {
                    self.cursor_position = self.current_request.url.len();
                }
                return;
            }
        }

        if let Some((px, py, pw, ph)) = self.layout_areas.request_editor {
            if x >= px && x < px + pw && y >= py && y < py + ph {
                self.focused_panel = FocusedPanel::RequestEditor;

                // Check if a tab was clicked (must be on the tabs row)
                if let Some(tabs_y) = self.layout_areas.tabs_row_y {
                    if y == tabs_y {
                        for (tab_x, tab_width, tab) in &self.layout_areas.tab_positions {
                            if x >= *tab_x && x < tab_x + tab_width {
                                self.request_tab = *tab;
                                self.input_mode = InputMode::Normal;
                                self.editing_field = None;
                                return;
                            }
                        }
                    }
                }

                // Check if content area was clicked
                if let Some((cx, cy, cw, ch)) = self.layout_areas.request_content_area {
                    if x >= cx && x < cx + cw && y >= cy && y < cy + ch {
                        let click_row = (y - cy) as usize;

                        match self.request_tab {
                            RequestTab::Body => {
                                // Handle body click-to-cursor
                                if let Some((bx, by, bw, bh)) = self.layout_areas.body_area {
                                    if x >= bx && x < bx + bw && y >= by && y < by + bh {
                                        self.input_mode = InputMode::Editing;
                                        self.editing_field = Some(EditingField::Body);

                                        // Account for scroll offset when calculating clicked row
                                        let click_row = (y - by) as usize + self.body_scroll as usize;
                                        let click_col = (x - bx) as usize;

                                        let body = &self.current_request.body;
                                        let lines: Vec<&str> = body.split('\n').collect();

                                        let mut char_pos = 0;
                                        for (i, line) in lines.iter().enumerate() {
                                            if i == click_row {
                                                char_pos += click_col.min(line.len());
                                                break;
                                            }
                                            if i < lines.len() - 1 {
                                                char_pos += line.len() + 1;
                                            }
                                        }

                                        self.cursor_position = char_pos.min(body.len());
                                        return;
                                    }
                                }
                            }
                            RequestTab::Params => {
                                // Select the clicked param
                                let param_count = self.current_request.query_params.len();
                                if click_row < param_count {
                                    self.selected_param_index = click_row;
                                    self.input_mode = InputMode::Normal;
                                    self.editing_field = None;
                                    return;
                                }
                            }
                            RequestTab::Headers => {
                                // Select the clicked header
                                let header_count = self.current_request.headers.len();
                                if click_row < header_count {
                                    self.selected_header_index = click_row;
                                    self.input_mode = InputMode::Normal;
                                    self.editing_field = None;
                                    return;
                                }
                            }
                            _ => {}
                        }
                    }
                }

                // Otherwise just focus the panel
                self.input_mode = InputMode::Normal;
                self.editing_field = None;
                return;
            }
        }

        if let Some((px, py, pw, ph)) = self.layout_areas.response_view {
            if x >= px && x < px + pw && y >= py && y < py + ph {
                self.focused_panel = FocusedPanel::ResponseView;
                self.input_mode = InputMode::Normal;
                self.editing_field = None;
                return;
            }
        }
    }

    /// Handle mouse scroll wheel events
    pub fn handle_scroll(&mut self, x: u16, y: u16, up: bool) {
        // Check if scroll is within response pane
        if let Some((px, py, pw, ph)) = self.layout_areas.response_view {
            if x >= px && x < px + pw && y >= py && y < py + ph {
                if up {
                    self.response_scroll = self.response_scroll.saturating_sub(3);
                } else {
                    self.response_scroll = self.response_scroll.saturating_add(3);
                }
                return;
            }
        }

        // Check if scroll is within body area
        if let Some((bx, by, bw, bh)) = self.layout_areas.body_area {
            if x >= bx && x < bx + bw && y >= by && y < by + bh {
                if up {
                    self.body_scroll = self.body_scroll.saturating_sub(3);
                } else {
                    self.body_scroll = self.body_scroll.saturating_add(3);
                }
            }
        }
    }

    /// Get the accent color based on the active environment (defaults to Cyan)
    pub fn accent_color(&self) -> Color {
        self.environments.active_color()
            .map(|s| Self::parse_color(s))
            .unwrap_or(Color::Cyan)
    }

    /// Parse a color string into a ratatui Color
    fn parse_color(color_str: &str) -> Color {
        match color_str.to_lowercase().as_str() {
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
                let r = u8::from_str_radix(&s[1..3], 16).unwrap_or(0);
                let g = u8::from_str_radix(&s[3..5], 16).unwrap_or(0);
                let b = u8::from_str_radix(&s[5..7], 16).unwrap_or(0);
                Color::Rgb(r, g, b)
            }
            _ => Color::Cyan,
        }
    }

    async fn handle_normal_mode(&mut self, key: KeyEvent) -> Result<bool> {
        // Cancel pending move with Esc
        if key.code == KeyCode::Esc && self.pending_move.is_some() {
            self.pending_move = None;
            self.status_message = Some("Move cancelled".to_string());
            return Ok(false);
        }

        match key.code {
            // Panel navigation
            KeyCode::Tab => {
                self.focused_panel = self.focused_panel.next();
            }
            KeyCode::BackTab => {
                self.focused_panel = self.focused_panel.prev();
            }

            // Arrow keys for navigation
            KeyCode::Up | KeyCode::Char('k') => self.navigate_up(),
            KeyCode::Down | KeyCode::Char('j') => self.navigate_down(),
            KeyCode::Left | KeyCode::Char('h') => self.navigate_left(),
            KeyCode::Right | KeyCode::Char('l') => self.navigate_right(),

            // Enter to select/edit
            KeyCode::Enter => self.handle_enter().await?,

            // Send request
            KeyCode::Char('s') | KeyCode::Char('S') => {
                if self.focused_panel != FocusedPanel::RequestList {
                    self.send_request().await?;
                }
            }

            // Toggle history view
            KeyCode::Char('H') => {
                self.show_history = !self.show_history;
            }

            // New request
            KeyCode::Char('n') | KeyCode::Char('N') => {
                self.new_request();
            }

            // Switch environment
            KeyCode::Char('e') => {
                self.environments.next();
                self.status_message = Some(format!(
                    "Switched to environment: {}",
                    self.environments.active_name()
                ));
            }

            // Reload environments from disk
            KeyCode::Char('E') => {
                self.reload_environments();
            }

            // Edit current field
            KeyCode::Char('i') => {
                if self.focused_panel == FocusedPanel::UrlBar {
                    self.input_mode = InputMode::Editing;
                    self.set_editing_field(EditingField::Url);
                } else if self.focused_panel == FocusedPanel::RequestEditor {
                    self.enter_edit_mode();
                }
            }

            // Cycle HTTP method (not in RequestList - 'm' is used for move there)
            KeyCode::Char('m') | KeyCode::Char('M')
                if self.focused_panel == FocusedPanel::UrlBar
                    || self.focused_panel == FocusedPanel::RequestEditor =>
            {
                self.current_request.method = self.current_request.method.next();
            }

            // Cycle auth type
            KeyCode::Char('a') => {
                if self.focused_panel == FocusedPanel::RequestEditor
                    && self.request_tab == RequestTab::Auth
                {
                    self.current_request.auth.auth_type =
                        self.current_request.auth.auth_type.next();
                }
            }

            // Toggle param/header enabled/disabled
            KeyCode::Char('t') => {
                if self.focused_panel == FocusedPanel::RequestEditor {
                    match self.request_tab {
                        RequestTab::Params => self.toggle_selected_param(),
                        RequestTab::Headers => self.toggle_selected_header(),
                        _ => {}
                    }
                }
            }

            // Delete selected param/header
            KeyCode::Char('x') => {
                if self.focused_panel == FocusedPanel::RequestEditor {
                    match self.request_tab {
                        RequestTab::Params => self.delete_selected_param(),
                        RequestTab::Headers => self.delete_selected_header(),
                        _ => {}
                    }
                }
            }

            // Help popup
            KeyCode::Char('?') => {
                self.show_help = true;
            }

            // Panel switching by number
            KeyCode::Char('1') => {
                self.focused_panel = FocusedPanel::RequestList;
            }
            KeyCode::Char('2') => {
                self.focused_panel = FocusedPanel::UrlBar;
            }
            KeyCode::Char('3') => {
                self.focused_panel = FocusedPanel::RequestEditor;
            }
            KeyCode::Char('4') => {
                self.focused_panel = FocusedPanel::ResponseView;
            }

            // Save current request (W for write, like vim :w)
            KeyCode::Char('W') => {
                self.save_current_request();
            }

            // Copy request as curl command
            KeyCode::Char('y') => {
                self.copy_as_curl();
            }

            // Copy response body to clipboard (in response view)
            KeyCode::Char('c') if self.focused_panel == FocusedPanel::ResponseView => {
                self.copy_response();
            }

            // Format JSON body
            KeyCode::Char('f') if self.focused_panel == FocusedPanel::RequestEditor
                && self.request_tab == RequestTab::Body =>
            {
                self.format_body_json();
            }

            // CRUD operations (only in RequestList panel)
            KeyCode::Char('C') if self.focused_panel == FocusedPanel::RequestList => {
                self.start_create_collection();
            }
            KeyCode::Char('F') if self.focused_panel == FocusedPanel::RequestList => {
                self.start_create_folder();
            }
            KeyCode::Char('r') if self.focused_panel == FocusedPanel::RequestList && !self.show_history => {
                self.start_create_request();
            }
            KeyCode::Char('R') if self.focused_panel == FocusedPanel::RequestList => {
                self.start_rename_item();
            }
            KeyCode::Char('d') | KeyCode::Delete if self.focused_panel == FocusedPanel::RequestList => {
                self.start_delete_item();
            }
            // Delete collection with D
            KeyCode::Char('D') if self.focused_panel == FocusedPanel::RequestList && !self.show_history => {
                self.start_delete_collection();
            }
            // Duplicate request with p
            KeyCode::Char('p') if self.focused_panel == FocusedPanel::RequestList && !self.show_history => {
                self.duplicate_selected_request();
            }
            // Toggle expand/collapse with space
            KeyCode::Char(' ') if self.focused_panel == FocusedPanel::RequestList && !self.show_history => {
                self.toggle_expand_collapse();
            }
            // Move item with m
            KeyCode::Char('m') if self.focused_panel == FocusedPanel::RequestList && !self.show_history => {
                self.start_move_item();
            }

            _ => {}
        }

        Ok(false)
    }

    fn handle_editing_mode(&mut self, key: KeyEvent) -> Result<bool> {
        match key.code {
            KeyCode::Esc => {
                self.input_mode = InputMode::Normal;
                self.editing_field = None;
            }
            // Tab to move to next field
            KeyCode::Tab => {
                self.next_editing_field();
            }
            KeyCode::Enter => {
                // For body, add newline at cursor
                // For other fields, move to next field
                if matches!(self.editing_field, Some(EditingField::Body)) {
                    self.handle_char_input('\n');
                } else {
                    self.next_editing_field();
                }
            }
            KeyCode::Backspace => {
                self.handle_backspace();
            }
            KeyCode::Delete => {
                self.handle_delete();
            }
            KeyCode::Left => {
                self.cursor_left();
            }
            KeyCode::Right => {
                self.cursor_right();
            }
            KeyCode::Up => {
                self.cursor_up();
            }
            KeyCode::Down => {
                self.cursor_down();
            }
            KeyCode::Home => {
                self.cursor_home();
            }
            KeyCode::End => {
                self.cursor_end();
            }
            KeyCode::Char(c) => {
                self.handle_char_input(c);
            }
            _ => {}
        }
        Ok(false)
    }

    /// Get mutable reference to current editing field's text
    fn get_current_field_mut(&mut self) -> Option<&mut String> {
        let field = self.editing_field.clone()?;
        match field {
            EditingField::Url => Some(&mut self.current_request.url),
            EditingField::Body => Some(&mut self.current_request.body),
            EditingField::HeaderKey(i) => self.current_request.headers.get_mut(i).map(|h| &mut h.key),
            EditingField::HeaderValue(i) => self.current_request.headers.get_mut(i).map(|h| &mut h.value),
            EditingField::ParamKey(i) => self.current_request.query_params.get_mut(i).map(|p| &mut p.key),
            EditingField::ParamValue(i) => self.current_request.query_params.get_mut(i).map(|p| &mut p.value),
            EditingField::AuthBearerToken => Some(&mut self.current_request.auth.bearer_token),
            EditingField::AuthBasicUsername => Some(&mut self.current_request.auth.basic_username),
            EditingField::AuthBasicPassword => Some(&mut self.current_request.auth.basic_password),
            EditingField::AuthApiKeyName => Some(&mut self.current_request.auth.api_key_name),
            EditingField::AuthApiKeyValue => Some(&mut self.current_request.auth.api_key_value),
        }
    }

    /// Get current field text length
    fn get_current_field_len(&self) -> usize {
        let Some(field) = &self.editing_field else { return 0 };
        match field {
            EditingField::Url => self.current_request.url.len(),
            EditingField::Body => self.current_request.body.len(),
            EditingField::HeaderKey(i) => self.current_request.headers.get(*i).map(|h| h.key.len()).unwrap_or(0),
            EditingField::HeaderValue(i) => self.current_request.headers.get(*i).map(|h| h.value.len()).unwrap_or(0),
            EditingField::ParamKey(i) => self.current_request.query_params.get(*i).map(|p| p.key.len()).unwrap_or(0),
            EditingField::ParamValue(i) => self.current_request.query_params.get(*i).map(|p| p.value.len()).unwrap_or(0),
            EditingField::AuthBearerToken => self.current_request.auth.bearer_token.len(),
            EditingField::AuthBasicUsername => self.current_request.auth.basic_username.len(),
            EditingField::AuthBasicPassword => self.current_request.auth.basic_password.len(),
            EditingField::AuthApiKeyName => self.current_request.auth.api_key_name.len(),
            EditingField::AuthApiKeyValue => self.current_request.auth.api_key_value.len(),
        }
    }

    fn handle_backspace(&mut self) {
        let cursor_pos = self.cursor_position;
        if cursor_pos > 0 {
            if let Some(text) = self.get_current_field_mut() {
                // Remove character before cursor
                let byte_pos = text.char_indices()
                    .nth(cursor_pos - 1)
                    .map(|(i, _)| i)
                    .unwrap_or(0);
                let next_byte_pos = text.char_indices()
                    .nth(cursor_pos)
                    .map(|(i, _)| i)
                    .unwrap_or(text.len());
                text.replace_range(byte_pos..next_byte_pos, "");
            }
            self.cursor_position -= 1;
        }
    }

    fn handle_delete(&mut self) {
        let len = self.get_current_field_len();
        let cursor_pos = self.cursor_position;
        if cursor_pos < len {
            if let Some(text) = self.get_current_field_mut() {
                // Remove character at cursor
                let byte_pos = text.char_indices()
                    .nth(cursor_pos)
                    .map(|(i, _)| i)
                    .unwrap_or(text.len());
                let next_byte_pos = text.char_indices()
                    .nth(cursor_pos + 1)
                    .map(|(i, _)| i)
                    .unwrap_or(text.len());
                text.replace_range(byte_pos..next_byte_pos, "");
            }
        }
    }

    fn handle_char_input(&mut self, c: char) {
        let cursor_pos = self.cursor_position;
        if let Some(text) = self.get_current_field_mut() {
            // Insert character at cursor position
            let byte_pos = text.char_indices()
                .nth(cursor_pos)
                .map(|(i, _)| i)
                .unwrap_or(text.len());
            text.insert(byte_pos, c);
        }
        self.cursor_position += 1;
        // Keep cursor visible when typing (especially for newlines)
        self.ensure_body_cursor_visible();
    }

    fn cursor_left(&mut self) {
        if self.cursor_position > 0 {
            self.cursor_position -= 1;
        }
    }

    fn cursor_right(&mut self) {
        let len = self.get_current_field_len();
        if self.cursor_position < len {
            self.cursor_position += 1;
        }
    }

    fn cursor_home(&mut self) {
        self.cursor_position = 0;
    }

    fn cursor_end(&mut self) {
        self.cursor_position = self.get_current_field_len();
    }

    fn cursor_up(&mut self) {
        // Only works for body field (multiline)
        if !matches!(self.editing_field, Some(EditingField::Body)) {
            return;
        }

        let body = &self.current_request.body;
        let cursor_pos = self.cursor_position.min(body.len());

        // Find current line start and position within line
        let before_cursor = &body[..cursor_pos];
        let current_line_start = before_cursor.rfind('\n').map(|i| i + 1).unwrap_or(0);
        let col = cursor_pos - current_line_start;

        // If we're on the first line, can't go up
        if current_line_start == 0 {
            return;
        }

        // Find previous line
        let prev_line_end = current_line_start - 1; // position of '\n'
        let prev_line_start = body[..prev_line_end].rfind('\n').map(|i| i + 1).unwrap_or(0);
        let prev_line_len = prev_line_end - prev_line_start;

        // Move to same column on previous line (or end of line if shorter)
        self.cursor_position = prev_line_start + col.min(prev_line_len);
        self.ensure_body_cursor_visible();
    }

    fn cursor_down(&mut self) {
        // Only works for body field (multiline)
        if !matches!(self.editing_field, Some(EditingField::Body)) {
            return;
        }

        let body = &self.current_request.body;
        let cursor_pos = self.cursor_position.min(body.len());

        // Find current line start and position within line
        let before_cursor = &body[..cursor_pos];
        let current_line_start = before_cursor.rfind('\n').map(|i| i + 1).unwrap_or(0);
        let col = cursor_pos - current_line_start;

        // Find next line
        let Some(next_line_start) = body[cursor_pos..].find('\n').map(|i| cursor_pos + i + 1) else {
            return; // No next line
        };

        // Find end of next line
        let next_line_end = body[next_line_start..].find('\n')
            .map(|i| next_line_start + i)
            .unwrap_or(body.len());
        let next_line_len = next_line_end - next_line_start;

        // Move to same column on next line (or end of line if shorter)
        self.cursor_position = next_line_start + col.min(next_line_len);
        self.ensure_body_cursor_visible();
    }

    /// Set editing field and position cursor at end
    fn set_editing_field(&mut self, field: EditingField) {
        self.editing_field = Some(field);
        self.cursor_position = self.get_current_field_len();
    }

    /// Ensure the cursor is visible in the body editor by adjusting scroll
    fn ensure_body_cursor_visible(&mut self) {
        if !matches!(self.editing_field, Some(EditingField::Body)) {
            return;
        }

        let body = &self.current_request.body;
        let cursor_pos = self.cursor_position.min(body.len());

        // Find which line the cursor is on
        let cursor_line = body[..cursor_pos].matches('\n').count();

        // Get visible height from layout (default to 10 if not set)
        let visible_height = self.layout_areas.body_area
            .map(|(_, _, _, h)| h as usize)
            .unwrap_or(10);

        // Adjust scroll if cursor is above visible area
        if cursor_line < self.body_scroll as usize {
            self.body_scroll = cursor_line as u16;
        }

        // Adjust scroll if cursor is below visible area
        if cursor_line >= self.body_scroll as usize + visible_height {
            self.body_scroll = (cursor_line - visible_height + 1) as u16;
        }
    }

    fn navigate_up(&mut self) {
        match self.focused_panel {
            FocusedPanel::RequestList => {
                if self.show_history {
                    self.selected_history = self.selected_history.saturating_sub(1);
                } else {
                    self.navigate_collection_up();
                }
            }
            FocusedPanel::ResponseView => {
                self.response_scroll = self.response_scroll.saturating_sub(1);
            }
            FocusedPanel::RequestEditor if self.request_tab == RequestTab::Params => {
                self.selected_param_index = self.selected_param_index.saturating_sub(1);
            }
            FocusedPanel::RequestEditor if self.request_tab == RequestTab::Headers => {
                self.selected_header_index = self.selected_header_index.saturating_sub(1);
            }
            FocusedPanel::RequestEditor if self.request_tab == RequestTab::Body => {
                self.body_scroll = self.body_scroll.saturating_sub(1);
            }
            _ => {}
        }
    }

    fn navigate_down(&mut self) {
        match self.focused_panel {
            FocusedPanel::RequestList => {
                if self.show_history {
                    let max = self.history.entries.len().saturating_sub(1);
                    self.selected_history = (self.selected_history + 1).min(max);
                } else {
                    self.navigate_collection_down();
                }
            }
            FocusedPanel::ResponseView => {
                self.response_scroll = self.response_scroll.saturating_add(1);
            }
            FocusedPanel::RequestEditor if self.request_tab == RequestTab::Params => {
                let max = self.current_request.query_params.len().saturating_sub(1);
                self.selected_param_index = (self.selected_param_index + 1).min(max);
            }
            FocusedPanel::RequestEditor if self.request_tab == RequestTab::Headers => {
                let max = self.current_request.headers.len().saturating_sub(1);
                self.selected_header_index = (self.selected_header_index + 1).min(max);
            }
            FocusedPanel::RequestEditor if self.request_tab == RequestTab::Body => {
                self.body_scroll = self.body_scroll.saturating_add(1);
            }
            _ => {}
        }
    }

    /// Check if collection header is selected (usize::MAX means header selected)
    pub fn is_collection_header_selected(&self) -> bool {
        self.selected_item == usize::MAX
    }

    fn navigate_collection_up(&mut self) {
        if self.collections.is_empty() {
            return;
        }

        let col = match self.collections.get(self.selected_collection) {
            Some(c) => c,
            None => return,
        };

        if col.expanded {
            if self.selected_item == usize::MAX {
                // Already on header, move to previous collection's last item
                if self.selected_collection > 0 {
                    self.selected_collection -= 1;
                    if let Some(prev_col) = self.collections.get(self.selected_collection) {
                        if prev_col.expanded {
                            let count = prev_col.flatten().len();
                            self.selected_item = if count > 0 { count - 1 } else { usize::MAX };
                        } else {
                            self.selected_item = usize::MAX;
                        }
                    }
                }
            } else if self.selected_item == 0 {
                // On first item, move to collection header
                self.selected_item = usize::MAX;
            } else {
                // Move up within items
                self.selected_item -= 1;
            }
        } else {
            // Collapsed collection - header is the only option, move to previous collection
            if self.selected_collection > 0 {
                self.selected_collection -= 1;
                if let Some(prev_col) = self.collections.get(self.selected_collection) {
                    if prev_col.expanded {
                        let count = prev_col.flatten().len();
                        self.selected_item = if count > 0 { count - 1 } else { usize::MAX };
                    } else {
                        self.selected_item = usize::MAX;
                    }
                }
            }
        }
    }

    fn navigate_collection_down(&mut self) {
        if self.collections.is_empty() {
            return;
        }

        let col = match self.collections.get(self.selected_collection) {
            Some(c) => c,
            None => return,
        };

        if col.expanded {
            let item_count = col.flatten().len();
            if self.selected_item == usize::MAX {
                // On header, move to first item (or next collection if no items)
                if item_count > 0 {
                    self.selected_item = 0;
                } else if self.selected_collection < self.collections.len() - 1 {
                    self.selected_collection += 1;
                    self.selected_item = usize::MAX;
                }
            } else if self.selected_item < item_count.saturating_sub(1) {
                // Move down within items
                self.selected_item += 1;
            } else if self.selected_collection < self.collections.len() - 1 {
                // At last item, move to next collection header
                self.selected_collection += 1;
                self.selected_item = usize::MAX;
            }
        } else {
            // Collapsed collection - move to next collection
            if self.selected_collection < self.collections.len() - 1 {
                self.selected_collection += 1;
                self.selected_item = usize::MAX;
            }
        }
    }

    fn navigate_left(&mut self) {
        if self.focused_panel == FocusedPanel::RequestEditor {
            self.request_tab = self.request_tab.prev();
        }
    }

    fn navigate_right(&mut self) {
        if self.focused_panel == FocusedPanel::RequestEditor {
            self.request_tab = self.request_tab.next();
        }
    }

    async fn handle_enter(&mut self) -> Result<()> {
        // Check for pending move operation
        if self.pending_move.is_some() && self.focused_panel == FocusedPanel::RequestList {
            self.execute_pending_move();
            return Ok(());
        }

        match self.focused_panel {
            FocusedPanel::RequestList => {
                if self.show_history {
                    // Load request from history
                    if let Some(entry) = self.history.entries.get(self.selected_history) {
                        self.current_request = entry.request.clone();
                        self.response = None;
                        self.selected_param_index = 0;
                        self.selected_header_index = 0;
                        self.body_scroll = 0;
                        self.focused_panel = FocusedPanel::UrlBar;
                    }
                } else {
                    // Load selected request from collection
                    self.load_selected_request();
                    self.focused_panel = FocusedPanel::UrlBar;
                }
            }
            FocusedPanel::UrlBar => {
                // Start editing URL
                self.input_mode = InputMode::Editing;
                self.set_editing_field(EditingField::Url);
            }
            FocusedPanel::RequestEditor => {
                self.enter_edit_mode();
            }
            FocusedPanel::ResponseView => {}
        }
        Ok(())
    }

    fn enter_edit_mode(&mut self) {
        self.input_mode = InputMode::Editing;
        // Set editing field based on current tab
        let field = self.get_default_editing_field();
        self.set_editing_field(field);
    }

    /// Get the default editing field for the current tab
    fn get_default_editing_field(&mut self) -> EditingField {
        match self.request_tab {
            RequestTab::Headers => {
                if self.current_request.headers.is_empty() {
                    // Add a new header if none exist
                    self.current_request.headers.push(crate::storage::KeyValue::new("", ""));
                    self.selected_header_index = 0;
                }
                // Start editing the selected header
                let idx = self.selected_header_index.min(self.current_request.headers.len().saturating_sub(1));
                EditingField::HeaderKey(idx)
            }
            RequestTab::Body => EditingField::Body,
            RequestTab::Auth => {
                match self.current_request.auth.auth_type {
                    crate::storage::AuthType::None => {
                        self.status_message = Some("Select auth type first with 'a' key".to_string());
                        EditingField::Url
                    }
                    crate::storage::AuthType::Bearer => EditingField::AuthBearerToken,
                    crate::storage::AuthType::Basic => EditingField::AuthBasicUsername,
                    crate::storage::AuthType::ApiKey => EditingField::AuthApiKeyName,
                }
            }
            RequestTab::Params => {
                if self.current_request.query_params.is_empty() {
                    self.current_request.query_params.push(crate::storage::KeyValue::new("", ""));
                    self.selected_param_index = 0;
                }
                // Start editing the selected param
                let idx = self.selected_param_index.min(self.current_request.query_params.len().saturating_sub(1));
                EditingField::ParamKey(idx)
            }
        }
    }

    /// Navigate to the next editable field within current context
    fn next_editing_field(&mut self) {
        let next = match (&self.editing_field, &self.request_tab) {
            // Headers: key -> value -> next key -> next value -> ...
            (Some(EditingField::HeaderKey(i)), RequestTab::Headers) => {
                EditingField::HeaderValue(*i)
            }
            (Some(EditingField::HeaderValue(i)), RequestTab::Headers) => {
                let next_idx = i + 1;
                if next_idx < self.current_request.headers.len() {
                    EditingField::HeaderKey(next_idx)
                } else {
                    // Add new header and edit it
                    self.current_request.headers.push(crate::storage::KeyValue::new("", ""));
                    EditingField::HeaderKey(next_idx)
                }
            }
            // Params: key -> value -> next key -> next value -> ...
            (Some(EditingField::ParamKey(i)), RequestTab::Params) => {
                EditingField::ParamValue(*i)
            }
            (Some(EditingField::ParamValue(i)), RequestTab::Params) => {
                let next_idx = i + 1;
                if next_idx < self.current_request.query_params.len() {
                    EditingField::ParamKey(next_idx)
                } else {
                    // Add new param and edit it
                    self.current_request.query_params.push(crate::storage::KeyValue::new("", ""));
                    EditingField::ParamKey(next_idx)
                }
            }
            // Auth: cycle through auth fields
            (Some(EditingField::AuthBearerToken), RequestTab::Auth) => {
                EditingField::AuthBearerToken // Only one field for bearer
            }
            (Some(EditingField::AuthBasicUsername), RequestTab::Auth) => {
                EditingField::AuthBasicPassword
            }
            (Some(EditingField::AuthBasicPassword), RequestTab::Auth) => {
                EditingField::AuthBasicUsername
            }
            (Some(EditingField::AuthApiKeyName), RequestTab::Auth) => {
                EditingField::AuthApiKeyValue
            }
            (Some(EditingField::AuthApiKeyValue), RequestTab::Auth) => {
                EditingField::AuthApiKeyName
            }
            // Body: stay on body
            (Some(EditingField::Body), RequestTab::Body) => EditingField::Body,
            // URL stays on URL
            (Some(EditingField::Url), _) => EditingField::Url,
            // Default
            _ => self.get_default_editing_field(),
        };
        self.set_editing_field(next);
    }

    fn load_selected_request(&mut self) {
        if let Some(collection) = self.collections.get(self.selected_collection) {
            let flattened = collection.flatten();
            if let Some((_, item)) = flattened.get(self.selected_item) {
                if let CollectionItem::Request(req) = item {
                    self.current_request = req.clone();
                    self.current_request_source = Some((self.selected_collection, req.id.clone()));
                    self.response = None;
                    self.selected_param_index = 0;
                    self.selected_header_index = 0;
                    self.body_scroll = 0;
                }
            }
        }
    }

    fn get_visible_items_count(&self) -> usize {
        self.collections
            .get(self.selected_collection)
            .map(|c| if c.expanded { c.flatten().len() } else { 0 })
            .unwrap_or(0)
    }

    fn new_request(&mut self) {
        self.current_request = ApiRequest::default();
        self.current_request_source = None;
        self.response = None;
        self.selected_param_index = 0;
        self.selected_header_index = 0;
        self.body_scroll = 0;
        self.focused_panel = FocusedPanel::UrlBar;
        self.input_mode = InputMode::Editing;
        self.set_editing_field(EditingField::Url);
    }

    fn toggle_selected_param(&mut self) {
        if let Some(param) = self.current_request.query_params.get_mut(self.selected_param_index) {
            param.enabled = !param.enabled;
        }
    }

    fn delete_selected_param(&mut self) {
        if self.selected_param_index < self.current_request.query_params.len() {
            self.current_request.query_params.remove(self.selected_param_index);
            // Adjust selection if needed
            if self.selected_param_index >= self.current_request.query_params.len()
                && self.selected_param_index > 0
            {
                self.selected_param_index -= 1;
            }
        }
    }

    fn toggle_selected_header(&mut self) {
        if let Some(header) = self.current_request.headers.get_mut(self.selected_header_index) {
            header.enabled = !header.enabled;
        }
    }

    fn delete_selected_header(&mut self) {
        if self.selected_header_index < self.current_request.headers.len() {
            self.current_request.headers.remove(self.selected_header_index);
            // Adjust selection if needed
            if self.selected_header_index >= self.current_request.headers.len()
                && self.selected_header_index > 0
            {
                self.selected_header_index -= 1;
            }
        }
    }

    fn reload_environments(&mut self) {
        let path = &self.config.environments_file;
        let exists = path.exists();
        // Remember current active environment name
        let current_env_name = self.environments.active_name().to_string();

        match EnvironmentManager::load(path) {
            Ok(mut env_manager) => {
                let count = env_manager.environments.len();
                let names: Vec<_> = env_manager.environments.iter().map(|e| e.name.clone()).collect();

                // Try to restore the previously active environment by name
                if let Some(idx) = env_manager.environments.iter().position(|e| e.name == current_env_name) {
                    env_manager.set_active(idx);
                }

                self.environments = env_manager;
                self.status_message = Some(format!(
                    "Loaded {} [{}] from {:?} (exists={})",
                    count,
                    names.join(", "),
                    path,
                    exists
                ));
            }
            Err(e) => {
                self.error_message = Some(format!("Failed: {:?} (exists={}): {}", path, exists, e));
            }
        }
    }

    fn copy_to_clipboard(content: &str) -> Result<(), std::io::Error> {
        use std::io::Write;

        #[cfg(target_os = "macos")]
        {
            let mut child = std::process::Command::new("pbcopy")
                .stdin(std::process::Stdio::piped())
                .spawn()?;
            if let Some(stdin) = child.stdin.as_mut() {
                stdin.write_all(content.as_bytes())?;
            }
            child.wait()?;
            return Ok(());
        }

        #[cfg(target_os = "linux")]
        {
            // Try wl-copy first (Wayland), then fall back to xclip (X11)
            let wayland_result = std::process::Command::new("wl-copy")
                .stdin(std::process::Stdio::piped())
                .spawn()
                .and_then(|mut child| {
                    if let Some(stdin) = child.stdin.as_mut() {
                        stdin.write_all(content.as_bytes())?;
                    }
                    child.wait()
                });

            if wayland_result.is_ok() {
                return Ok(());
            }

            // Fall back to xclip for X11
            let mut child = std::process::Command::new("xclip")
                .args(["-selection", "clipboard"])
                .stdin(std::process::Stdio::piped())
                .spawn()?;
            if let Some(stdin) = child.stdin.as_mut() {
                stdin.write_all(content.as_bytes())?;
            }
            child.wait()?;
            return Ok(());
        }

        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "Clipboard not supported on this platform",
        ))
    }

    fn copy_as_curl(&mut self) {
        let curl_cmd = self.request_to_curl();

        match Self::copy_to_clipboard(&curl_cmd) {
            Ok(_) => self.status_message = Some("Copied curl command to clipboard".to_string()),
            Err(e) => self.error_message = Some(format!("Failed to copy: {}", e)),
        }
    }

    fn copy_response(&mut self) {
        let Some(response) = &self.response else {
            self.error_message = Some("No response to copy".to_string());
            return;
        };

        match Self::copy_to_clipboard(&response.body) {
            Ok(_) => self.status_message = Some("Copied response to clipboard".to_string()),
            Err(e) => self.error_message = Some(format!("Failed to copy: {}", e)),
        }
    }

    fn format_body_json(&mut self) {
        let body = &self.current_request.body;
        if body.trim().is_empty() {
            return;
        }

        match serde_json::from_str::<serde_json::Value>(body) {
            Ok(parsed) => {
                match serde_json::to_string_pretty(&parsed) {
                    Ok(formatted) => {
                        self.current_request.body = formatted;
                        self.status_message = Some("Formatted JSON".to_string());
                    }
                    Err(e) => {
                        self.error_message = Some(format!("Failed to format: {}", e));
                    }
                }
            }
            Err(e) => {
                self.error_message = Some(format!("Invalid JSON: {}", e));
            }
        }
    }

    fn request_to_curl(&self) -> String {
        let mut parts = vec!["curl".to_string()];

        // Method (if not GET)
        let method = self.current_request.method.as_str();
        if method != "GET" {
            parts.push(format!("-X {}", method));
        }

        // URL with interpolation
        let url = self.environments.interpolate(&self.current_request.url);

        // Headers
        for header in &self.current_request.headers {
            if header.enabled && !header.key.is_empty() {
                let key = self.environments.interpolate(&header.key);
                let value = self.environments.interpolate(&header.value);
                parts.push(format!("-H '{}: {}'", key, value));
            }
        }

        // Auth
        match self.current_request.auth.auth_type {
            crate::storage::AuthType::Bearer => {
                let token = self.environments.interpolate(&self.current_request.auth.bearer_token);
                parts.push(format!("-H 'Authorization: Bearer {}'", token));
            }
            crate::storage::AuthType::Basic => {
                let user = self.environments.interpolate(&self.current_request.auth.basic_username);
                let pass = self.environments.interpolate(&self.current_request.auth.basic_password);
                parts.push(format!("-u '{}:{}'", user, pass));
            }
            crate::storage::AuthType::ApiKey => {
                let name = self.environments.interpolate(&self.current_request.auth.api_key_name);
                let value = self.environments.interpolate(&self.current_request.auth.api_key_value);
                if self.current_request.auth.api_key_location == "header" {
                    parts.push(format!("-H '{}: {}'", name, value));
                }
                // Query params handled below with URL
            }
            crate::storage::AuthType::None => {}
        }

        // Body
        if !self.current_request.body.is_empty() {
            let body = self.environments.interpolate(&self.current_request.body);
            // Escape single quotes in body
            let escaped_body = body.replace("'", "'\\''");
            parts.push(format!("-d '{}'", escaped_body));
        }

        // Query params - build URL with params
        let mut full_url = url;
        let enabled_params: Vec<_> = self.current_request.query_params
            .iter()
            .filter(|p| p.enabled && !p.key.is_empty())
            .collect();

        if !enabled_params.is_empty() {
            let query_string: Vec<String> = enabled_params
                .iter()
                .map(|p| {
                    let key = self.environments.interpolate(&p.key);
                    let value = self.environments.interpolate(&p.value);
                    format!("{}={}", key, value)
                })
                .collect();

            if full_url.contains('?') {
                full_url = format!("{}&{}", full_url, query_string.join("&"));
            } else {
                full_url = format!("{}?{}", full_url, query_string.join("&"));
            }
        }

        // Add API key to URL if location is query
        if self.current_request.auth.auth_type == crate::storage::AuthType::ApiKey
            && self.current_request.auth.api_key_location == "query"
        {
            let name = self.environments.interpolate(&self.current_request.auth.api_key_name);
            let value = self.environments.interpolate(&self.current_request.auth.api_key_value);
            if full_url.contains('?') {
                full_url = format!("{}&{}={}", full_url, name, value);
            } else {
                full_url = format!("{}?{}={}", full_url, name, value);
            }
        }

        parts.push(format!("'{}'", full_url));

        parts.join(" ")
    }

    fn save_current_request(&mut self) {
        if let Some((collection_idx, request_id)) = &self.current_request_source {
            let request = self.current_request.clone();
            if let Some(collection) = self.collections.get_mut(*collection_idx) {
                if collection.update_request(&request_id, |r| {
                    r.name = request.name.clone();
                    r.method = request.method.clone();
                    r.url = request.url.clone();
                    r.headers = request.headers.clone();
                    r.query_params = request.query_params.clone();
                    r.body = request.body.clone();
                    r.auth = request.auth.clone();
                }) {
                    self.status_message = Some("Request saved".to_string());
                } else {
                    self.error_message = Some("Failed to save request".to_string());
                }
            }
        } else {
            // No source - this is a new request, prompt to save to collection
            self.error_message = Some("Use 'r' in Request List to create a new saved request".to_string());
        }
    }

    async fn send_request(&mut self) -> Result<()> {
        if self.current_request.url.is_empty() {
            self.error_message = Some("URL is required".to_string());
            return Ok(());
        }

        self.is_loading = true;
        self.status_message = Some("Sending request...".to_string());

        let env_manager = self.environments.clone();
        let interpolate = move |s: &str| env_manager.interpolate(s);

        match self
            .http_client
            .execute(&self.current_request, interpolate)
            .await
        {
            Ok(response) => {
                // Add to history
                let history_entry = HistoryEntry::new(
                    self.current_request.clone(),
                    Some(response.status),
                    response.duration_ms,
                );
                self.history.add(history_entry);

                self.status_message = Some(format!(
                    "{} {} - {}ms",
                    response.status, response.status_text, response.duration_ms
                ));
                self.response = Some(response);
                self.response_scroll = 0;
            }
            Err(e) => {
                // Add failed request to history
                let history_entry =
                    HistoryEntry::new(self.current_request.clone(), None, 0);
                self.history.add(history_entry);

                self.error_message = Some(format!("Request failed: {}", e));
                self.response = None;
            }
        }

        self.is_loading = false;
        Ok(())
    }

    pub fn set_error(&mut self, msg: String) {
        self.error_message = Some(msg);
    }

    /// Called periodically to process async tasks
    pub async fn tick(&mut self) -> Result<()> {
        // Placeholder for any background async operations
        Ok(())
    }

    /// Handle key input when a dialog is showing
    fn handle_dialog_input(&mut self, key: KeyEvent) -> Result<bool> {
        let Some(dialog_type) = self.dialog.dialog_type.clone() else {
            return Ok(false);
        };

        match &dialog_type {
            DialogType::ConfirmDelete { item_type, item_id, collection_index, .. } => {
                match key.code {
                    KeyCode::Char('y') | KeyCode::Char('Y') => {
                        self.execute_delete(item_type.clone(), item_id.clone(), *collection_index);
                        self.dialog = DialogState::default();
                    }
                    KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                        self.dialog = DialogState::default();
                    }
                    _ => {}
                }
            }
            _ => {
                // Input dialog handling
                match key.code {
                    KeyCode::Esc => {
                        self.dialog = DialogState::default();
                    }
                    KeyCode::Enter => {
                        if !self.dialog.input_buffer.trim().is_empty() {
                            self.execute_dialog_action();
                        }
                    }
                    KeyCode::Backspace => {
                        self.dialog.input_buffer.pop();
                    }
                    KeyCode::Char(c) => {
                        self.dialog.input_buffer.push(c);
                    }
                    _ => {}
                }
            }
        }
        Ok(false)
    }

    fn execute_dialog_action(&mut self) {
        let name = self.dialog.input_buffer.trim().to_string();
        let Some(dialog_type) = self.dialog.dialog_type.take() else {
            return;
        };

        match dialog_type {
            DialogType::CreateCollection => {
                let collection = Collection::new(&name);
                self.collections.push(collection);
                self.status_message = Some(format!("Created collection: {}", name));
            }
            DialogType::CreateFolder { parent_collection, parent_folder_id } => {
                if let Some(collection) = self.collections.get_mut(parent_collection) {
                    collection.add_folder_to(&name, parent_folder_id.as_deref());
                    self.status_message = Some(format!("Created folder: {}", name));
                }
            }
            DialogType::CreateRequest { parent_collection, parent_folder_id } => {
                if let Some(collection) = self.collections.get_mut(parent_collection) {
                    let request = ApiRequest::new(&name);
                    collection.add_request_to(request, parent_folder_id.as_deref());
                    self.status_message = Some(format!("Created request: {}", name));
                }
            }
            DialogType::RenameItem { item_type, item_id, collection_index } => {
                match item_type {
                    ItemType::Collection => {
                        if let Some(collection) = self.collections.get_mut(collection_index) {
                            collection.rename(&name);
                            self.status_message = Some(format!("Renamed to: {}", name));
                        }
                    }
                    ItemType::Folder | ItemType::Request => {
                        if let Some(collection) = self.collections.get_mut(collection_index) {
                            collection.rename_item(&item_id, &name);
                            self.status_message = Some(format!("Renamed to: {}", name));
                        }
                    }
                }
            }
            DialogType::ConfirmDelete { .. } => unreachable!(),
        }

        self.dialog = DialogState::default();
    }

    fn execute_delete(&mut self, item_type: ItemType, item_id: String, collection_index: usize) {
        match item_type {
            ItemType::Collection => {
                if collection_index < self.collections.len() {
                    let name = self.collections[collection_index].name.clone();
                    let source_path = self.collections[collection_index].source_path.clone();
                    let id = self.collections[collection_index].id.clone();
                    self.collections.remove(collection_index);

                    // Adjust selected_collection if needed
                    if self.selected_collection >= self.collections.len() && !self.collections.is_empty() {
                        self.selected_collection = self.collections.len() - 1;
                    }
                    self.selected_item = usize::MAX; // Select header of remaining collection
                    self.status_message = Some(format!("Deleted collection: {}", name));

                    // Delete the collection file from disk
                    // Use source_path if available (for files with non-standard names),
                    // otherwise fall back to id-based path
                    let path = source_path.unwrap_or_else(|| {
                        self.config.collections_dir.join(format!("{}.json", id))
                    });
                    let _ = std::fs::remove_file(path);
                }
            }
            ItemType::Folder | ItemType::Request => {
                if let Some(collection) = self.collections.get_mut(collection_index) {
                    collection.delete_item(&item_id);
                    // Adjust selected_item if needed (but not if header is selected)
                    if self.selected_item != usize::MAX {
                        let max = self.get_visible_items_count().saturating_sub(1);
                        if self.selected_item > max {
                            self.selected_item = if max == usize::MAX { usize::MAX } else { max };
                        }
                    }
                    self.status_message = Some("Item deleted".to_string());
                }
            }
        }
    }

    fn start_create_collection(&mut self) {
        self.dialog = DialogState {
            dialog_type: Some(DialogType::CreateCollection),
            input_buffer: String::new(),
        };
    }

    fn start_create_folder(&mut self) {
        if self.collections.is_empty() {
            self.error_message = Some("Create a collection first".to_string());
            return;
        }
        let parent_folder_id = self.get_selected_folder_id();
        self.dialog = DialogState {
            dialog_type: Some(DialogType::CreateFolder {
                parent_collection: self.selected_collection,
                parent_folder_id,
            }),
            input_buffer: String::new(),
        };
    }

    fn start_create_request(&mut self) {
        if self.collections.is_empty() {
            self.error_message = Some("Create a collection first".to_string());
            return;
        }
        let parent_folder_id = self.get_selected_folder_id();
        self.dialog = DialogState {
            dialog_type: Some(DialogType::CreateRequest {
                parent_collection: self.selected_collection,
                parent_folder_id,
            }),
            input_buffer: String::new(),
        };
    }

    fn start_rename_item(&mut self) {
        if let Some((item_type, item_id, current_name)) = self.get_selected_item_info() {
            self.dialog = DialogState {
                dialog_type: Some(DialogType::RenameItem {
                    item_type,
                    item_id,
                    collection_index: self.selected_collection,
                }),
                input_buffer: current_name,
            };
        }
    }

    fn start_delete_item(&mut self) {
        if let Some((item_type, item_id, item_name)) = self.get_selected_item_info() {
            self.dialog = DialogState {
                dialog_type: Some(DialogType::ConfirmDelete {
                    item_type,
                    item_id,
                    item_name,
                    collection_index: self.selected_collection,
                }),
                input_buffer: String::new(),
            };
        }
    }

    fn start_delete_collection(&mut self) {
        if let Some(collection) = self.collections.get(self.selected_collection) {
            self.dialog = DialogState {
                dialog_type: Some(DialogType::ConfirmDelete {
                    item_type: ItemType::Collection,
                    item_id: collection.id.clone(),
                    item_name: collection.name.clone(),
                    collection_index: self.selected_collection,
                }),
                input_buffer: String::new(),
            };
        }
    }

    fn duplicate_selected_request(&mut self) {
        if self.collections.is_empty() {
            return;
        }

        let collection = match self.collections.get(self.selected_collection) {
            Some(c) => c,
            None => return,
        };

        let flattened = collection.flatten();
        let Some((_, item)) = flattened.get(self.selected_item) else {
            return;
        };

        // Only duplicate requests, not folders
        let CollectionItem::Request(original) = item else {
            self.status_message = Some("Can only duplicate requests".to_string());
            return;
        };

        // Create a copy with new ID and modified name
        let mut new_request = original.clone();
        new_request.id = uuid::Uuid::new_v4().to_string();
        new_request.name = format!("{} (copy)", original.name);

        // Find the parent folder of the original request (if any)
        let parent_folder_id = self.find_parent_folder_id(&original.id);

        // Add to the collection
        let collection = self.collections.get_mut(self.selected_collection).unwrap();
        if let Some(folder_id) = parent_folder_id {
            collection.add_request_to(new_request, Some(&folder_id));
        } else {
            collection.add_request(new_request);
        }

        self.status_message = Some("Request duplicated".to_string());
    }

    fn start_move_item(&mut self) {
        if let Some((item_type, item_id, item_name)) = self.get_selected_item_info() {
            // Don't allow moving collections
            if item_type == ItemType::Collection {
                self.status_message = Some("Cannot move collections".to_string());
                return;
            }
            self.pending_move = Some(PendingMove {
                item_id,
                item_type,
                item_name: item_name.clone(),
                source_collection_index: self.selected_collection,
            });
            self.status_message = Some(format!("Moving: {} - navigate to destination, Enter to move, Esc to cancel", item_name));
        }
    }

    fn execute_pending_move(&mut self) {
        let pending = match self.pending_move.take() {
            Some(p) => p,
            None => return,
        };

        // Determine the destination
        let dest_collection_index = self.selected_collection;
        let dest_folder_id = self.get_destination_folder_id();

        // Check if trying to move to the same location
        let source_folder_id = {
            let source_collection = match self.collections.get(pending.source_collection_index) {
                Some(c) => c,
                None => {
                    self.error_message = Some("Source collection not found".to_string());
                    return;
                }
            };
            Self::find_parent_folder_recursive(&source_collection.items, &pending.item_id)
        };

        // If same collection and same folder, it's a no-op
        if pending.source_collection_index == dest_collection_index && source_folder_id == dest_folder_id {
            self.status_message = Some("Item already in this location".to_string());
            return;
        }

        // Extract item from source collection
        let item = {
            let source_collection = match self.collections.get_mut(pending.source_collection_index) {
                Some(c) => c,
                None => {
                    self.error_message = Some("Source collection not found".to_string());
                    return;
                }
            };
            match source_collection.extract_item(&pending.item_id) {
                Some(item) => item,
                None => {
                    self.error_message = Some("Item not found in source collection".to_string());
                    return;
                }
            }
        };

        // Insert into destination
        let dest_collection = match self.collections.get_mut(dest_collection_index) {
            Some(c) => c,
            None => {
                self.error_message = Some("Destination collection not found".to_string());
                return;
            }
        };

        if dest_collection.insert_item(item, dest_folder_id.as_deref()) {
            self.status_message = Some(format!("Moved: {}", pending.item_name));
            // Save both collections
            let _ = self.save();
        } else {
            self.error_message = Some("Failed to move item to destination".to_string());
        }
    }

    /// Get the folder ID to insert into based on current selection
    fn get_destination_folder_id(&self) -> Option<String> {
        let collection = self.collections.get(self.selected_collection)?;

        // If collection header is selected (usize::MAX), insert at root
        if self.is_collection_header_selected() {
            return None;
        }

        let flattened = collection.flatten();
        let (_, item) = flattened.get(self.selected_item)?;

        // If selected item is a folder, move into it
        if item.is_folder() {
            Some(item.id().to_string())
        } else {
            // If selected item is a request, move to its parent folder (or root)
            Self::find_parent_folder_recursive(&collection.items, item.id())
        }
    }

    fn find_parent_folder_id(&self, item_id: &str) -> Option<String> {
        let collection = self.collections.get(self.selected_collection)?;
        Self::find_parent_folder_recursive(&collection.items, item_id)
    }

    fn find_parent_folder_recursive(items: &[CollectionItem], item_id: &str) -> Option<String> {
        for item in items {
            if let CollectionItem::Folder { id, items: folder_items, .. } = item {
                // Check if the item is directly in this folder
                for child in folder_items {
                    if child.id() == item_id {
                        return Some(id.clone());
                    }
                }
                // Recursively check subfolders
                if let Some(parent_id) = Self::find_parent_folder_recursive(folder_items, item_id) {
                    return Some(parent_id);
                }
            }
        }
        None
    }

    fn toggle_expand_collapse(&mut self) {
        if self.collections.is_empty() {
            return;
        }

        let collection = match self.collections.get_mut(self.selected_collection) {
            Some(c) => c,
            None => return,
        };

        // If collection is collapsed, toggle it open
        if !collection.expanded {
            collection.expanded = true;
            return;
        }

        let flattened = collection.flatten();

        // If no items or selected_item is out of range, toggle the collection itself
        if flattened.is_empty() || self.selected_item >= flattened.len() {
            collection.expanded = !collection.expanded;
            return;
        }

        // Check if selected item is a folder
        if let Some((_, item)) = flattened.get(self.selected_item) {
            if let CollectionItem::Folder { id, .. } = item {
                let folder_id = id.clone();
                // Need to toggle the folder's expanded state
                Self::toggle_folder_expanded(&mut collection.items, &folder_id);
            } else {
                // Selected item is a request - find parent folder and collapse that
                let item_id = item.id().to_string();
                if let Some(parent_folder_id) = Self::find_parent_folder_recursive(&collection.items, &item_id) {
                    Self::toggle_folder_expanded(&mut collection.items, &parent_folder_id);
                } else {
                    // No parent folder, request is at root level - toggle the collection
                    collection.expanded = !collection.expanded;
                }
            }
        }
    }

    fn toggle_folder_expanded(items: &mut [CollectionItem], folder_id: &str) -> bool {
        for item in items {
            if let CollectionItem::Folder { id, expanded, items: sub_items, .. } = item {
                if id == folder_id {
                    *expanded = !*expanded;
                    return true;
                }
                if Self::toggle_folder_expanded(sub_items, folder_id) {
                    return true;
                }
            }
        }
        false
    }

    /// Get the folder ID if current selection is a folder, None otherwise
    fn get_selected_folder_id(&self) -> Option<String> {
        let collection = self.collections.get(self.selected_collection)?;
        let flattened = collection.flatten();
        let (_, item) = flattened.get(self.selected_item)?;
        if item.is_folder() {
            Some(item.id().to_string())
        } else {
            None
        }
    }

    /// Get info about the currently selected item
    fn get_selected_item_info(&self) -> Option<(ItemType, String, String)> {
        if self.collections.is_empty() {
            return None;
        }

        let collection = self.collections.get(self.selected_collection)?;
        let flattened = collection.flatten();

        if let Some((_, item)) = flattened.get(self.selected_item) {
            let item_type = if item.is_folder() {
                ItemType::Folder
            } else {
                ItemType::Request
            };
            Some((item_type, item.id().to_string(), item.name().to_string()))
        } else {
            // No items in collection - allow operations on the collection itself
            Some((ItemType::Collection, collection.id.clone(), collection.name.clone()))
        }
    }

    /// Save current state to disk
    pub fn save(&self) -> Result<()> {
        // Save history
        self.history.save(&self.config.history_file)?;

        // Save environments
        self.environments.save(&self.config.environments_file)?;

        // Save collections
        for collection in &self.collections {
            let path = self
                .config
                .collections_dir
                .join(format!("{}.json", collection.id));
            collection.save(&path)?;
        }

        Ok(())
    }

    /// Get contextual help based on current state
    pub fn get_help_content(&self) -> Vec<(&'static str, &'static str)> {
        let mut help = Vec::new();

        // Global commands (always shown)
        help.push(("", "── Global ──"));
        help.push(("1-4", "Jump to panel"));
        help.push(("Tab", "Next panel"));
        help.push(("Shift+Tab", "Previous panel"));
        help.push(("W / Ctrl+s", "Save request to collection"));
        help.push(("y", "Copy as curl to clipboard"));
        help.push(("?", "Toggle help"));
        help.push(("q / Ctrl+c", "Quit"));

        match self.input_mode {
            InputMode::Editing => {
                help.push(("", "── Editing Mode ──"));
                help.push(("Esc", "Exit edit mode"));
                help.push(("Tab", "Next field"));
                help.push(("Enter", "Next field / New line (body)"));
                help.push(("Backspace", "Delete character"));
                help.push(("", "Just start typing to enter text"));
            }
            InputMode::Normal => {
                match self.focused_panel {
                    FocusedPanel::RequestList => {
                        help.push(("", "── Request List ──"));
                        help.push(("j / ↓", "Move down"));
                        help.push(("k / ↑", "Move up"));
                        help.push(("Enter", "Load request"));
                        help.push(("Space", "Toggle expand/collapse"));
                        help.push(("H", "Toggle history view"));
                        help.push(("n", "New request (in editor)"));
                        help.push(("", "── Collection CRUD ──"));
                        help.push(("C", "Create collection"));
                        help.push(("F", "Create folder"));
                        help.push(("r", "Create request"));
                        help.push(("p", "Duplicate request"));
                        help.push(("m", "Move item (cut/paste)"));
                        help.push(("R", "Rename selected"));
                        help.push(("d", "Delete selected item"));
                        help.push(("D", "Delete collection"));
                    }
                    FocusedPanel::UrlBar => {
                        help.push(("", "── URL Bar ──"));
                        help.push(("Enter / i", "Edit URL"));
                        help.push(("m", "Cycle HTTP method (GET/POST/...)"));
                        help.push(("s", "Send request"));
                        help.push(("e / E", "Switch / Reload environments"));
                        help.push(("n", "New request"));
                    }
                    FocusedPanel::RequestEditor => {
                        help.push(("", "── Request Editor ──"));
                        help.push(("h / ←", "Previous tab"));
                        help.push(("l / →", "Next tab"));
                        help.push(("Enter", "Start editing current tab"));
                        help.push(("m", "Cycle HTTP method (GET/POST/...)"));
                        help.push(("s", "Send request"));
                        help.push(("e / E", "Switch / Reload environments"));
                        help.push(("n", "New request"));

                        // Tab-specific hints
                        match self.request_tab {
                            RequestTab::Headers => {
                                help.push(("", "── Headers Tab ──"));
                                help.push(("j / ↓", "Select next header"));
                                help.push(("k / ↑", "Select previous header"));
                                help.push(("t", "Toggle header on/off"));
                                help.push(("x", "Delete selected header"));
                                help.push(("Enter", "Edit headers (Tab to next field)"));
                            }
                            RequestTab::Body => {
                                help.push(("", "── Body Tab ──"));
                                help.push(("Enter", "Edit request body"));
                                help.push(("f", "Format JSON"));
                            }
                            RequestTab::Auth => {
                                help.push(("", "── Auth Tab ──"));
                                help.push(("a", "Cycle auth type first"));
                                help.push(("Enter", "Edit auth credentials"));
                                help.push(("", "Types: None → Bearer → Basic → API Key"));
                            }
                            RequestTab::Params => {
                                help.push(("", "── Params Tab ──"));
                                help.push(("j / ↓", "Select next param"));
                                help.push(("k / ↑", "Select previous param"));
                                help.push(("t", "Toggle param on/off"));
                                help.push(("x", "Delete selected param"));
                                help.push(("Enter", "Edit params (Tab to next field)"));
                            }
                        }
                    }
                    FocusedPanel::ResponseView => {
                        help.push(("", "── Response View ──"));
                        help.push(("j / ↓", "Scroll down"));
                        help.push(("k / ↑", "Scroll up"));
                        help.push(("c", "Copy response to clipboard"));
                        help.push(("s", "Send request again"));
                    }
                }
            }
        }

        help
    }
}

impl Drop for App {
    fn drop(&mut self) {
        // Try to save on exit
        let _ = self.save();
    }
}
