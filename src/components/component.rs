use anyhow::Result;
use crossterm::event::Event;
use ratatui::prelude::*;
use crate::ui::Screen;

/// Action that a component can return after handling an event
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComponentAction {
    /// No action needed
    None,
    /// Navigate to a different screen
    Navigate(Screen),
    /// Quit the application
    Quit,
    /// Component state was updated, needs re-render
    Update,
}

/// Trait for all UI components
///
/// Components are self-contained UI elements that:
/// - Manage their own state
/// - Handle their own events
/// - Render themselves
/// - Return actions for the app to handle
pub trait Component {
    /// Render the component to the given area
    fn render(&mut self, frame: &mut Frame, area: Rect) -> Result<()>;

    /// Handle an event (keyboard, mouse, etc.)
    /// Returns an action that the app should take
    fn handle_event(&mut self, event: Event) -> Result<ComponentAction>;
}
