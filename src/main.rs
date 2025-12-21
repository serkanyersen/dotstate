use anyhow::Result;

mod app;
mod cli;
mod components;
mod config;
mod dotfile_candidates;
mod file_manager;
mod git;
mod github;
mod tui;
mod ui;
mod utils;
mod widgets;

use app::App;
use cli::Cli;
use clap::Parser;

/// Set up panic hook to restore terminal state on panic
fn setup_panic_hook() {
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        // Restore terminal state before handling panic
        // This ensures the terminal is usable after a panic
        let _ = crossterm::terminal::disable_raw_mode();
        let _ = crossterm::execute!(
            std::io::stdout(),
            crossterm::terminal::LeaveAlternateScreen,
            crossterm::event::DisableMouseCapture
        );
        // Call the original panic hook to show the panic message
        original_hook(panic_info);
    }));
}

fn main() -> Result<()> {
    // Set up panic hook to restore terminal on panic
    setup_panic_hook();

    // Parse CLI arguments
    let cli = Cli::parse();

    // If a command was provided, execute it and exit (non-TUI mode)
    if cli.command.is_some() {
        return cli.execute();
    }

    // Otherwise, launch TUI
    // Set up logging directory
    let log_dir = dirs::cache_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_default())
        .join("dotstate");
    std::fs::create_dir_all(&log_dir)?;

    let log_file = log_dir.join("dotstate.log");

    // Initialize tracing with file logging
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

    // Write to file
    let file_appender = tracing_appender::rolling::never(&log_dir, "dotstate.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_writer(non_blocking)
        .with_ansi(false) // Disable ANSI colors in file
        .init();

    // Print log location before TUI starts (this will be visible briefly)
    eprintln!("Logs are being written to: {:?}", log_file);
    eprintln!("View logs in real-time: tail -f {:?}", log_file);

    let mut app = App::new()?;
    let result = app.run();

    // Restore terminal state on normal exit
    // (panic hook handles panics)
    drop(guard);

    result
}


