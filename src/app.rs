use crate::config::Config;
use crate::http::{HttpClient, HttpResponse};
use crate::storage::{
    ApiRequest, Collection, CollectionItem, EnvironmentManager, HistoryEntry, HistoryManager,
    HttpMethod,
};
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
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

    // Help popup
    pub show_help: bool,

    // Dialog state
    pub dialog: DialogState,

    // Layout areas for mouse click detection
    pub layout_areas: LayoutAreas,
}

/// Stores the layout areas for mouse click detection
#[derive(Debug, Clone, Default)]
pub struct LayoutAreas {
    pub request_list: Option<(u16, u16, u16, u16)>,  // x, y, width, height
    pub url_bar: Option<(u16, u16, u16, u16)>,
    pub request_editor: Option<(u16, u16, u16, u16)>,
    pub response_view: Option<(u16, u16, u16, u16)>,
    pub tabs: Option<(u16, u16, u16, u16)>,
    pub tab_positions: Vec<(u16, u16, RequestTab)>,  // x, width, tab
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
            selected_collection: 0,
            selected_item: 0,
            selected_history: 0,
            show_history: false,
            current_request: ApiRequest::default(),
            current_request_source: None,
            response: None,
            is_loading: false,
            status_message: None,
            error_message: None,
            response_scroll: 0,
            show_help: false,
            dialog: DialogState::default(),
            layout_areas: LayoutAreas::default(),
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
                            // Clicked on collection header - select it and toggle expand
                            self.selected_collection = col_idx;
                            self.selected_item = 0;
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
                return;
            }
        }

        if let Some((px, py, pw, ph)) = self.layout_areas.request_editor {
            if x >= px && x < px + pw && y >= py && y < py + ph {
                self.focused_panel = FocusedPanel::RequestEditor;
                
                // Check if a tab was clicked
                for (tab_x, tab_width, tab) in &self.layout_areas.tab_positions {
                    if x >= *tab_x && x < tab_x + tab_width {
                        self.request_tab = *tab;
                        self.input_mode = InputMode::Normal;
                        self.editing_field = None;
                        return;
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

    async fn handle_normal_mode(&mut self, key: KeyEvent) -> Result<bool> {
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
            KeyCode::Char('e') | KeyCode::Char('E') => {
                self.environments.next();
                self.status_message = Some(format!(
                    "Switched to environment: {}",
                    self.environments.active_name()
                ));
            }

            // Edit current field
            KeyCode::Char('i') => {
                if self.focused_panel == FocusedPanel::UrlBar {
                    self.input_mode = InputMode::Editing;
                    self.editing_field = Some(EditingField::Url);
                } else if self.focused_panel == FocusedPanel::RequestEditor {
                    self.enter_edit_mode();
                }
            }

            // Cycle HTTP method
            KeyCode::Char('m') | KeyCode::Char('M') => {
                if self.focused_panel == FocusedPanel::UrlBar
                    || self.focused_panel == FocusedPanel::RequestEditor
                {
                    self.current_request.method = self.current_request.method.next();
                }
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
                // For body, add newline
                // For other fields, move to next field
                if matches!(self.editing_field, Some(EditingField::Body)) {
                    self.current_request.body.push('\n');
                } else {
                    self.next_editing_field();
                }
            }
            KeyCode::Backspace => {
                self.handle_backspace();
            }
            KeyCode::Char(c) => {
                self.handle_char_input(c);
            }
            _ => {}
        }
        Ok(false)
    }

    fn handle_backspace(&mut self) {
        if let Some(field) = &self.editing_field {
            match field {
                EditingField::Url => {
                    self.current_request.url.pop();
                }
                EditingField::Body => {
                    self.current_request.body.pop();
                }
                EditingField::HeaderKey(i) => {
                    if let Some(h) = self.current_request.headers.get_mut(*i) {
                        h.key.pop();
                    }
                }
                EditingField::HeaderValue(i) => {
                    if let Some(h) = self.current_request.headers.get_mut(*i) {
                        h.value.pop();
                    }
                }
                EditingField::ParamKey(i) => {
                    if let Some(p) = self.current_request.query_params.get_mut(*i) {
                        p.key.pop();
                    }
                }
                EditingField::ParamValue(i) => {
                    if let Some(p) = self.current_request.query_params.get_mut(*i) {
                        p.value.pop();
                    }
                }
                EditingField::AuthBearerToken => {
                    self.current_request.auth.bearer_token.pop();
                }
                EditingField::AuthBasicUsername => {
                    self.current_request.auth.basic_username.pop();
                }
                EditingField::AuthBasicPassword => {
                    self.current_request.auth.basic_password.pop();
                }
                EditingField::AuthApiKeyName => {
                    self.current_request.auth.api_key_name.pop();
                }
                EditingField::AuthApiKeyValue => {
                    self.current_request.auth.api_key_value.pop();
                }
            }
        }
    }

    fn handle_char_input(&mut self, c: char) {
        if let Some(field) = &self.editing_field {
            match field {
                EditingField::Url => {
                    self.current_request.url.push(c);
                }
                EditingField::Body => {
                    self.current_request.body.push(c);
                }
                EditingField::HeaderKey(i) => {
                    if let Some(h) = self.current_request.headers.get_mut(*i) {
                        h.key.push(c);
                    }
                }
                EditingField::HeaderValue(i) => {
                    if let Some(h) = self.current_request.headers.get_mut(*i) {
                        h.value.push(c);
                    }
                }
                EditingField::ParamKey(i) => {
                    if let Some(p) = self.current_request.query_params.get_mut(*i) {
                        p.key.push(c);
                    }
                }
                EditingField::ParamValue(i) => {
                    if let Some(p) = self.current_request.query_params.get_mut(*i) {
                        p.value.push(c);
                    }
                }
                EditingField::AuthBearerToken => {
                    self.current_request.auth.bearer_token.push(c);
                }
                EditingField::AuthBasicUsername => {
                    self.current_request.auth.basic_username.push(c);
                }
                EditingField::AuthBasicPassword => {
                    self.current_request.auth.basic_password.push(c);
                }
                EditingField::AuthApiKeyName => {
                    self.current_request.auth.api_key_name.push(c);
                }
                EditingField::AuthApiKeyValue => {
                    self.current_request.auth.api_key_value.push(c);
                }
            }
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
            _ => {}
        }
    }

    fn navigate_collection_up(&mut self) {
        if self.collections.is_empty() {
            return;
        }

        if self.selected_item > 0 {
            // Move up within current collection
            self.selected_item -= 1;
        } else if self.selected_collection > 0 {
            // Move to previous collection
            self.selected_collection -= 1;
            // Select last item in previous collection (or 0 if collapsed/empty)
            if let Some(col) = self.collections.get(self.selected_collection) {
                if col.expanded {
                    let count = col.flatten().len();
                    self.selected_item = count.saturating_sub(1);
                } else {
                    self.selected_item = 0;
                }
            }
        }
        // else: already at top of first collection, do nothing
    }

    fn navigate_collection_down(&mut self) {
        if self.collections.is_empty() {
            return;
        }

        let current_max = self.get_visible_items_count().saturating_sub(1);

        if self.selected_item < current_max {
            // Move down within current collection
            self.selected_item += 1;
        } else if self.selected_collection < self.collections.len() - 1 {
            // Move to next collection
            self.selected_collection += 1;
            self.selected_item = 0;
        }
        // else: already at bottom of last collection, do nothing
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
        match self.focused_panel {
            FocusedPanel::RequestList => {
                if self.show_history {
                    // Load request from history
                    if let Some(entry) = self.history.entries.get(self.selected_history) {
                        self.current_request = entry.request.clone();
                        self.response = None;
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
                self.editing_field = Some(EditingField::Url);
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
        self.editing_field = Some(self.get_default_editing_field());
    }

    /// Get the default editing field for the current tab
    fn get_default_editing_field(&mut self) -> EditingField {
        match self.request_tab {
            RequestTab::Headers => {
                if self.current_request.headers.is_empty() {
                    // Add a new header if none exist
                    self.current_request.headers.push(crate::storage::KeyValue::new("", ""));
                }
                EditingField::HeaderKey(0)
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
                }
                EditingField::ParamKey(0)
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
        self.editing_field = Some(next);
    }

    fn load_selected_request(&mut self) {
        if let Some(collection) = self.collections.get(self.selected_collection) {
            let flattened = collection.flatten();
            if let Some((_, item)) = flattened.get(self.selected_item) {
                if let CollectionItem::Request(req) = item {
                    self.current_request = req.clone();
                    self.current_request_source = Some((self.selected_collection, req.id.clone()));
                    self.response = None;
                }
            }
        }
    }

    fn get_visible_items_count(&self) -> usize {
        self.collections
            .get(self.selected_collection)
            .map(|c| c.flatten().len())
            .unwrap_or(0)
    }

    fn new_request(&mut self) {
        self.current_request = ApiRequest::default();
        self.current_request_source = None;
        self.response = None;
        self.focused_panel = FocusedPanel::UrlBar;
        self.input_mode = InputMode::Editing;
        self.editing_field = Some(EditingField::Url);
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
                    let id = self.collections[collection_index].id.clone();
                    self.collections.remove(collection_index);

                    // Adjust selected_collection if needed
                    if self.selected_collection >= self.collections.len() && !self.collections.is_empty() {
                        self.selected_collection = self.collections.len() - 1;
                    }
                    self.selected_item = 0;
                    self.status_message = Some(format!("Deleted collection: {}", name));

                    // Delete the collection file from disk
                    let path = self.config.collections_dir.join(format!("{}.json", id));
                    let _ = std::fs::remove_file(path);
                }
            }
            ItemType::Folder | ItemType::Request => {
                if let Some(collection) = self.collections.get_mut(collection_index) {
                    collection.delete_item(&item_id);
                    // Adjust selected_item if needed
                    let max = self.get_visible_items_count().saturating_sub(1);
                    if self.selected_item > max {
                        self.selected_item = max;
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
                        help.push(("H", "Toggle history view"));
                        help.push(("n", "New request (in editor)"));
                        help.push(("", "── Collection CRUD ──"));
                        help.push(("C", "Create collection"));
                        help.push(("F", "Create folder"));
                        help.push(("r", "Create request"));
                        help.push(("R", "Rename selected"));
                        help.push(("d", "Delete selected"));
                    }
                    FocusedPanel::UrlBar => {
                        help.push(("", "── URL Bar ──"));
                        help.push(("Enter / i", "Edit URL"));
                        help.push(("m", "Cycle HTTP method (GET/POST/...)"));
                        help.push(("s", "Send request"));
                        help.push(("e", "Switch environment"));
                        help.push(("n", "New request"));
                    }
                    FocusedPanel::RequestEditor => {
                        help.push(("", "── Request Editor ──"));
                        help.push(("h / ←", "Previous tab"));
                        help.push(("l / →", "Next tab"));
                        help.push(("Enter", "Start editing current tab"));
                        help.push(("m", "Cycle HTTP method (GET/POST/...)"));
                        help.push(("s", "Send request"));
                        help.push(("e", "Switch environment"));
                        help.push(("n", "New request"));

                        // Tab-specific hints
                        match self.request_tab {
                            RequestTab::Headers => {
                                help.push(("", "── Headers Tab ──"));
                                help.push(("Enter", "Edit headers (Tab to next field)"));
                            }
                            RequestTab::Body => {
                                help.push(("", "── Body Tab ──"));
                                help.push(("Enter", "Edit request body"));
                            }
                            RequestTab::Auth => {
                                help.push(("", "── Auth Tab ──"));
                                help.push(("a", "Cycle auth type first"));
                                help.push(("Enter", "Edit auth credentials"));
                                help.push(("", "Types: None → Bearer → Basic → API Key"));
                            }
                            RequestTab::Params => {
                                help.push(("", "── Params Tab ──"));
                                help.push(("Enter", "Edit params (Tab to next field)"));
                            }
                        }
                    }
                    FocusedPanel::ResponseView => {
                        help.push(("", "── Response View ──"));
                        help.push(("j / ↓", "Scroll down"));
                        help.push(("k / ↑", "Scroll up"));
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
