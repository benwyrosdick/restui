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
        self, DisableFocusChange, DisableMouseCapture, EnableFocusChange, EnableMouseCapture,
        Event, KeyEventKind, MouseButton, MouseEventKind,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io::{self, Write};
use std::panic;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

static TERMINAL_INITIALIZED: AtomicBool = AtomicBool::new(false);

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

#[tokio::main(flavor = "current_thread")]
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

    // Set up panic hook to restore terminal on panic
    let original_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        // Only restore if terminal was initialized
        if TERMINAL_INITIALIZED.load(Ordering::SeqCst) {
            restore_terminal();
        }
        // Call the original panic hook
        original_hook(panic_info);
    }));

    // Set up terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(
        stdout,
        EnterAlternateScreen,
        EnableMouseCapture,
        EnableFocusChange
    )?;
    TERMINAL_INITIALIZED.store(true, Ordering::SeqCst);
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app and run
    let mut app = App::new().await?;
    let result = run_app(&mut terminal, &mut app).await;

    // Restore terminal (also show cursor which restore_terminal doesn't do)
    restore_terminal();
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
    let mut skip_events_until_focus = false;

    loop {
        terminal.draw(|frame| ui::draw(frame, app))?;

        // Poll for events with a timeout to allow async operations
        match event::poll(Duration::from_millis(100)) {
            Ok(true) => {
                match event::read() {
                    Ok(evt) => {
                        // If we lost focus, skip all events until we regain it
                        // This helps avoid processing garbage data from corrupted terminal state
                        if skip_events_until_focus {
                            if matches!(evt, Event::FocusGained) {
                                skip_events_until_focus = false;
                                // Re-establish terminal state
                                let _ = execute!(
                                    terminal.backend_mut(),
                                    EnableMouseCapture,
                                    EnableFocusChange
                                );
                            }
                            continue;
                        }

                        match evt {
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
                                MouseEventKind::Drag(MouseButton::Left) => {
                                    app.handle_mouse_drag(mouse.column, mouse.row);
                                }
                                MouseEventKind::ScrollUp => {
                                    app.handle_scroll(mouse.column, mouse.row, true);
                                }
                                MouseEventKind::ScrollDown => {
                                    app.handle_scroll(mouse.column, mouse.row, false);
                                }
                                _ => {}
                            },
                            Event::FocusGained => {
                                // Re-establish terminal state when returning from screensaver/sleep
                                let _ = execute!(
                                    terminal.backend_mut(),
                                    EnableMouseCapture,
                                    EnableFocusChange
                                );
                            }
                            Event::FocusLost => {
                                // Terminal lost focus - skip events until we regain focus
                                // This helps avoid processing corrupted data after sleep/screensaver
                                skip_events_until_focus = true;
                            }
                            _ => {}
                        }
                    }
                    Err(_) => {
                        // Event read error - could be corrupted terminal state
                        // Try to recover by re-establishing terminal mode
                        let _ = execute!(
                            terminal.backend_mut(),
                            EnableMouseCapture,
                            EnableFocusChange
                        );
                    }
                }
            }
            Ok(false) => {
                // No events available, continue to tick
            }
            Err(_) => {
                // Poll error - try to continue
            }
        }

        // Process any pending async operations
        app.tick().await?;
    }
}

/// Restore terminal to normal state
/// This is called on panic and normal exit to ensure terminal is usable
fn restore_terminal() {
    let _ = disable_raw_mode();
    let _ = execute!(
        io::stdout(),
        LeaveAlternateScreen,
        DisableMouseCapture,
        DisableFocusChange
    );
    let _ = io::stdout().flush();
}
