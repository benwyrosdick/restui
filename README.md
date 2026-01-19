# restui

A terminal-based API testing tool built with Rust and Ratatui. Similar to Postman/Posting, but runs entirely in your terminal.

## Features

- **HTTP Methods**: GET, POST, PUT, DELETE
- **Request Editor**: Tabs for Headers, Body, Auth, and Query Params
- **Authentication**: Bearer token, Basic auth, API Key (header or query param)
- **Environment Variables**: Postman-style `{{variable}}` interpolation
- **Collections**: Organize requests in collections and folders with full CRUD support
- **Request History**: Track previously sent requests
- **Response Viewer**: JSON syntax highlighting with scroll wheel support
- **Clipboard**: Copy requests as curl commands or copy response body
- **Mouse Support**: Click on panels, tabs, and request list items
- **Keyboard Navigation**: Vim-style (j/k/h/l) and arrow keys

## Installation

### Homebrew (macOS/Linux)

```bash
brew install benwyrosdick/tap/restui
```

### From Source

```bash
cargo build --release
./target/release/restui
```

## Usage

### Keyboard Shortcuts

#### Global

| Key | Action |
|-----|--------|
| `Tab` | Cycle between panels |
| `Shift+Tab` | Cycle panels (reverse) |
| `1/2/3/4` | Jump to panel (Collections/URL/Request/Response) |
| `j/k` or `↑/↓` | Navigate up/down |
| `h/l` or `←/→` | Switch tabs (in Request Editor) |
| `Enter` or `i` | Start editing |
| `Esc` | Exit edit mode |
| `s` | Send request |
| `m` | Cycle HTTP method |
| `n` | New request |
| `e` | Switch environment |
| `E` | Reload environments from file |
| `H` | Toggle history view |
| `y` | Copy request as curl command |
| `W` | Save changes to collection |
| `?` | Show help |
| `q` | Quit |

#### Editing Mode

| Key | Action |
|-----|--------|
| `←/→` | Move cursor left/right |
| `Home` | Move cursor to start |
| `End` | Move cursor to end |
| `Delete` | Delete character at cursor |
| `Backspace` | Delete character before cursor |
| `Tab` | Next field |
| `Esc` | Exit edit mode |

#### Collection Management (in Request List)

| Key | Action |
|-----|--------|
| `C` | Create new collection |
| `F` | Create folder in selected location |
| `r` | Create request in selected location |
| `R` | Rename selected item |
| `d` | Delete selected item |

#### Response View

| Key | Action |
|-----|--------|
| `j/k` or `↑/↓` | Scroll response |
| `c` | Copy response to clipboard |
| Scroll wheel | Scroll response |

#### Auth Tab

| Key | Action |
|-----|--------|
| `a` | Cycle auth type (None → Bearer → Basic → API Key) |

### Mouse Support

- Click on any panel to focus it
- Click on tabs (Headers/Body/Auth/Params) to switch
- Click on requests in the list to select them
- Click on URL bar to start editing
- Scroll wheel to scroll response content

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

Create `~/.config/restui/environments.json`:

```json
{
  "active_index": 0,
  "environments": [
    {
      "name": "development",
      "color": "green",
      "variables": {
        "base_url": "http://localhost:3000",
        "api_token": "dev-token-123"
      }
    },
    {
      "name": "production",
      "color": "red",
      "variables": {
        "base_url": "https://api.example.com",
        "api_token": "prod-token-456"
      }
    }
  ]
}
```

Use `{{variable_name}}` syntax in URLs, headers, or body. Variables are interpolated from the active environment before sending requests.

Press `e` to cycle through environments, or `E` to reload from file after making changes.

### Environment Colors

Set the `color` field to change the header bar color for each environment. This helps visually distinguish between environments (e.g., red for production).

Supported colors:
- Named: `red`, `green`, `blue`, `yellow`, `magenta`, `cyan`, `white`, `gray`, `darkgray`
- Light variants: `lightred`, `lightgreen`, `lightblue`, `lightyellow`, `lightmagenta`, `lightcyan`
- Hex: `#FF5733` (RGB hex format)

## License

MIT
