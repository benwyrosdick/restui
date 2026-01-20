#![allow(dead_code)]

mod app;
mod config;
mod filter;
mod http;
mod storage;
mod ui;

use anyhow::Result;
use app::App;
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind, MouseButton,
        MouseEventKind,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::time::Duration;

fn print_help() {
    println!("restui - A TUI API testing tool like Postman");
    println!();
    println!("USAGE:");
    println!("    restui [OPTIONS]");
    println!();
    println!("OPTIONS:");
    println!("    -h, --help       Print help information");
    println!("    -V, --version    Print version information");
}

fn print_version() {
    println!("restui {}", env!("CARGO_PKG_VERSION"));
}

#[tokio::main]
async fn main() -> Result<()> {
    // Handle command line arguments
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        match args[1].as_str() {
            "-h" | "--help" => {
                print_help();
                return Ok(());
            }
            "-V" | "--version" => {
                print_version();
                return Ok(());
            }
            arg => {
                eprintln!("Unknown argument: {}", arg);
                eprintln!("Use --help for usage information");
                std::process::exit(1);
            }
        }
    }
    // Set up logging (optional, for debugging)
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .with_writer(std::io::stderr)
        .init();

    // Set up terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app and run
    let mut app = App::new().await?;
    let result = run_app(&mut terminal, &mut app).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = result {
        eprintln!("Error: {err:?}");
    }

    Ok(())
}

async fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> Result<()> {
    loop {
        terminal.draw(|frame| ui::draw(frame, app))?;

        // Poll for events with a timeout to allow async operations
        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key) => {
                    if key.kind == KeyEventKind::Press {
                        match app.handle_key(key).await {
                            Ok(should_quit) => {
                                if should_quit {
                                    return Ok(());
                                }
                            }
                            Err(e) => {
                                app.set_error(format!("Error: {e}"));
                            }
                        }
                    }
                }
                Event::Mouse(mouse) => match mouse.kind {
                    MouseEventKind::Down(MouseButton::Left) => {
                        app.handle_mouse_click(mouse.column, mouse.row);
                    }
                    MouseEventKind::ScrollUp => {
                        app.handle_scroll(mouse.column, mouse.row, true);
                    }
                    MouseEventKind::ScrollDown => {
                        app.handle_scroll(mouse.column, mouse.row, false);
                    }
                    _ => {}
                },
                _ => {}
            }
        }

        // Process any pending async operations
        app.tick().await?;
    }
}
