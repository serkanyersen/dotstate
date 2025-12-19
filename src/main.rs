use anyhow::Result;

mod app;
mod components;
mod config;
mod file_manager;
mod git;
mod github;
mod tui;
mod ui;
mod utils;
mod widgets;

use app::App;

fn main() -> Result<()> {
    // Set up logging directory
    let log_dir = dirs::cache_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_default())
        .join("dotzz");
    std::fs::create_dir_all(&log_dir)?;

    let log_file = log_dir.join("dotzz.log");

    // Initialize tracing with file logging
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

    // Write to file
    let file_appender = tracing_appender::rolling::never(&log_dir, "dotzz.log");
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
    app.run()?;

    // Keep guard alive until program ends
    drop(guard);

    Ok(())
}


