use anyhow::Result;

mod app;
mod cli;
mod components;
mod config;
mod dotfile_candidates;
mod file_manager;
mod git;
mod github;
mod styles;
mod tui;
mod ui;
mod utils;
mod version_check;
mod widgets;

use app::App;
use clap::Parser;
use cli::Cli;

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
        // Set up logging for CLI mode
        let log_dir = dirs::cache_dir()
            .unwrap_or_else(|| dirs::home_dir().unwrap_or_default())
            .join("dotstate");
        std::fs::create_dir_all(&log_dir)?;

        let filter = tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

        let file_appender = tracing_appender::rolling::never(&log_dir, "dotstate.log");
        let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_target(false)
            .with_writer(non_blocking)
            .with_ansi(false)
            .init();

        use tracing::info;
        info!("Starting dotstate CLI mode");
        let result = cli.execute();
        drop(guard);
        return result;
    }

    // Otherwise, launch TUI
    // Set up logging directory
    let log_dir = dirs::cache_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_default())
        .join("dotstate");
    std::fs::create_dir_all(&log_dir)?;

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

    use tracing::info;
    info!("Starting dotstate TUI mode");
    info!("Log directory: {:?}", log_dir);

    // Load config to get theme preference
    let config_path = crate::utils::get_config_path();
    let config = config::Config::load_or_create(&config_path)?;

    // Determine whether colors should be disabled (NO_COLOR env var, --no-colors flag, or theme=nocolor)
    let env_no_color = std::env::var_os("NO_COLOR").is_some();
    let config_theme_type = styles::ThemeType::from_str(&config.theme);
    let no_colors =
        cli.no_colors || env_no_color || config_theme_type == styles::ThemeType::NoColor;

    // If any source disables colors, set NO_COLOR so crossterm/ratatui respects it.
    if no_colors {
        std::env::set_var("NO_COLOR", "1");
        info!("Colors disabled via NO_COLOR env var, --no-colors flag, or theme=nocolor");
    }

    // Initialize theme based on config, but force NoColor when requested.
    let theme_type = if no_colors {
        styles::ThemeType::NoColor
    } else {
        config_theme_type
    };
    styles::init_theme(theme_type);
    info!("Theme initialized: {:?}", theme_type);

    let mut app = App::new()?;
    let result = app.run();

    info!("Shutting down dotstate");

    // Restore terminal state on normal exit
    // (panic hook handles panics)
    drop(guard);

    result
}
