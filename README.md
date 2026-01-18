# restui

A terminal-based API testing tool built with Rust and Ratatui. Similar to Postman/Posting, but runs entirely in your terminal.

## Features

- **HTTP Methods**: GET, POST, PUT, DELETE
- **Request Editor**: Tabs for Headers, Body, Auth, and Query Params
- **Authentication**: Bearer token, Basic auth, API Key (header or query param)
- **Environment Variables**: Postman-style `{{variable}}` interpolation
- **Collections**: Save and organize requests as JSON files
- **Request History**: Track previously sent requests
- **Response Viewer**: JSON syntax highlighting
- **Mouse Support**: Click on panels, tabs, and request list items
- **Keyboard Navigation**: Vim-style (j/k/h/l) and arrow keys

## Installation

```bash
cargo build --release
./target/release/restui
```

## Usage

### Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `Tab` | Cycle between panels |
| `Shift+Tab` | Cycle panels (reverse) |
| `j/k` or arrows | Navigate up/down |
| `h/l` or arrows | Switch tabs (in Request Editor) |
| `Enter` or `i` | Start editing |
| `Esc` | Exit edit mode |
| `s` | Send request |
| `m` | Cycle HTTP method |
| `n` | New request |
| `e` | Switch environment |
| `H` | Toggle history view |
| `a` | Cycle auth type (in Auth tab) |
| `?` | Show help |
| `q` | Quit |

### Mouse Support

- Click on any panel to focus it
- Click on tabs (Headers/Body/Auth/Params) to switch
- Click on requests in the list to select them
- Click on URL bar to start editing

### Panels

1. **Request List** (left): Browse collections or history
2. **URL Bar** (top right): HTTP method and URL
3. **Request Editor** (middle right): Configure headers, body, auth, params
4. **Response View** (bottom right): View response with syntax highlighting

## Configuration

Data is stored in `~/.config/restui/`:
- `collections/` - Saved request collections (JSON)
- `history.json` - Request history
- `environments.json` - Environment variables

## Environment Variables

Use `{{variable_name}}` syntax in URLs, headers, or body. Variables are interpolated from the active environment before sending requests.

## License

MIT
