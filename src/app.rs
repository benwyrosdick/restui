use crate::config::Config;
use crate::http::{HttpClient, HttpResponse};
use crate::storage::{
    ApiRequest, Collection, CollectionItem, EnvironmentManager, HistoryEntry, HistoryManager,
    HttpMethod, KeyValue, Settings,
};
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use graphql_parser::query::parse_query;
use ratatui::style::Color;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tokio::sync::oneshot;
use tokio::sync::oneshot::error::TryRecvError;

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

/// Response pane mode for search/filter
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ResponseMode {
    #[default]
    Normal,
    Search,
    Filter,
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
    EnvSharedKey(usize),
    EnvSharedValue(usize),
    EnvActiveKey(usize),
    EnvActiveValue(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnvPopupSection {
    Shared,
    Active,
}

#[derive(Debug, Clone)]
pub struct Theme {
    pub name: &'static str,
    pub accent: Color,
    pub background: Color,
    pub surface: Color,
    pub text: Color,
    pub muted: Color,
    pub selection_bg: Color,
    pub selection_fg: Color,
}

impl Theme {
    pub fn presets() -> Vec<Theme> {
        vec![
            Theme {
                name: "Classic",
                accent: Color::Cyan,
                background: Color::Rgb(0, 0, 0),
                surface: Color::Rgb(0, 0, 0),
                text: Color::White,
                muted: Color::DarkGray,
                selection_bg: Color::Cyan,
                selection_fg: Color::Black,
            },
            Theme {
                name: "Solarized",
                accent: Color::Rgb(38, 139, 210),
                background: Color::Rgb(0, 20, 25),
                surface: Color::Rgb(0, 28, 33),
                text: Color::Rgb(238, 232, 213),
                muted: Color::Rgb(147, 161, 161),
                selection_bg: Color::Rgb(38, 139, 210),
                selection_fg: Color::Rgb(238, 232, 213),
            },
            Theme {
                name: "Dracula",
                accent: Color::Rgb(189, 147, 249),
                background: Color::Rgb(20, 20, 28),
                surface: Color::Rgb(28, 28, 38),
                text: Color::Rgb(248, 248, 242),
                muted: Color::Rgb(98, 114, 164),
                selection_bg: Color::Rgb(68, 71, 90),
                selection_fg: Color::Rgb(248, 248, 242),
            },
            Theme {
                name: "Nord",
                accent: Color::Rgb(94, 129, 172),
                background: Color::Rgb(20, 24, 32),
                surface: Color::Rgb(30, 34, 44),
                text: Color::Rgb(236, 239, 244),
                muted: Color::Rgb(129, 161, 193),
                selection_bg: Color::Rgb(76, 86, 106),
                selection_fg: Color::Rgb(236, 239, 244),
            },
            Theme {
                name: "Tokyo Night",
                accent: Color::Rgb(122, 162, 247),
                background: Color::Rgb(16, 17, 24),
                surface: Color::Rgb(22, 24, 34),
                text: Color::Rgb(192, 202, 245),
                muted: Color::Rgb(86, 95, 137),
                selection_bg: Color::Rgb(65, 79, 140),
                selection_fg: Color::Rgb(241, 246, 255),
            },
            Theme {
                name: "Hacker Green",
                accent: Color::Rgb(80, 255, 120),
                background: Color::Black,
                surface: Color::Rgb(0, 24, 0),
                text: Color::Rgb(120, 255, 140),
                muted: Color::Rgb(64, 160, 80),
                selection_bg: Color::Rgb(0, 110, 0),
                selection_fg: Color::Rgb(210, 255, 220),
            },
        ]
    }
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
    CreateFolder {
        parent_collection: usize,
        parent_folder_id: Option<String>,
    },
    CreateRequest {
        parent_collection: usize,
        parent_folder_id: Option<String>,
    },
    RenameItem {
        item_type: ItemType,
        item_id: String,
        collection_index: usize,
    },
    ConfirmDelete {
        item_type: ItemType,
        item_id: String,
        item_name: String,
        collection_index: usize,
    },
    SaveResponseAs,
    ConfirmOverwrite {
        path: PathBuf,
    },
}

/// Dialog state for input dialogs
#[derive(Debug, Clone, Default)]
pub struct DialogState {
    pub dialog_type: Option<DialogType>,
    pub input_buffer: String,
}

#[derive(Debug, Clone)]
pub struct EnvPopupState {
    pub scroll: u16,
    pub visible_height: usize,
    pub shared: Vec<KeyValue>,
    pub active: Vec<KeyValue>,
    pub selected_section: EnvPopupSection,
    pub selected_index: usize,
}

impl Default for EnvPopupState {
    fn default() -> Self {
        Self {
            scroll: 0,
            visible_height: 0,
            shared: Vec::new(),
            active: Vec::new(),
            selected_section: EnvPopupSection::Shared,
            selected_index: 0,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ThemePopupState {
    pub selected_index: usize,
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
    // Text selection anchor (None = no selection, Some(pos) = selection started at pos)
    pub selection_anchor: Option<usize>,
    // Track mouse drag state for text selection
    mouse_drag_field: Option<EditingField>,

    // Collection/item selection state
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
    pub response_lines: Vec<String>, // Cached pretty-printed lines for efficient rendering
    pub is_loading: bool,
    pub spinner_index: usize,
    pub spinner_last_tick: Instant,
    pub pending_request: Option<oneshot::Receiver<Result<HttpResponse>>>,
    pub pending_request_snapshot: Option<ApiRequest>,

    // Status/error message
    pub status_message: Option<String>,
    pub error_message: Option<String>,

    // Response scroll
    pub response_scroll: u16,

    // Response search/filter state
    pub response_mode: ResponseMode,
    pub response_search_query: String,
    pub response_filter_query: String,
    pub response_cursor_position: usize,
    pub response_filtered_content: Option<String>,
    pub response_search_matches: Vec<usize>,
    pub response_current_match: usize,

    // Filter history
    pub filter_history: Vec<String>,
    pub show_filter_history: bool,
    pub filter_history_selected: usize,

    // Body scroll (for request body editor)
    pub body_scroll: u16,

    // Help popup
    pub show_help: bool,
    // Environment variables popup
    pub show_env_popup: bool,
    pub env_popup: EnvPopupState,

    // Theme selector popup
    pub show_theme_popup: bool,
    pub theme_popup: ThemePopupState,

    // Selected param index for navigation in Params tab
    pub selected_param_index: usize,
    // Selected header index for navigation in Headers tab
    pub selected_header_index: usize,

    // Dialog state
    pub dialog: DialogState,
    pub layout_areas: LayoutAreas,
    pub pending_move: Option<PendingMove>,
    pub settings: Settings,
    pub themes: Vec<Theme>,
    pub active_theme_index: usize,
}

/// Stores the layout areas for mouse click detection
#[derive(Debug, Clone, Default)]
pub struct LayoutAreas {
    pub request_list: Option<(u16, u16, u16, u16)>, // x, y, width, height
    pub url_bar: Option<(u16, u16, u16, u16)>,
    pub request_editor: Option<(u16, u16, u16, u16)>,
    pub response_view: Option<(u16, u16, u16, u16)>,
    pub tabs_row_y: Option<u16>, // y-coordinate of the tabs row
    pub tab_positions: Vec<(u16, u16, RequestTab)>, // x, width, tab
    // Text field positions for click-to-cursor (x where text starts, y, width)
    pub url_text_start: Option<u16>,
    pub body_area: Option<(u16, u16, u16, u16)>, // x, y, width, height for body text area
    pub request_content_area: Option<(u16, u16, u16, u16)>, // content area below tabs
}

impl App {
    pub async fn new() -> Result<Self> {
        let config = Config::new()?;
        config.ensure_dirs()?;

        // Load existing data or create defaults
        let history = HistoryManager::load(&config.history_file).unwrap_or_default();
        let environments = EnvironmentManager::load(&config.environments_file)
            .unwrap_or_else(|_| EnvironmentManager::new());
        let settings = Settings::load(&config.settings_file).unwrap_or_default();
        let filter_history = Self::load_filter_history(&config.filter_history_file);

        // Load collections from the collections directory
        let collections = Self::load_collections(&config.collections_dir)?;

        let http_client = HttpClient::new()?;
        let themes = Theme::presets();
        let active_theme_index = themes
            .iter()
            .position(|theme| theme.name == settings.theme)
            .unwrap_or(0);

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
            selection_anchor: None,
            mouse_drag_field: None,
            selected_collection: 0,
            selected_item: usize::MAX, // usize::MAX means collection header is selected
            selected_history: 0,
            show_history: false,
            current_request: ApiRequest::default(),
            current_request_source: None,
            response: None,
            response_lines: Vec::new(),
            is_loading: false,
            spinner_index: 0,
            spinner_last_tick: Instant::now(),
            pending_request: None,
            pending_request_snapshot: None,
            status_message: None,
            error_message: None,
            response_scroll: 0,
            response_mode: ResponseMode::default(),
            response_search_query: String::new(),
            response_filter_query: String::new(),
            response_cursor_position: 0,
            response_filtered_content: None,
            response_search_matches: Vec::new(),
            response_current_match: 0,
            filter_history,
            show_filter_history: false,
            filter_history_selected: 0,
            body_scroll: 0,
            show_help: false,
            show_env_popup: false,
            env_popup: EnvPopupState::default(),
            show_theme_popup: false,
            theme_popup: ThemePopupState::default(),
            selected_param_index: 0,
            selected_header_index: 0,
            dialog: DialogState::default(),
            layout_areas: LayoutAreas::default(),
            pending_move: None,
            settings,
            themes,
            active_theme_index,
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

        // If theme popup is showing, handle it first
        if self.show_theme_popup {
            return self.handle_theme_popup_input(key);
        }

        // If filter history popup is showing, handle it first
        if self.show_filter_history {
            return self.handle_filter_history_input(key);
        }

        // If env popup is showing, handle it first
        if self.show_env_popup {
            return self.handle_env_popup_input(key);
        }

        // If in response search/filter mode, handle it first
        if self.response_mode != ResponseMode::Normal {
            return self.handle_response_mode_input(key);
        }

        // Global shortcuts
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            match key.code {
                KeyCode::Char('c') if self.input_mode == InputMode::Normal => {
                    return Ok(true);
                }
                KeyCode::Char('e') => {
                    self.open_env_popup();
                    return Ok(false);
                }
                KeyCode::Char('t') => {
                    self.open_theme_popup();
                    return Ok(false);
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

    fn open_env_popup(&mut self) {
        self.show_env_popup = true;
        self.show_help = false;
        self.env_popup.scroll = 0;
        self.env_popup.shared = self.env_popup_items_from_map(&self.environments.shared);
        self.env_popup.active = self
            .environments
            .active()
            .map(|env| self.env_popup_items_from_map(&env.variables))
            .unwrap_or_default();
        self.env_popup.selected_section = if !self.env_popup.shared.is_empty() {
            EnvPopupSection::Shared
        } else if !self.env_popup.active.is_empty() {
            EnvPopupSection::Active
        } else {
            EnvPopupSection::Shared
        };
        self.env_popup.selected_index = 0;
        self.input_mode = InputMode::Normal;
        self.editing_field = None;
    }

    fn open_theme_popup(&mut self) {
        self.show_theme_popup = true;
        self.show_help = false;
        self.theme_popup.selected_index = self.active_theme_index;
        self.input_mode = InputMode::Normal;
        self.editing_field = None;
    }

    fn close_theme_popup(&mut self) {
        self.show_theme_popup = false;
        self.input_mode = InputMode::Normal;
        self.editing_field = None;
    }

    fn close_env_popup(&mut self, save_changes: bool) {
        if save_changes {
            self.apply_env_popup_changes();
        }
        self.show_env_popup = false;
        self.input_mode = InputMode::Normal;
        self.editing_field = None;
    }

    fn env_popup_items_from_map(&self, map: &HashMap<String, String>) -> Vec<KeyValue> {
        let mut items: Vec<KeyValue> = map
            .iter()
            .map(|(key, value)| KeyValue::new(key, value))
            .collect();
        items.sort_by(|a, b| a.key.cmp(&b.key));
        items
    }

    fn apply_env_popup_changes(&mut self) {
        let mut shared = HashMap::new();
        for item in &self.env_popup.shared {
            let key = item.key.trim();
            if key.is_empty() {
                continue;
            }
            shared.insert(key.to_string(), item.value.clone());
        }
        self.environments.shared = shared;

        if let Some(active) = self.environments.active_mut() {
            let mut variables = HashMap::new();
            for item in &self.env_popup.active {
                let key = item.key.trim();
                if key.is_empty() {
                    continue;
                }
                variables.insert(key.to_string(), item.value.clone());
            }
            active.variables = variables;
        }

        match self.environments.save(&self.config.environments_file) {
            Ok(()) => {
                self.status_message = Some("Saved environments".to_string());
            }
            Err(err) => {
                self.error_message = Some(format!("Failed to save environments: {}", err));
            }
        }
    }

    fn apply_theme(&mut self, index: usize) {
        let index = index.min(self.themes.len().saturating_sub(1));
        self.active_theme_index = index;
        if let Some(theme) = self.themes.get(index) {
            self.settings.theme = theme.name.to_string();
        }
        if let Err(err) = self.settings.save(&self.config.settings_file) {
            self.error_message = Some(format!("Failed to save settings: {}", err));
        } else {
            self.status_message = Some("Theme updated".to_string());
        }
    }

    fn theme_popup_move_selection(&mut self, delta: isize) {
        if self.themes.is_empty() {
            return;
        }
        let len = self.themes.len();
        let current = self.theme_popup.selected_index.min(len - 1);
        let step = delta.abs() as usize;
        let next = if delta.is_negative() {
            current.saturating_sub(step)
        } else {
            (current + step).min(len - 1)
        };
        self.theme_popup.selected_index = next;
    }

    fn handle_theme_popup_input(&mut self, key: KeyEvent) -> Result<bool> {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.close_theme_popup();
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.theme_popup_move_selection(-1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.theme_popup_move_selection(1);
            }
            KeyCode::Enter => {
                self.apply_theme(self.theme_popup.selected_index);
                self.close_theme_popup();
            }
            _ => {}
        }
        Ok(false)
    }

    fn handle_filter_history_input(&mut self, key: KeyEvent) -> Result<bool> {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('F') => {
                self.show_filter_history = false;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if self.filter_history_selected > 0 {
                    self.filter_history_selected -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.filter_history_selected + 1 < self.filter_history.len() {
                    self.filter_history_selected += 1;
                }
            }
            KeyCode::Enter => {
                if let Some(filter) = self.filter_history.get(self.filter_history_selected).cloned()
                {
                    self.response_filter_query = filter;
                    self.execute_filter();
                    self.show_filter_history = false;
                }
            }
            KeyCode::Char('d') | KeyCode::Delete => {
                // Delete the selected filter from history
                if self.filter_history_selected < self.filter_history.len() {
                    self.filter_history.remove(self.filter_history_selected);
                    if self.filter_history_selected >= self.filter_history.len()
                        && self.filter_history_selected > 0
                    {
                        self.filter_history_selected -= 1;
                    }
                    // Persist immediately
                    self.save_filter_history();
                    // Close popup if history is now empty
                    if self.filter_history.is_empty() {
                        self.show_filter_history = false;
                    }
                }
            }
            _ => {}
        }
        Ok(false)
    }

    fn handle_env_popup_input(&mut self, key: KeyEvent) -> Result<bool> {
        if self.input_mode == InputMode::Editing {
            return self.handle_env_popup_editing(key);
        }

        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.close_env_popup(true);
            }
            KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.close_env_popup(true);
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.env_popup_move_selection(-1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.env_popup_move_selection(1);
            }
            KeyCode::PageUp => {
                let jump = self.env_popup.visible_height.saturating_sub(1).max(1) as isize;
                self.env_popup_move_selection(-jump);
            }
            KeyCode::PageDown => {
                let jump = self.env_popup.visible_height.saturating_sub(1).max(1) as isize;
                self.env_popup_move_selection(jump);
            }
            KeyCode::Home => {
                self.env_popup_select_first();
            }
            KeyCode::End => {
                self.env_popup_select_last();
            }
            KeyCode::Char('a') => {
                self.env_popup_add_item();
            }
            KeyCode::Char('x') | KeyCode::Delete => {
                self.env_popup_delete_item();
            }
            KeyCode::Enter => {
                self.start_env_popup_editing();
            }
            _ => {}
        }

        Ok(false)
    }

    fn handle_env_popup_editing(&mut self, key: KeyEvent) -> Result<bool> {
        match key.code {
            KeyCode::Esc => {
                self.input_mode = InputMode::Normal;
                self.editing_field = None;
            }
            KeyCode::Tab | KeyCode::Enter => {
                self.env_popup_next_editing_field();
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

    fn start_env_popup_editing(&mut self) {
        match self.env_popup.selected_section {
            EnvPopupSection::Shared => {
                if self.env_popup.shared.is_empty() {
                    self.env_popup.shared.push(KeyValue::new("", ""));
                    self.env_popup.selected_index = 0;
                } else if self.env_popup.selected_index >= self.env_popup.shared.len() {
                    self.env_popup.selected_index = self.env_popup.shared.len() - 1;
                }
                self.input_mode = InputMode::Editing;
                self.set_editing_field(EditingField::EnvSharedKey(self.env_popup.selected_index));
            }
            EnvPopupSection::Active => {
                if self.env_popup.active.is_empty() {
                    self.env_popup.active.push(KeyValue::new("", ""));
                    self.env_popup.selected_index = 0;
                } else if self.env_popup.selected_index >= self.env_popup.active.len() {
                    self.env_popup.selected_index = self.env_popup.active.len() - 1;
                }
                self.input_mode = InputMode::Editing;
                self.set_editing_field(EditingField::EnvActiveKey(self.env_popup.selected_index));
            }
        }
        self.ensure_env_popup_visible();
    }

    fn env_popup_add_item(&mut self) {
        match self.env_popup.selected_section {
            EnvPopupSection::Shared => {
                self.env_popup.shared.push(KeyValue::new("", ""));
                self.env_popup.selected_index = self.env_popup.shared.len() - 1;
                self.input_mode = InputMode::Editing;
                self.set_editing_field(EditingField::EnvSharedKey(self.env_popup.selected_index));
            }
            EnvPopupSection::Active => {
                self.env_popup.active.push(KeyValue::new("", ""));
                self.env_popup.selected_index = self.env_popup.active.len() - 1;
                self.input_mode = InputMode::Editing;
                self.set_editing_field(EditingField::EnvActiveKey(self.env_popup.selected_index));
            }
        }
        self.ensure_env_popup_visible();
    }

    fn env_popup_delete_item(&mut self) {
        match self.env_popup.selected_section {
            EnvPopupSection::Shared => {
                if self.env_popup.selected_index < self.env_popup.shared.len() {
                    self.env_popup.shared.remove(self.env_popup.selected_index);
                }
            }
            EnvPopupSection::Active => {
                if self.env_popup.selected_index < self.env_popup.active.len() {
                    self.env_popup.active.remove(self.env_popup.selected_index);
                }
            }
        }
        self.env_popup_normalize_selection();
        self.ensure_env_popup_visible();
    }

    fn env_popup_move_selection(&mut self, delta: isize) {
        if self.env_popup.shared.is_empty() && self.env_popup.active.is_empty() {
            return;
        }

        let mut section = self.env_popup.selected_section;
        let mut index = self.env_popup.selected_index;
        let mut steps = delta.abs() as usize;
        let moving_down = delta > 0;

        while steps > 0 {
            match (section, moving_down) {
                (EnvPopupSection::Shared, true) => {
                    let len = self.env_popup.shared.len();
                    if len == 0 {
                        if !self.env_popup.active.is_empty() {
                            section = EnvPopupSection::Active;
                            index = 0;
                        }
                    } else if index + 1 < len {
                        index += 1;
                    } else if !self.env_popup.active.is_empty() {
                        section = EnvPopupSection::Active;
                        index = 0;
                    }
                }
                (EnvPopupSection::Active, true) => {
                    let len = self.env_popup.active.len();
                    if len == 0 {
                        if !self.env_popup.shared.is_empty() {
                            section = EnvPopupSection::Shared;
                            index = 0;
                        }
                    } else if index + 1 < len {
                        index += 1;
                    }
                }
                (EnvPopupSection::Active, false) => {
                    let len = self.env_popup.active.len();
                    if len == 0 {
                        if !self.env_popup.shared.is_empty() {
                            section = EnvPopupSection::Shared;
                            index = self.env_popup.shared.len().saturating_sub(1);
                        }
                    } else if index > 0 {
                        index -= 1;
                    } else if !self.env_popup.shared.is_empty() {
                        section = EnvPopupSection::Shared;
                        index = self.env_popup.shared.len().saturating_sub(1);
                    }
                }
                (EnvPopupSection::Shared, false) => {
                    let len = self.env_popup.shared.len();
                    if len == 0 {
                        if !self.env_popup.active.is_empty() {
                            section = EnvPopupSection::Active;
                            index = self.env_popup.active.len().saturating_sub(1);
                        }
                    } else if index > 0 {
                        index -= 1;
                    }
                }
            }
            steps = steps.saturating_sub(1);
        }

        self.env_popup.selected_section = section;
        self.env_popup.selected_index = index;
        self.env_popup_normalize_selection();
        self.ensure_env_popup_visible();
    }

    fn env_popup_select_first(&mut self) {
        if !self.env_popup.shared.is_empty() {
            self.env_popup.selected_section = EnvPopupSection::Shared;
            self.env_popup.selected_index = 0;
        } else if !self.env_popup.active.is_empty() {
            self.env_popup.selected_section = EnvPopupSection::Active;
            self.env_popup.selected_index = 0;
        } else {
            self.env_popup.selected_section = EnvPopupSection::Shared;
            self.env_popup.selected_index = 0;
        }
        self.ensure_env_popup_visible();
    }

    fn env_popup_select_last(&mut self) {
        if !self.env_popup.active.is_empty() {
            self.env_popup.selected_section = EnvPopupSection::Active;
            self.env_popup.selected_index = self.env_popup.active.len() - 1;
        } else if !self.env_popup.shared.is_empty() {
            self.env_popup.selected_section = EnvPopupSection::Shared;
            self.env_popup.selected_index = self.env_popup.shared.len() - 1;
        } else {
            self.env_popup.selected_section = EnvPopupSection::Shared;
            self.env_popup.selected_index = 0;
        }
        self.ensure_env_popup_visible();
    }

    fn env_popup_normalize_selection(&mut self) {
        match self.env_popup.selected_section {
            EnvPopupSection::Shared => {
                if self.env_popup.shared.is_empty() {
                    if !self.env_popup.active.is_empty() {
                        self.env_popup.selected_section = EnvPopupSection::Active;
                        self.env_popup.selected_index =
                            self.env_popup.active.len().saturating_sub(1);
                    } else {
                        self.env_popup.selected_index = 0;
                    }
                } else if self.env_popup.selected_index >= self.env_popup.shared.len() {
                    self.env_popup.selected_index = self.env_popup.shared.len() - 1;
                }
            }
            EnvPopupSection::Active => {
                if self.env_popup.active.is_empty() {
                    if !self.env_popup.shared.is_empty() {
                        self.env_popup.selected_section = EnvPopupSection::Shared;
                        self.env_popup.selected_index =
                            self.env_popup.shared.len().saturating_sub(1);
                    } else {
                        self.env_popup.selected_index = 0;
                    }
                } else if self.env_popup.selected_index >= self.env_popup.active.len() {
                    self.env_popup.selected_index = self.env_popup.active.len() - 1;
                }
            }
        }
    }

    fn env_popup_next_editing_field(&mut self) {
        let next = match self.editing_field {
            Some(EditingField::EnvSharedKey(i)) => EditingField::EnvSharedValue(i),
            Some(EditingField::EnvSharedValue(i)) => {
                let next_idx = i + 1;
                if next_idx >= self.env_popup.shared.len() {
                    self.env_popup.shared.push(KeyValue::new("", ""));
                }
                EditingField::EnvSharedKey(next_idx)
            }
            Some(EditingField::EnvActiveKey(i)) => EditingField::EnvActiveValue(i),
            Some(EditingField::EnvActiveValue(i)) => {
                let next_idx = i + 1;
                if next_idx >= self.env_popup.active.len() {
                    self.env_popup.active.push(KeyValue::new("", ""));
                }
                EditingField::EnvActiveKey(next_idx)
            }
            _ => return,
        };

        match next {
            EditingField::EnvSharedKey(idx) | EditingField::EnvSharedValue(idx) => {
                self.env_popup.selected_section = EnvPopupSection::Shared;
                self.env_popup.selected_index = idx;
            }
            EditingField::EnvActiveKey(idx) | EditingField::EnvActiveValue(idx) => {
                self.env_popup.selected_section = EnvPopupSection::Active;
                self.env_popup.selected_index = idx;
            }
            _ => {}
        }

        self.set_editing_field(next);
        self.ensure_env_popup_visible();
    }

    fn env_popup_selected_line(&self) -> Option<usize> {
        let mut line = 0usize;
        for (section, items) in [
            (EnvPopupSection::Shared, &self.env_popup.shared),
            (EnvPopupSection::Active, &self.env_popup.active),
        ] {
            if line > 0 {
                line += 1;
            }
            line += 1; // header
            let item_count = items.len().max(1);
            if section == self.env_popup.selected_section {
                if items.is_empty() {
                    return Some(line);
                }
                if self.env_popup.selected_index < items.len() {
                    return Some(line + self.env_popup.selected_index);
                }
            }
            line += item_count;
        }
        None
    }

    fn ensure_env_popup_visible(&mut self) {
        let visible_height = self.env_popup.visible_height.max(1);
        let Some(selected_line) = self.env_popup_selected_line() else {
            return;
        };
        let scroll = self.env_popup.scroll as usize;
        if selected_line < scroll {
            self.env_popup.scroll = selected_line as u16;
        } else if selected_line >= scroll + visible_height {
            let new_scroll = selected_line.saturating_sub(visible_height - 1);
            self.env_popup.scroll = new_scroll as u16;
        }

        let max_scroll = self.env_popup_line_count().saturating_sub(visible_height) as u16;
        if self.env_popup.scroll > max_scroll {
            self.env_popup.scroll = max_scroll;
        }
    }

    /// Handle mouse click events
    pub fn handle_mouse_click(&mut self, x: u16, y: u16) {
        // Close help popup if showing
        if self.show_help {
            self.show_help = false;
            return;
        }
        // Close env popup if showing
        if self.show_env_popup {
            self.close_env_popup(true);
            return;
        }
        // Close theme popup if showing
        if self.show_theme_popup {
            self.close_theme_popup();
            return;
        }
        // Close filter history popup if showing
        if self.show_filter_history {
            self.show_filter_history = false;
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
                    self.load_selected_history_request();
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
                                self.load_selected_request();
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
                    self.cursor_position = self.current_request.url.chars().count();
                }
                // Set selection anchor for potential drag selection
                self.selection_anchor = Some(self.cursor_position);
                self.mouse_drag_field = Some(EditingField::Url);
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
                                        let click_row =
                                            (y - by) as usize + self.body_scroll as usize;
                                        let click_col = (x - bx) as usize;

                                        let body = &self.current_request.body;
                                        let lines: Vec<&str> = body.split('\n').collect();

                                        let mut char_pos = 0;
                                        for (i, line) in lines.iter().enumerate() {
                                            if i == click_row {
                                                char_pos += click_col.min(line.chars().count());
                                                break;
                                            }
                                            if i < lines.len() - 1 {
                                                char_pos += line.chars().count() + 1; // +1 for newline
                                            }
                                        }

                                        self.cursor_position = char_pos.min(body.chars().count());
                                        // Set selection anchor for potential drag selection
                                        self.selection_anchor = Some(self.cursor_position);
                                        self.mouse_drag_field = Some(EditingField::Body);
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

        // Clear drag state if click was not in an editable field
        self.mouse_drag_field = None;
        self.selection_anchor = None;
    }

    /// Handle mouse drag events for text selection
    pub fn handle_mouse_drag(&mut self, x: u16, y: u16) {
        // Only handle drag if we started in an editable field
        let Some(drag_field) = &self.mouse_drag_field else {
            return;
        };

        match drag_field {
            EditingField::Url => {
                // Calculate new cursor position from drag position
                if let Some(text_start) = self.layout_areas.url_text_start {
                    let url_len = self.current_request.url.chars().count();
                    if x >= text_start {
                        let drag_offset = (x - text_start) as usize;
                        self.cursor_position = drag_offset.min(url_len);
                    } else {
                        self.cursor_position = 0;
                    }
                }
            }
            EditingField::Body => {
                // Calculate position in body from drag coordinates
                if let Some((bx, by, _bw, _bh)) = self.layout_areas.body_area {
                    let drag_row = (y.saturating_sub(by)) as usize + self.body_scroll as usize;
                    let drag_col = (x.saturating_sub(bx)) as usize;

                    let body = &self.current_request.body;
                    let lines: Vec<&str> = body.split('\n').collect();

                    let mut char_pos = 0;
                    for (i, line) in lines.iter().enumerate() {
                        if i == drag_row {
                            char_pos += drag_col.min(line.chars().count());
                            break;
                        }
                        if i < lines.len() - 1 {
                            char_pos += line.chars().count() + 1; // +1 for newline
                        } else if i < drag_row {
                            // Dragged past last line, position at end
                            char_pos += line.chars().count();
                        }
                    }

                    self.cursor_position = char_pos.min(body.chars().count());
                }
            }
            _ => {
                // Other fields don't support mouse drag selection currently
            }
        }
    }

    /// Handle mouse scroll wheel events
    pub fn handle_scroll(&mut self, x: u16, y: u16, up: bool) {
        if self.show_env_popup {
            if up {
                self.env_popup.scroll = self.env_popup.scroll.saturating_sub(3);
            } else {
                self.env_popup.scroll = self.env_popup.scroll.saturating_add(3);
            }
            return;
        }
        if self.show_theme_popup {
            return;
        }

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

    pub fn theme(&self) -> &Theme {
        &self.themes[self.active_theme_index]
    }

    pub fn theme_text_color(&self) -> Color {
        self.theme().text
    }

    pub fn theme_muted_color(&self) -> Color {
        self.theme().muted
    }

    pub fn theme_surface_color(&self) -> Color {
        self.theme().surface
    }

    pub fn theme_selection_bg(&self) -> Color {
        self.theme().selection_bg
    }

    pub fn theme_selection_fg(&self) -> Color {
        self.theme().selection_fg
    }

    /// Get the display lines for the response (filtered if filter is active, otherwise cached pretty lines)
    pub fn response_display_lines(&self) -> &[String] {
        // If there's filtered content, we need to compute lines from it
        // Otherwise use the cached pretty-printed lines
        &self.response_lines
    }

    /// Get the total number of display lines for the response
    pub fn response_line_count(&self) -> usize {
        if self.response_filtered_content.is_some() {
            // For filtered content, count lines from the filtered string
            self.response_filtered_content
                .as_ref()
                .map(|c| c.lines().count())
                .unwrap_or(0)
        } else {
            self.response_lines.len()
        }
    }

    /// Get the accent color based on the active environment (defaults to theme accent)
    pub fn accent_color(&self) -> Color {
        self.environments
            .active_color()
            .map(|s| Self::parse_color(s))
            .unwrap_or(self.theme().accent)
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

        // Clear search/filter in ResponseView with Esc
        if key.code == KeyCode::Esc && self.focused_panel == FocusedPanel::ResponseView {
            if !self.response_search_matches.is_empty()
                || self.response_filtered_content.is_some()
            {
                self.response_search_query.clear();
                self.response_filter_query.clear();
                self.response_filtered_content = None;
                self.response_search_matches.clear();
                self.response_current_match = 0;
                return Ok(false);
            }
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

            // Send request (lowercase 's' everywhere, uppercase 'S' except ResponseView where it saves)
            KeyCode::Char('s') => {
                self.send_request().await?;
            }
            KeyCode::Char('S') if self.focused_panel != FocusedPanel::ResponseView => {
                self.send_request().await?;
            }

            // Toggle history view
            KeyCode::Char('H') => {
                self.show_history = !self.show_history;
            }

            // New request (not in ResponseView where n/N are for search navigation)
            KeyCode::Char('n') | KeyCode::Char('N')
                if self.focused_panel != FocusedPanel::ResponseView =>
            {
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

            // Save response to file (in response view)
            KeyCode::Char('S') if self.focused_panel == FocusedPanel::ResponseView => {
                self.start_save_response_dialog();
            }

            // Search in response (in response view)
            KeyCode::Char('/') if self.focused_panel == FocusedPanel::ResponseView => {
                if self.response.is_some() {
                    self.response_mode = ResponseMode::Search;
                    self.response_search_query.clear();
                    self.response_cursor_position = 0;
                }
            }

            // JQ filter in response (in response view)
            KeyCode::Char('f') if self.focused_panel == FocusedPanel::ResponseView => {
                if self.response.is_some() {
                    self.response_mode = ResponseMode::Filter;
                    // Keep existing filter query for editing, set cursor at end
                    self.response_cursor_position = self.response_filter_query.len();
                    // Keep filtered content visible while editing
                }
            }

            // Show filter history popup (in response view)
            KeyCode::Char('F') if self.focused_panel == FocusedPanel::ResponseView => {
                if self.response.is_some() && !self.filter_history.is_empty() {
                    self.show_filter_history = true;
                    self.filter_history_selected = 0;
                }
            }

            // Next/prev search match in response view
            KeyCode::Char('n')
                if self.focused_panel == FocusedPanel::ResponseView
                    && !self.response_search_matches.is_empty() =>
            {
                self.next_search_match();
            }
            KeyCode::Char('N')
                if self.focused_panel == FocusedPanel::ResponseView
                    && !self.response_search_matches.is_empty() =>
            {
                self.prev_search_match();
            }

            // Format JSON/GraphQL body
            KeyCode::Char('f')
                if self.focused_panel == FocusedPanel::RequestEditor
                    && self.request_tab == RequestTab::Body =>
            {
                self.format_body();
            }

            // CRUD operations (only in RequestList panel)
            // Uppercase = Create: C (collection), F (folder), R (request)
            // Lowercase = Actions: r (rename), d (delete), p (duplicate), m (move)
            KeyCode::Char('C') if self.focused_panel == FocusedPanel::RequestList => {
                self.start_create_collection();
            }
            KeyCode::Char('F') if self.focused_panel == FocusedPanel::RequestList => {
                self.start_create_folder();
            }
            KeyCode::Char('R')
                if self.focused_panel == FocusedPanel::RequestList && !self.show_history =>
            {
                self.start_create_request();
            }
            KeyCode::Char('r') if self.focused_panel == FocusedPanel::RequestList => {
                self.start_rename_item();
            }
            KeyCode::Char('d') | KeyCode::Delete
                if self.focused_panel == FocusedPanel::RequestList =>
            {
                self.start_delete_item();
            }
            // Duplicate request with p
            KeyCode::Char('p')
                if self.focused_panel == FocusedPanel::RequestList && !self.show_history =>
            {
                self.duplicate_selected_request();
            }
            // Toggle expand/collapse with space
            KeyCode::Char(' ')
                if self.focused_panel == FocusedPanel::RequestList && !self.show_history =>
            {
                self.toggle_expand_collapse();
            }
            // Move item with m
            KeyCode::Char('m')
                if self.focused_panel == FocusedPanel::RequestList && !self.show_history =>
            {
                self.start_move_item();
            }

            _ => {}
        }

        Ok(false)
    }

    fn handle_editing_mode(&mut self, key: KeyEvent) -> Result<bool> {
        let shift = key.modifiers.contains(KeyModifiers::SHIFT);
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

        match key.code {
            KeyCode::Esc => {
                self.input_mode = InputMode::Normal;
                self.editing_field = None;
                self.selection_anchor = None;
            }
            // Tab to move to next field
            KeyCode::Tab => {
                self.selection_anchor = None;
                self.next_editing_field();
            }
            KeyCode::Enter => {
                // For body, add newline at cursor
                // For other fields, move to next field
                if matches!(self.editing_field, Some(EditingField::Body)) {
                    self.delete_selection_if_any();
                    self.handle_char_input('\n');
                } else {
                    self.selection_anchor = None;
                    self.next_editing_field();
                }
            }
            KeyCode::Backspace => {
                if self.has_selection() {
                    self.delete_selection_if_any();
                } else {
                    self.handle_backspace();
                }
            }
            KeyCode::Delete => {
                if self.has_selection() {
                    self.delete_selection_if_any();
                } else {
                    self.handle_delete();
                }
            }
            KeyCode::Left => {
                if shift {
                    self.select_left();
                } else {
                    self.selection_anchor = None;
                    self.cursor_left();
                }
            }
            KeyCode::Right => {
                if shift {
                    self.select_right();
                } else {
                    self.selection_anchor = None;
                    self.cursor_right();
                }
            }
            KeyCode::Up => {
                if shift {
                    self.select_up();
                } else {
                    self.selection_anchor = None;
                    self.cursor_up();
                }
            }
            KeyCode::Down => {
                if shift {
                    self.select_down();
                } else {
                    self.selection_anchor = None;
                    self.cursor_down();
                }
            }
            KeyCode::Home => {
                if shift {
                    self.select_home();
                } else {
                    self.selection_anchor = None;
                    self.cursor_home();
                }
            }
            KeyCode::End => {
                if shift {
                    self.select_end();
                } else {
                    self.selection_anchor = None;
                    self.cursor_end();
                }
            }
            KeyCode::Char('a') if ctrl => {
                self.select_all();
            }
            KeyCode::Char('c') if ctrl => {
                self.copy_selection();
            }
            KeyCode::Char('x') if ctrl => {
                self.cut_selection();
            }
            KeyCode::Char('v') if ctrl => {
                self.paste();
            }
            KeyCode::Char(c) => {
                self.delete_selection_if_any();
                self.handle_char_input(c);
            }
            _ => {}
        }
        Ok(false)
    }

    /// Handle input when in response search/filter mode
    fn handle_response_mode_input(&mut self, key: KeyEvent) -> Result<bool> {
        match key.code {
            KeyCode::Esc => {
                self.response_mode = ResponseMode::Normal;
                self.response_search_query.clear();
                self.response_filter_query.clear();
                self.response_filtered_content = None;
                self.response_search_matches.clear();
                self.response_current_match = 0;
            }
            KeyCode::Enter => {
                match self.response_mode {
                    ResponseMode::Search => {
                        self.execute_search();
                        // Exit search input mode but keep matches visible
                        self.response_mode = ResponseMode::Normal;
                    }
                    ResponseMode::Filter => {
                        self.execute_filter();
                        // Exit filter input mode but keep filtered content
                        self.response_mode = ResponseMode::Normal;
                    }
                    ResponseMode::Normal => {}
                }
            }
            KeyCode::Backspace => {
                match self.response_mode {
                    ResponseMode::Search => {
                        if self.response_cursor_position > 0 {
                            self.response_search_query
                                .remove(self.response_cursor_position - 1);
                            self.response_cursor_position -= 1;
                        }
                    }
                    ResponseMode::Filter => {
                        if self.response_cursor_position > 0 {
                            self.response_filter_query
                                .remove(self.response_cursor_position - 1);
                            self.response_cursor_position -= 1;
                        }
                    }
                    ResponseMode::Normal => {}
                }
            }
            KeyCode::Delete => {
                match self.response_mode {
                    ResponseMode::Search => {
                        if self.response_cursor_position < self.response_search_query.len() {
                            self.response_search_query.remove(self.response_cursor_position);
                        }
                    }
                    ResponseMode::Filter => {
                        if self.response_cursor_position < self.response_filter_query.len() {
                            self.response_filter_query.remove(self.response_cursor_position);
                        }
                    }
                    ResponseMode::Normal => {}
                }
            }
            KeyCode::Left => {
                self.response_cursor_position = self.response_cursor_position.saturating_sub(1);
            }
            KeyCode::Right => {
                let max_pos = match self.response_mode {
                    ResponseMode::Search => self.response_search_query.len(),
                    ResponseMode::Filter => self.response_filter_query.len(),
                    ResponseMode::Normal => 0,
                };
                if self.response_cursor_position < max_pos {
                    self.response_cursor_position += 1;
                }
            }
            KeyCode::Home => {
                self.response_cursor_position = 0;
            }
            KeyCode::End => {
                self.response_cursor_position = match self.response_mode {
                    ResponseMode::Search => self.response_search_query.len(),
                    ResponseMode::Filter => self.response_filter_query.len(),
                    ResponseMode::Normal => 0,
                };
            }
            KeyCode::Char(c) => {
                match self.response_mode {
                    ResponseMode::Search => {
                        self.response_search_query
                            .insert(self.response_cursor_position, c);
                        self.response_cursor_position += 1;
                    }
                    ResponseMode::Filter => {
                        self.response_filter_query
                            .insert(self.response_cursor_position, c);
                        self.response_cursor_position += 1;
                    }
                    ResponseMode::Normal => {}
                }
            }
            _ => {}
        }
        Ok(false)
    }

    /// Execute search on response body
    fn execute_search(&mut self) {
        if self.response.is_none() {
            return;
        }

        let query = self.response_search_query.to_lowercase();

        if query.is_empty() {
            self.response_search_matches.clear();
            self.response_current_match = 0;
            return;
        }

        // Use cached lines or filtered content
        self.response_search_matches = if let Some(filtered) = &self.response_filtered_content {
            filtered
                .lines()
                .enumerate()
                .filter(|(_, line)| line.to_lowercase().contains(&query))
                .map(|(i, _)| i)
                .collect()
        } else {
            self.response_lines
                .iter()
                .enumerate()
                .filter(|(_, line)| line.to_lowercase().contains(&query))
                .map(|(i, _)| i)
                .collect()
        };

        // Jump to first match
        if let Some(&first) = self.response_search_matches.first() {
            self.response_scroll = first as u16;
            self.response_current_match = 0;
        }
    }

    /// Execute JQ filter on response body
    fn execute_filter(&mut self) {
        if let Some(response) = &self.response {
            let query = &self.response_filter_query;

            if query.is_empty() {
                self.response_filtered_content = None;
                self.response_search_matches.clear();
                return;
            }

            match crate::filter::apply_jq_filter(&response.body, query) {
                Ok(result) => {
                    self.response_filtered_content = Some(result);
                    self.response_scroll = 0;
                    self.response_search_matches.clear();
                    self.error_message = None;
                    // Add to filter history if not already present
                    self.add_to_filter_history(query.clone());
                }
                Err(e) => {
                    self.error_message = Some(format!("Filter error: {}", e));
                }
            }
        }
    }

    /// Add a filter to history (avoiding duplicates, most recent first)
    fn add_to_filter_history(&mut self, filter: String) {
        // Remove if already exists (to move it to the front)
        self.filter_history.retain(|f| f != &filter);
        // Add to the front
        self.filter_history.insert(0, filter);
        // Keep only the last 20 filters
        self.filter_history.truncate(20);
        // Persist immediately
        self.save_filter_history();
    }

    /// Jump to next search match
    fn next_search_match(&mut self) {
        if self.response_search_matches.is_empty() {
            return;
        }
        self.response_current_match =
            (self.response_current_match + 1) % self.response_search_matches.len();
        if let Some(&line) = self.response_search_matches.get(self.response_current_match) {
            self.response_scroll = line as u16;
        }
    }

    /// Jump to previous search match
    fn prev_search_match(&mut self) {
        if self.response_search_matches.is_empty() {
            return;
        }
        if self.response_current_match == 0 {
            self.response_current_match = self.response_search_matches.len() - 1;
        } else {
            self.response_current_match -= 1;
        }
        if let Some(&line) = self.response_search_matches.get(self.response_current_match) {
            self.response_scroll = line as u16;
        }
    }

    /// Get mutable reference to current editing field's text
    fn get_current_field_mut(&mut self) -> Option<&mut String> {
        let field = self.editing_field.clone()?;
        match field {
            EditingField::Url => Some(&mut self.current_request.url),
            EditingField::Body => Some(&mut self.current_request.body),
            EditingField::HeaderKey(i) => {
                self.current_request.headers.get_mut(i).map(|h| &mut h.key)
            }
            EditingField::HeaderValue(i) => self
                .current_request
                .headers
                .get_mut(i)
                .map(|h| &mut h.value),
            EditingField::ParamKey(i) => self
                .current_request
                .query_params
                .get_mut(i)
                .map(|p| &mut p.key),
            EditingField::ParamValue(i) => self
                .current_request
                .query_params
                .get_mut(i)
                .map(|p| &mut p.value),
            EditingField::AuthBearerToken => Some(&mut self.current_request.auth.bearer_token),
            EditingField::AuthBasicUsername => Some(&mut self.current_request.auth.basic_username),
            EditingField::AuthBasicPassword => Some(&mut self.current_request.auth.basic_password),
            EditingField::AuthApiKeyName => Some(&mut self.current_request.auth.api_key_name),
            EditingField::AuthApiKeyValue => Some(&mut self.current_request.auth.api_key_value),
            EditingField::EnvSharedKey(i) => {
                self.env_popup.shared.get_mut(i).map(|item| &mut item.key)
            }
            EditingField::EnvSharedValue(i) => {
                self.env_popup.shared.get_mut(i).map(|item| &mut item.value)
            }
            EditingField::EnvActiveKey(i) => {
                self.env_popup.active.get_mut(i).map(|item| &mut item.key)
            }
            EditingField::EnvActiveValue(i) => {
                self.env_popup.active.get_mut(i).map(|item| &mut item.value)
            }
        }
    }

    /// Get current field text length
    fn get_current_field_len(&self) -> usize {
        let Some(field) = &self.editing_field else {
            return 0;
        };
        match field {
            EditingField::Url => self.current_request.url.len(),
            EditingField::Body => self.current_request.body.len(),
            EditingField::HeaderKey(i) => self
                .current_request
                .headers
                .get(*i)
                .map(|h| h.key.len())
                .unwrap_or(0),
            EditingField::HeaderValue(i) => self
                .current_request
                .headers
                .get(*i)
                .map(|h| h.value.len())
                .unwrap_or(0),
            EditingField::ParamKey(i) => self
                .current_request
                .query_params
                .get(*i)
                .map(|p| p.key.len())
                .unwrap_or(0),
            EditingField::ParamValue(i) => self
                .current_request
                .query_params
                .get(*i)
                .map(|p| p.value.len())
                .unwrap_or(0),
            EditingField::AuthBearerToken => self.current_request.auth.bearer_token.len(),
            EditingField::AuthBasicUsername => self.current_request.auth.basic_username.len(),
            EditingField::AuthBasicPassword => self.current_request.auth.basic_password.len(),
            EditingField::AuthApiKeyName => self.current_request.auth.api_key_name.len(),
            EditingField::AuthApiKeyValue => self.current_request.auth.api_key_value.len(),
            EditingField::EnvSharedKey(i) => self
                .env_popup
                .shared
                .get(*i)
                .map(|item| item.key.len())
                .unwrap_or(0),
            EditingField::EnvSharedValue(i) => self
                .env_popup
                .shared
                .get(*i)
                .map(|item| item.value.len())
                .unwrap_or(0),
            EditingField::EnvActiveKey(i) => self
                .env_popup
                .active
                .get(*i)
                .map(|item| item.key.len())
                .unwrap_or(0),
            EditingField::EnvActiveValue(i) => self
                .env_popup
                .active
                .get(*i)
                .map(|item| item.value.len())
                .unwrap_or(0),
        }
    }

    fn handle_backspace(&mut self) {
        let cursor_pos = self.cursor_position;
        if cursor_pos > 0 {
            if let Some(text) = self.get_current_field_mut() {
                // Remove character before cursor
                let byte_pos = text
                    .char_indices()
                    .nth(cursor_pos - 1)
                    .map(|(i, _)| i)
                    .unwrap_or(0);
                let next_byte_pos = text
                    .char_indices()
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
                let byte_pos = text
                    .char_indices()
                    .nth(cursor_pos)
                    .map(|(i, _)| i)
                    .unwrap_or(text.len());
                let next_byte_pos = text
                    .char_indices()
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
            let byte_pos = text
                .char_indices()
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
        let prev_line_start = body[..prev_line_end]
            .rfind('\n')
            .map(|i| i + 1)
            .unwrap_or(0);
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
        let Some(next_line_start) = body[cursor_pos..].find('\n').map(|i| cursor_pos + i + 1)
        else {
            return; // No next line
        };

        // Find end of next line
        let next_line_end = body[next_line_start..]
            .find('\n')
            .map(|i| next_line_start + i)
            .unwrap_or(body.len());
        let next_line_len = next_line_end - next_line_start;

        // Move to same column on next line (or end of line if shorter)
        self.cursor_position = next_line_start + col.min(next_line_len);
        self.ensure_body_cursor_visible();
    }

    // Selection helper functions

    fn has_selection(&self) -> bool {
        if let Some(anchor) = self.selection_anchor {
            anchor != self.cursor_position
        } else {
            false
        }
    }

    /// Get selection range as (start, end) where start <= end
    pub fn get_selection_range(&self) -> Option<(usize, usize)> {
        self.selection_anchor.map(|anchor| {
            let start = anchor.min(self.cursor_position);
            let end = anchor.max(self.cursor_position);
            (start, end)
        })
    }

    fn start_selection_if_needed(&mut self) {
        if self.selection_anchor.is_none() {
            self.selection_anchor = Some(self.cursor_position);
        }
    }

    fn select_left(&mut self) {
        self.start_selection_if_needed();
        self.cursor_left();
    }

    fn select_right(&mut self) {
        self.start_selection_if_needed();
        self.cursor_right();
    }

    fn select_up(&mut self) {
        self.start_selection_if_needed();
        self.cursor_up();
    }

    fn select_down(&mut self) {
        self.start_selection_if_needed();
        self.cursor_down();
    }

    fn select_home(&mut self) {
        self.start_selection_if_needed();
        self.cursor_home();
    }

    fn select_end(&mut self) {
        self.start_selection_if_needed();
        self.cursor_end();
    }

    fn select_all(&mut self) {
        let len = self.get_current_field_len();
        self.selection_anchor = Some(0);
        self.cursor_position = len;
    }

    fn get_selected_text(&self) -> Option<String> {
        let (start, end) = self.get_selection_range()?;
        let text = self.get_current_field_ref()?;

        // Convert char positions to byte positions
        let byte_start = text.char_indices().nth(start).map(|(i, _)| i).unwrap_or(0);
        let byte_end = text.char_indices().nth(end).map(|(i, _)| i).unwrap_or(text.len());

        Some(text[byte_start..byte_end].to_string())
    }

    fn delete_selection_if_any(&mut self) {
        let Some((start, end)) = self.get_selection_range() else {
            return;
        };
        if start == end {
            self.selection_anchor = None;
            return;
        }

        if let Some(text) = self.get_current_field_mut() {
            // Convert char positions to byte positions
            let byte_start = text.char_indices().nth(start).map(|(i, _)| i).unwrap_or(0);
            let byte_end = text.char_indices().nth(end).map(|(i, _)| i).unwrap_or(text.len());
            text.replace_range(byte_start..byte_end, "");
        }
        self.cursor_position = start;
        self.selection_anchor = None;
    }

    fn copy_selection(&mut self) {
        if let Some(text) = self.get_selected_text() {
            if !text.is_empty() {
                let _ = Self::copy_to_clipboard(&text);
            }
        }
    }

    fn cut_selection(&mut self) {
        if let Some(text) = self.get_selected_text() {
            if !text.is_empty() {
                let _ = Self::copy_to_clipboard(&text);
                self.delete_selection_if_any();
            }
        }
    }

    fn paste(&mut self) {
        if let Ok(text) = Self::paste_from_clipboard() {
            self.delete_selection_if_any();
            // Insert pasted text character by character
            for c in text.chars() {
                self.handle_char_input(c);
            }
        }
    }

    fn get_current_field_ref(&self) -> Option<&String> {
        let field = self.editing_field.clone()?;
        match field {
            EditingField::Url => Some(&self.current_request.url),
            EditingField::Body => Some(&self.current_request.body),
            EditingField::HeaderKey(i) => self.current_request.headers.get(i).map(|h| &h.key),
            EditingField::HeaderValue(i) => self.current_request.headers.get(i).map(|h| &h.value),
            EditingField::ParamKey(i) => self.current_request.query_params.get(i).map(|p| &p.key),
            EditingField::ParamValue(i) => {
                self.current_request.query_params.get(i).map(|p| &p.value)
            }
            EditingField::AuthBearerToken => Some(&self.current_request.auth.bearer_token),
            EditingField::AuthBasicUsername => Some(&self.current_request.auth.basic_username),
            EditingField::AuthBasicPassword => Some(&self.current_request.auth.basic_password),
            EditingField::AuthApiKeyName => Some(&self.current_request.auth.api_key_name),
            EditingField::AuthApiKeyValue => Some(&self.current_request.auth.api_key_value),
            EditingField::EnvSharedKey(i) => self.env_popup.shared.get(i).map(|kv| &kv.key),
            EditingField::EnvSharedValue(i) => self.env_popup.shared.get(i).map(|kv| &kv.value),
            EditingField::EnvActiveKey(i) => self.env_popup.active.get(i).map(|kv| &kv.key),
            EditingField::EnvActiveValue(i) => self.env_popup.active.get(i).map(|kv| &kv.value),
        }
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
        let visible_height = self
            .layout_areas
            .body_area
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
                    self.load_selected_history_request();
                } else {
                    self.navigate_collection_up();
                    self.load_selected_request();
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
                    self.load_selected_history_request();
                } else {
                    self.navigate_collection_down();
                    self.load_selected_request();
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
                    // History request already loaded on selection, just move focus
                    self.focused_panel = FocusedPanel::UrlBar;
                } else if self.is_collection_header_selected() {
                    // Toggle collection expansion
                    if let Some(collection) = self.collections.get_mut(self.selected_collection) {
                        collection.expanded = !collection.expanded;
                    }
                } else if let Some(collection) = self.collections.get(self.selected_collection) {
                    // Check if selected item is a folder or request
                    let flattened = collection.flatten();
                    if let Some((_, item)) = flattened.get(self.selected_item) {
                        match item {
                            CollectionItem::Folder { .. } => {
                                // Toggle folder expansion
                                self.toggle_expand_collapse();
                            }
                            CollectionItem::Request(_) => {
                                // Request already loaded on selection, just move focus
                                self.focused_panel = FocusedPanel::UrlBar;
                            }
                        }
                    }
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
                    self.current_request
                        .headers
                        .push(crate::storage::KeyValue::new("", ""));
                    self.selected_header_index = 0;
                }
                // Start editing the selected header
                let idx = self
                    .selected_header_index
                    .min(self.current_request.headers.len().saturating_sub(1));
                EditingField::HeaderKey(idx)
            }
            RequestTab::Body => EditingField::Body,
            RequestTab::Auth => match self.current_request.auth.auth_type {
                crate::storage::AuthType::None => {
                    self.status_message = Some("Select auth type first with 'a' key".to_string());
                    EditingField::Url
                }
                crate::storage::AuthType::Bearer => EditingField::AuthBearerToken,
                crate::storage::AuthType::Basic => EditingField::AuthBasicUsername,
                crate::storage::AuthType::ApiKey => EditingField::AuthApiKeyName,
            },
            RequestTab::Params => {
                if self.current_request.query_params.is_empty() {
                    self.current_request
                        .query_params
                        .push(crate::storage::KeyValue::new("", ""));
                    self.selected_param_index = 0;
                }
                // Start editing the selected param
                let idx = self
                    .selected_param_index
                    .min(self.current_request.query_params.len().saturating_sub(1));
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
                    self.current_request
                        .headers
                        .push(crate::storage::KeyValue::new("", ""));
                    EditingField::HeaderKey(next_idx)
                }
            }
            // Params: key -> value -> next key -> next value -> ...
            (Some(EditingField::ParamKey(i)), RequestTab::Params) => EditingField::ParamValue(*i),
            (Some(EditingField::ParamValue(i)), RequestTab::Params) => {
                let next_idx = i + 1;
                if next_idx < self.current_request.query_params.len() {
                    EditingField::ParamKey(next_idx)
                } else {
                    // Add new param and edit it
                    self.current_request
                        .query_params
                        .push(crate::storage::KeyValue::new("", ""));
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
            (Some(EditingField::AuthApiKeyName), RequestTab::Auth) => EditingField::AuthApiKeyValue,
            (Some(EditingField::AuthApiKeyValue), RequestTab::Auth) => EditingField::AuthApiKeyName,
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

    fn load_selected_history_request(&mut self) {
        if let Some(entry) = self.history.entries.get(self.selected_history) {
            self.current_request = entry.request.clone();
            self.current_request_source = None; // History items aren't linked to collections
            self.response = None;
            self.selected_param_index = 0;
            self.selected_header_index = 0;
            self.body_scroll = 0;
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
        // Clear selection in request list (no item selected)
        self.selected_item = usize::MAX;
        self.focused_panel = FocusedPanel::UrlBar;
        self.input_mode = InputMode::Editing;
        self.set_editing_field(EditingField::Url);
    }

    fn toggle_selected_param(&mut self) {
        if let Some(param) = self
            .current_request
            .query_params
            .get_mut(self.selected_param_index)
        {
            param.enabled = !param.enabled;
        }
    }

    fn delete_selected_param(&mut self) {
        if self.selected_param_index < self.current_request.query_params.len() {
            self.current_request
                .query_params
                .remove(self.selected_param_index);
            // Adjust selection if needed
            if self.selected_param_index >= self.current_request.query_params.len()
                && self.selected_param_index > 0
            {
                self.selected_param_index -= 1;
            }
        }
    }

    fn toggle_selected_header(&mut self) {
        if let Some(header) = self
            .current_request
            .headers
            .get_mut(self.selected_header_index)
        {
            header.enabled = !header.enabled;
        }
    }

    fn delete_selected_header(&mut self) {
        if self.selected_header_index < self.current_request.headers.len() {
            self.current_request
                .headers
                .remove(self.selected_header_index);
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
                let names: Vec<_> = env_manager
                    .environments
                    .iter()
                    .map(|e| e.name.clone())
                    .collect();

                // Try to restore the previously active environment by name
                if let Some(idx) = env_manager
                    .environments
                    .iter()
                    .position(|e| e.name == current_env_name)
                {
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

    fn paste_from_clipboard() -> Result<String, std::io::Error> {
        #[cfg(target_os = "macos")]
        {
            let output = std::process::Command::new("pbpaste").output()?;
            return Ok(String::from_utf8_lossy(&output.stdout).to_string());
        }

        #[cfg(target_os = "linux")]
        {
            // Try wl-paste first (Wayland), then fall back to xclip (X11)
            if let Ok(output) = std::process::Command::new("wl-paste")
                .arg("-n")
                .output()
            {
                if output.status.success() {
                    return Ok(String::from_utf8_lossy(&output.stdout).to_string());
                }
            }

            // Fall back to xclip for X11
            let output = std::process::Command::new("xclip")
                .args(["-selection", "clipboard", "-o"])
                .output()?;
            return Ok(String::from_utf8_lossy(&output.stdout).to_string());
        }

        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "Clipboard not supported on this platform",
        ))
    }

    fn copy_as_curl(&mut self) {
        let mut curl_cmd = self.request_to_curl();

        // Append jq filter if one is active
        if self.response_filtered_content.is_some() && !self.response_filter_query.is_empty() {
            // Escape single quotes in the filter for shell
            let escaped_filter = self.response_filter_query.replace("'", "'\\''");
            curl_cmd = format!("{} | jq '{}'", curl_cmd, escaped_filter);
        }

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

        // Use filtered content if a filter is active, otherwise use full response
        let content = if let Some(filtered) = &self.response_filtered_content {
            filtered.clone()
        } else {
            response.pretty_body()
        };

        let message = if self.response_filtered_content.is_some() {
            "Copied filtered response to clipboard"
        } else {
            "Copied response to clipboard"
        };

        match Self::copy_to_clipboard(&content) {
            Ok(_) => self.status_message = Some(message.to_string()),
            Err(e) => self.error_message = Some(format!("Failed to copy: {}", e)),
        }
    }

    fn save_response_to_file(&mut self, path: &str) {
        if self.response.is_none() {
            self.error_message = Some("No response to save".to_string());
            return;
        }

        // Expand ~ to home directory
        let expanded_path = if path.starts_with("~/") {
            if let Some(home) = dirs::home_dir() {
                home.join(&path[2..])
            } else {
                PathBuf::from(path)
            }
        } else {
            PathBuf::from(path)
        };

        // Check if file exists - if so, prompt for overwrite
        if expanded_path.exists() {
            self.dialog = DialogState {
                dialog_type: Some(DialogType::ConfirmOverwrite {
                    path: expanded_path,
                }),
                input_buffer: String::new(),
            };
            return;
        }

        self.write_response_to_path(&expanded_path);
    }

    fn write_response_to_path(&mut self, path: &PathBuf) {
        let Some(response) = &self.response else {
            self.error_message = Some("No response to save".to_string());
            return;
        };

        // Use filtered content if active, otherwise use pretty body
        let content = if let Some(filtered) = &self.response_filtered_content {
            filtered.clone()
        } else {
            response.pretty_body()
        };

        match std::fs::write(path, &content) {
            Ok(_) => {
                let msg = if self.response_filtered_content.is_some() {
                    format!("Saved filtered response to {}", path.display())
                } else {
                    format!("Saved response to {}", path.display())
                };
                self.status_message = Some(msg);
            }
            Err(e) => {
                self.error_message = Some(format!("Failed to save: {}", e));
            }
        }
    }

    fn save_response_with_increment(&mut self, original_path: &PathBuf) {
        // Find next available filename by adding (n) before extension
        let stem = original_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("response");
        let extension = original_path
            .extension()
            .and_then(|s| s.to_str());
        let parent = original_path.parent();

        let mut counter = 1;
        let new_path = loop {
            let new_name = if let Some(ext) = extension {
                format!("{}({}).{}", stem, counter, ext)
            } else {
                format!("{}({})", stem, counter)
            };

            let candidate = if let Some(p) = parent {
                p.join(&new_name)
            } else {
                PathBuf::from(&new_name)
            };

            if !candidate.exists() {
                break candidate;
            }
            counter += 1;
        };

        self.write_response_to_path(&new_path);
    }

    fn format_body(&mut self) {
        if self.is_graphql_body() {
            self.format_body_graphql();
        } else {
            self.format_body_json();
        }
    }

    fn format_body_json(&mut self) {
        let body = &self.current_request.body;
        if body.trim().is_empty() {
            return;
        }

        match serde_json::from_str::<serde_json::Value>(body) {
            Ok(parsed) => match serde_json::to_string_pretty(&parsed) {
                Ok(formatted) => {
                    self.current_request.body = formatted;
                    self.status_message = Some("Formatted JSON".to_string());
                }
                Err(e) => {
                    self.error_message = Some(format!("Failed to format: {}", e));
                }
            },
            Err(e) => {
                self.error_message = Some(format!("Invalid JSON: {}", e));
            }
        }
    }

    fn format_body_graphql(&mut self) {
        let body = &self.current_request.body;
        if body.trim().is_empty() {
            return;
        }

        match parse_query::<String>(body) {
            Ok(document) => {
                let formatted = format!("{}", document);
                self.current_request.body = formatted;
                self.status_message = Some("Formatted GraphQL".to_string());
            }
            Err(e) => {
                self.error_message = Some(format!("Invalid GraphQL: {}", e));
            }
        }
    }

    fn is_graphql_body(&self) -> bool {
        self.current_request.headers.iter().any(|header| {
            if !header.enabled {
                return false;
            }
            if !header.key.eq_ignore_ascii_case("content-type") {
                return false;
            }
            header
                .value
                .to_ascii_lowercase()
                .contains("application/graphql")
        })
    }

    pub fn body_format_label(&self) -> &'static str {
        if self.is_graphql_body() {
            "GraphQL"
        } else {
            "JSON"
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
                let token = self
                    .environments
                    .interpolate(&self.current_request.auth.bearer_token);
                parts.push(format!("-H 'Authorization: Bearer {}'", token));
            }
            crate::storage::AuthType::Basic => {
                let user = self
                    .environments
                    .interpolate(&self.current_request.auth.basic_username);
                let pass = self
                    .environments
                    .interpolate(&self.current_request.auth.basic_password);
                parts.push(format!("-u '{}:{}'", user, pass));
            }
            crate::storage::AuthType::ApiKey => {
                let name = self
                    .environments
                    .interpolate(&self.current_request.auth.api_key_name);
                let value = self
                    .environments
                    .interpolate(&self.current_request.auth.api_key_value);
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
        let enabled_params: Vec<_> = self
            .current_request
            .query_params
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
            let name = self
                .environments
                .interpolate(&self.current_request.auth.api_key_name);
            let value = self
                .environments
                .interpolate(&self.current_request.auth.api_key_value);
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
            let collection_idx = *collection_idx;
            let request = self.current_request.clone();
            if let Some(collection) = self.collections.get_mut(collection_idx) {
                if collection.update_request(&request_id, |r| {
                    r.name = request.name.clone();
                    r.method = request.method.clone();
                    r.url = request.url.clone();
                    r.headers = request.headers.clone();
                    r.query_params = request.query_params.clone();
                    r.body = request.body.clone();
                    r.auth = request.auth.clone();
                }) {
                    self.save_collection(collection_idx);
                    self.status_message = Some("Request saved".to_string());
                } else {
                    self.error_message = Some("Failed to save request".to_string());
                }
            }
        } else {
            // No source - this is a new request, prompt to save to collection
            self.error_message =
                Some("Use 'r' in Request List to create a new saved request".to_string());
        }
    }

    async fn send_request(&mut self) -> Result<()> {
        if self.current_request.url.is_empty() {
            self.error_message = Some("URL is required".to_string());
            return Ok(());
        }

        if self.is_loading {
            return Ok(());
        }

        self.is_loading = true;
        self.status_message = Some("Sending request...".to_string());

        let request = self.current_request.clone();
        let http_client = self.http_client.clone();
        let env_manager = self.environments.clone();
        let (sender, receiver) = oneshot::channel();
        self.pending_request_snapshot = Some(request.clone());

        tokio::spawn(async move {
            let interpolate = move |s: &str| env_manager.interpolate(s);
            let result = http_client.execute(&request, interpolate).await;
            let _ = sender.send(result);
        });

        self.pending_request = Some(receiver);
        Ok(())
    }

    fn finish_request(&mut self, result: Result<HttpResponse>) {
        let request_snapshot = self
            .pending_request_snapshot
            .clone()
            .unwrap_or_else(|| self.current_request.clone());

        match result {
            Ok(response) => {
                // Add to history
                let history_entry = HistoryEntry::new(
                    request_snapshot,
                    Some(response.status),
                    response.duration_ms,
                );
                self.history.add(history_entry);

                self.status_message = Some(format!(
                    "{} {} - {}ms",
                    response.status, response.status_text, response.duration_ms
                ));
                // Cache pretty-printed lines for efficient rendering
                self.response_lines = response
                    .pretty_body()
                    .lines()
                    .map(String::from)
                    .collect();
                self.response = Some(response);
                self.response_scroll = 0;
                self.error_message = None;

                // Clear search/filter state for new response
                self.response_search_query.clear();
                self.response_filter_query.clear();
                self.response_filtered_content = None;
                self.response_search_matches.clear();
                self.response_current_match = 0;
                self.response_mode = ResponseMode::Normal;

                // Auto-focus response pane
                self.focused_panel = FocusedPanel::ResponseView;
            }
            Err(e) => {
                // Add failed request to history
                let history_entry = HistoryEntry::new(request_snapshot, None, 0);
                self.history.add(history_entry);

                self.error_message = Some(format!("Request failed: {}", e));
                self.response = None;
                self.response_lines.clear();
            }
        }

        self.pending_request_snapshot = None;
        self.is_loading = false;
    }

    pub fn set_error(&mut self, msg: String) {
        self.error_message = Some(msg);
    }

    /// Called periodically to process async tasks
    pub async fn tick(&mut self) -> Result<()> {
        if self.is_loading {
            if self.spinner_last_tick.elapsed() >= Duration::from_millis(120) {
                self.spinner_index = (self.spinner_index + 1) % Self::spinner_frames().len();
                self.spinner_last_tick = Instant::now();
            }
        } else {
            self.spinner_index = 0;
            self.spinner_last_tick = Instant::now();
        }

        if let Some(receiver) = &mut self.pending_request {
            match receiver.try_recv() {
                Ok(result) => {
                    self.pending_request = None;
                    self.finish_request(result);
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Closed) => {
                    self.pending_request = None;
                    self.pending_request_snapshot = None;
                    self.is_loading = false;
                    self.error_message = Some("Request cancelled".to_string());
                }
            }
        }

        Ok(())
    }

    pub fn spinner_frame(&self) -> &'static str {
        let frames = Self::spinner_frames();
        frames[self.spinner_index % frames.len()]
    }

    fn spinner_frames() -> &'static [&'static str] {
        &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]
    }

    /// Handle key input when a dialog is showing
    fn handle_dialog_input(&mut self, key: KeyEvent) -> Result<bool> {
        let Some(dialog_type) = self.dialog.dialog_type.clone() else {
            return Ok(false);
        };

        match &dialog_type {
            DialogType::ConfirmDelete {
                item_type,
                item_id,
                collection_index,
                ..
            } => match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    self.execute_delete(item_type.clone(), item_id.clone(), *collection_index);
                    self.dialog = DialogState::default();
                }
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                    self.dialog = DialogState::default();
                }
                _ => {}
            },
            DialogType::ConfirmOverwrite { path } => match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    let path = path.clone();
                    self.dialog = DialogState::default();
                    self.write_response_to_path(&path);
                }
                KeyCode::Char('n') | KeyCode::Char('N') => {
                    let path = path.clone();
                    self.dialog = DialogState::default();
                    self.save_response_with_increment(&path);
                }
                KeyCode::Esc => {
                    self.dialog = DialogState::default();
                }
                _ => {}
            },
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
                let idx = self.collections.len() - 1;
                self.save_collection(idx);
                self.status_message = Some(format!("Created collection: {}", name));
            }
            DialogType::CreateFolder {
                parent_collection,
                parent_folder_id,
            } => {
                if let Some(collection) = self.collections.get_mut(parent_collection) {
                    collection.add_folder_to(&name, parent_folder_id.as_deref());
                    self.save_collection(parent_collection);
                    self.status_message = Some(format!("Created folder: {}", name));
                }
            }
            DialogType::CreateRequest {
                parent_collection,
                parent_folder_id,
            } => {
                if let Some(collection) = self.collections.get_mut(parent_collection) {
                    let request = ApiRequest::new(&name);
                    collection.add_request_to(request, parent_folder_id.as_deref());
                    self.save_collection(parent_collection);
                    self.status_message = Some(format!("Created request: {}", name));
                }
            }
            DialogType::RenameItem {
                item_type,
                item_id,
                collection_index,
            } => match item_type {
                ItemType::Collection => {
                    if let Some(collection) = self.collections.get_mut(collection_index) {
                        collection.rename(&name);
                        self.save_collection(collection_index);
                        self.status_message = Some(format!("Renamed to: {}", name));
                    }
                }
                ItemType::Folder | ItemType::Request => {
                    if let Some(collection) = self.collections.get_mut(collection_index) {
                        collection.rename_item(&item_id, &name);
                        self.save_collection(collection_index);
                        self.status_message = Some(format!("Renamed to: {}", name));
                    }
                }
            },
            DialogType::ConfirmDelete { .. } | DialogType::ConfirmOverwrite { .. } => {
                unreachable!()
            }
            DialogType::SaveResponseAs => {
                self.save_response_to_file(&name);
                // save_response_to_file may set a new dialog (ConfirmOverwrite)
                // so only clear if no new dialog was set
                if self.dialog.dialog_type.is_some() {
                    return;
                }
            }
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
                    if self.selected_collection >= self.collections.len()
                        && !self.collections.is_empty()
                    {
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
                    self.save_collection(collection_index);
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

    fn start_save_response_dialog(&mut self) {
        if self.response.is_none() {
            self.error_message = Some("No response to save".to_string());
            return;
        }
        self.dialog = DialogState {
            dialog_type: Some(DialogType::SaveResponseAs),
            input_buffer: String::new(),
        };
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
        self.save_collection(self.selected_collection);

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
            self.status_message = Some(format!(
                "Moving: {} - navigate to destination, Enter to move, Esc to cancel",
                item_name
            ));
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
        if pending.source_collection_index == dest_collection_index
            && source_folder_id == dest_folder_id
        {
            self.status_message = Some("Item already in this location".to_string());
            return;
        }

        // Extract item from source collection
        let item = {
            let source_collection = match self.collections.get_mut(pending.source_collection_index)
            {
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
            // Save affected collections
            self.save_collection(pending.source_collection_index);
            if dest_collection_index != pending.source_collection_index {
                self.save_collection(dest_collection_index);
            }
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
            if let CollectionItem::Folder {
                id,
                items: folder_items,
                ..
            } = item
            {
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

        let collection_index = self.selected_collection;
        let mut toggled_folder_id: Option<String> = None;
        let mut should_adjust_selection = false;

        {
            let collection = match self.collections.get_mut(collection_index) {
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
                should_adjust_selection = true;
            } else if let Some((_, item)) = flattened.get(self.selected_item) {
                if let CollectionItem::Folder { id, .. } = item {
                    let folder_id = id.clone();
                    // Need to toggle the folder's expanded state
                    Self::toggle_folder_expanded(&mut collection.items, &folder_id);
                    toggled_folder_id = Some(folder_id);
                    should_adjust_selection = true;
                } else {
                    // Selected item is a request - find parent folder and collapse that
                    let item_id = item.id().to_string();
                    if let Some(parent_folder_id) =
                        Self::find_parent_folder_recursive(&collection.items, &item_id)
                    {
                        Self::toggle_folder_expanded(&mut collection.items, &parent_folder_id);
                        toggled_folder_id = Some(parent_folder_id);
                        should_adjust_selection = true;
                    } else {
                        // No parent folder, request is at root level - toggle the collection
                        collection.expanded = !collection.expanded;
                        should_adjust_selection = true;
                    }
                }
            }
        }

        if should_adjust_selection {
            self.adjust_collection_selection(collection_index, toggled_folder_id.as_deref());
        }
    }

    fn adjust_collection_selection(
        &mut self,
        collection_index: usize,
        toggled_folder_id: Option<&str>,
    ) {
        if self.is_collection_header_selected() {
            return;
        }

        let Some(collection) = self.collections.get(collection_index) else {
            return;
        };

        if !collection.expanded {
            self.selected_item = usize::MAX;
            return;
        }

        let flattened = collection.flatten();
        if flattened.is_empty() {
            self.selected_item = usize::MAX;
            return;
        }

        if let Some(folder_id) = toggled_folder_id {
            if let Some(expanded) = Self::folder_expanded(&collection.items, folder_id) {
                if !expanded {
                    if let Some((idx, _)) = flattened
                        .iter()
                        .enumerate()
                        .find(|(_, (_, item))| item.id() == folder_id)
                    {
                        self.selected_item = idx;
                        return;
                    }
                }
            }
        }

        if self.selected_item >= flattened.len() {
            self.selected_item = flattened.len().saturating_sub(1);
        }
    }

    fn folder_expanded(items: &[CollectionItem], folder_id: &str) -> Option<bool> {
        for item in items {
            if let CollectionItem::Folder {
                id,
                expanded,
                items: sub_items,
                ..
            } = item
            {
                if id == folder_id {
                    return Some(*expanded);
                }
                if let Some(found) = Self::folder_expanded(sub_items, folder_id) {
                    return Some(found);
                }
            }
        }
        None
    }

    fn toggle_folder_expanded(items: &mut [CollectionItem], folder_id: &str) -> bool {
        for item in items {
            if let CollectionItem::Folder {
                id,
                expanded,
                items: sub_items,
                ..
            } = item
            {
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
            Some((
                ItemType::Collection,
                collection.id.clone(),
                collection.name.clone(),
            ))
        }
    }

    /// Save current state to disk
    pub fn save(&self) -> Result<()> {
        // Save history
        self.history.save(&self.config.history_file)?;

        // Save environments
        self.environments.save(&self.config.environments_file)?;

        // Save filter history
        self.save_filter_history();

        // Save collections
        for collection in &self.collections {
            self.save_collection_to_disk(collection);
        }

        Ok(())
    }

    /// Save a single collection to disk
    fn save_collection_to_disk(&self, collection: &Collection) {
        let path = self
            .config
            .collections_dir
            .join(format!("{}.json", collection.id));
        if let Err(e) = collection.save(&path) {
            tracing::error!("Failed to save collection {}: {}", collection.name, e);
        }
    }

    /// Save a collection by index
    fn save_collection(&self, index: usize) {
        if let Some(collection) = self.collections.get(index) {
            self.save_collection_to_disk(collection);
        }
    }

    /// Load filter history from disk
    fn load_filter_history(path: &std::path::Path) -> Vec<String> {
        if let Ok(content) = std::fs::read_to_string(path) {
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            Vec::new()
        }
    }

    /// Save filter history to disk
    fn save_filter_history(&self) {
        if let Ok(content) = serde_json::to_string_pretty(&self.filter_history) {
            let _ = std::fs::write(&self.config.filter_history_file, content);
        }
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
        help.push(("Ctrl+e", "Edit env variables"));
        help.push(("Ctrl+t", "Select theme"));
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
                        help.push(("Space", "Toggle expand/collapse"));
                        help.push(("H", "Toggle history view"));
                        help.push(("n", "New request (in editor)"));
                        help.push(("", "── Create (uppercase) ──"));
                        help.push(("C", "Create collection"));
                        help.push(("F", "Create folder"));
                        help.push(("R", "Create request"));
                        help.push(("", "── Actions (lowercase) ──"));
                        help.push(("r", "Rename selected"));
                        help.push(("d", "Delete selected"));
                        help.push(("p", "Duplicate request"));
                        help.push(("m", "Move item (cut/paste)"));
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
                                help.push(("f", "Format JSON/GraphQL"));
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
                        help.push(("S", "Save response to file"));
                        help.push(("s", "Send request again"));
                        help.push(("/", "Search in response"));
                        help.push(("f", "JQ filter (e.g. .data, .[0])"));
                        help.push(("F", "Filter history"));
                        help.push(("n / N", "Next/prev search match"));
                        help.push(("Esc", "Clear search/filter"));
                    }
                }
            }
        }

        help
    }

    fn env_popup_line_count(&self) -> usize {
        let mut lines = 0usize;
        for items in [&self.env_popup.shared, &self.env_popup.active] {
            if lines > 0 {
                lines += 1;
            }
            lines += 1;
            lines += items.len().max(1);
        }
        lines
    }
}

impl Drop for App {
    fn drop(&mut self) {
        // Try to save on exit
        let _ = self.save();
    }
}
