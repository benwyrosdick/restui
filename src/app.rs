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
    RequestEditor,
    ResponseView,
}

impl FocusedPanel {
    pub fn next(&self) -> Self {
        match self {
            FocusedPanel::RequestList => FocusedPanel::RequestEditor,
            FocusedPanel::RequestEditor => FocusedPanel::ResponseView,
            FocusedPanel::ResponseView => FocusedPanel::RequestList,
        }
    }

    pub fn prev(&self) -> Self {
        match self {
            FocusedPanel::RequestList => FocusedPanel::ResponseView,
            FocusedPanel::RequestEditor => FocusedPanel::RequestList,
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

    // Response state
    pub response: Option<HttpResponse>,
    pub is_loading: bool,

    // Status/error message
    pub status_message: Option<String>,
    pub error_message: Option<String>,

    // Response scroll
    pub response_scroll: u16,
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
            response: None,
            is_loading: false,
            status_message: None,
            error_message: None,
            response_scroll: 0,
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

        // Global shortcuts
        match (key.code, key.modifiers) {
            (KeyCode::Char('c'), KeyModifiers::CONTROL) | (KeyCode::Char('q'), _)
                if self.input_mode == InputMode::Normal =>
            {
                return Ok(true);
            }
            _ => {}
        }

        // Mode-specific handling
        match self.input_mode {
            InputMode::Normal => self.handle_normal_mode(key).await,
            InputMode::Editing => self.handle_editing_mode(key),
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
                if self.focused_panel == FocusedPanel::RequestEditor {
                    self.enter_edit_mode();
                }
            }

            // Cycle HTTP method
            KeyCode::Char('m') | KeyCode::Char('M') => {
                if self.focused_panel == FocusedPanel::RequestEditor {
                    self.current_request.method = self.current_request.method.next();
                }
            }

            // Help (placeholder)
            KeyCode::Char('?') => {
                self.status_message = Some(
                    "Tab:switch panels | Enter:select | i:edit | s:send | n:new | e:env | q:quit"
                        .to_string(),
                );
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
            KeyCode::Enter => {
                // Exit editing mode on Enter (except for body which is multiline)
                if !matches!(self.editing_field, Some(EditingField::Body)) {
                    self.input_mode = InputMode::Normal;
                    self.editing_field = None;
                } else {
                    // Add newline to body
                    self.current_request.body.push('\n');
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
                    self.selected_item = self.selected_item.saturating_sub(1);
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
                    let max = self.get_visible_items_count().saturating_sub(1);
                    self.selected_item = (self.selected_item + 1).min(max);
                }
            }
            FocusedPanel::ResponseView => {
                self.response_scroll = self.response_scroll.saturating_add(1);
            }
            _ => {}
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
        match self.focused_panel {
            FocusedPanel::RequestList => {
                if self.show_history {
                    // Load request from history
                    if let Some(entry) = self.history.entries.get(self.selected_history) {
                        self.current_request = entry.request.clone();
                        self.response = None;
                        self.focused_panel = FocusedPanel::RequestEditor;
                    }
                } else {
                    // Load selected request from collection
                    self.load_selected_request();
                    self.focused_panel = FocusedPanel::RequestEditor;
                }
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
        // Default to URL editing
        self.editing_field = Some(EditingField::Url);
    }

    fn load_selected_request(&mut self) {
        if let Some(collection) = self.collections.get(self.selected_collection) {
            let flattened = collection.flatten();
            if let Some((_, item)) = flattened.get(self.selected_item) {
                if let CollectionItem::Request(req) = item {
                    self.current_request = req.clone();
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
        self.response = None;
        self.focused_panel = FocusedPanel::RequestEditor;
        self.enter_edit_mode();
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
}

impl Drop for App {
    fn drop(&mut self) {
        // Try to save on exit
        let _ = self.save();
    }
}
