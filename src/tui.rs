use anyhow::Result;
use crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io::stdout;
use std::time::Duration;

/// Terminal UI manager
pub struct Tui {
    terminal: Terminal<CrosstermBackend<std::io::Stdout>>,
}

impl Tui {
    pub fn new() -> Result<Self> {
        let backend = CrosstermBackend::new(stdout());
        let terminal = Terminal::new(backend)?;

        Ok(Self { terminal })
    }

    /// Enter alternate screen and enable raw mode
    pub fn enter(&mut self) -> Result<()> {
        enable_raw_mode()?;
        execute!(stdout(), EnterAlternateScreen, EnableMouseCapture)?;
        Ok(())
    }

    /// Exit alternate screen and disable raw mode
    pub fn exit(&mut self) -> Result<()> {
        disable_raw_mode()?;
        execute!(stdout(), LeaveAlternateScreen, DisableMouseCapture)?;
        Ok(())
    }

    /// Get mutable reference to terminal
    pub fn terminal_mut(&mut self) -> &mut Terminal<CrosstermBackend<std::io::Stdout>> {
        &mut self.terminal
    }

    /// Poll for events with timeout
    pub fn poll_event(&self, timeout: Duration) -> Result<Option<Event>> {
        if event::poll(timeout)? {
            Ok(Some(event::read()?))
        } else {
            Ok(None)
        }
    }
}

impl Drop for Tui {
    fn drop(&mut self) {
        let _ = self.exit();
    }
}
