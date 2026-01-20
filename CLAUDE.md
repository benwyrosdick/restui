# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build Commands

```bash
cargo build              # Debug build
cargo build --release    # Release build
cargo run                # Run in development
cargo test               # Run all tests
cargo test test_name     # Run specific test
cargo clippy             # Lint
cargo fmt                # Format code
```

## Architecture

restui is a terminal-based API testing tool (like Postman) built with Rust. It uses Ratatui for the TUI and Tokio for async operations.

### Core Modules

- **`app.rs`** - Central application state and keyboard/mouse handling. Contains the `App` struct which holds all UI state, current request data, and coordinates between components. All keyboard shortcuts are handled here via `handle_key()` and `handle_normal_mode()`.

- **`storage/`** - Data persistence layer:
  - `collection.rs` - Collections and folders of saved requests
  - `request.rs` - `ApiRequest` struct with HTTP method, headers, body, auth config
  - `environment.rs` - Environment variables with `{{variable}}` interpolation
  - `history.rs` - Request history tracking
  - `settings.rs` - User preferences (theme selection)

- **`http/client.rs`** - HTTP client wrapper using reqwest. Handles request execution with variable interpolation and auth application.

- **`ui/`** - Ratatui rendering:
  - `layout.rs` - Main 4-panel layout (request list, URL bar, request editor, response view)
  - `request_list.rs` - Left sidebar with collections/history tree
  - `url_bar.rs` - HTTP method selector and URL input
  - `request_editor.rs` - Tabbed editor (Headers, Body, Auth, Params)
  - `response.rs` - Response display with JSON highlighting, search (`/`), and jq filtering (`f`)
  - `widgets/` - Reusable UI components like text input with cursor

- **`filter.rs`** - JQ-style JSON filtering using jaq library

### Key Patterns

- **Panel focus**: `FocusedPanel` enum tracks which of the 4 panels is active. Tab/Shift+Tab cycles panels.
- **Input modes**: `InputMode::Normal` for navigation, `InputMode::Editing` for text input. Press `i`/`Enter` to edit, `Esc` to exit.
- **Variable interpolation**: `{{var}}` syntax in URLs/headers/body gets replaced via `EnvironmentManager::interpolate()` before HTTP requests.
- **Async requests**: HTTP requests run via Tokio with a oneshot channel. `app.tick()` polls for completion.

### Data Storage

All data stored in `~/.config/restui/`:
- `collections/` - JSON files for saved request collections
- `history.json` - Request history
- `environments.json` - Environment variables
- `settings.json` - Theme and preferences
