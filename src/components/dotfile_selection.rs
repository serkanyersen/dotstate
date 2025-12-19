use anyhow::Result;
use crossterm::event::Event;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Clear};
use crate::components::component::{Component, ComponentAction};

/// Dotfile selection component
/// Note: Event handling is done in app.rs due to complex state dependencies
/// This component handles rendering with Clear widget and can be extended with mouse support
pub struct DotfileSelectionComponent;

impl DotfileSelectionComponent {
    pub fn new() -> Self {
        Self
    }
}

impl Component for DotfileSelectionComponent {
    fn render(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
        // Clear the entire area first to prevent background bleed-through
        frame.render_widget(Clear, area);

        // Background
        let background = Block::default()
            .style(Style::default().bg(Color::Black));
        frame.render_widget(background, area);

        // The actual rendering is done by the existing render_dotfile_selection function
        // This component wrapper ensures Clear is called first
        // We'll call the render function from app.rs after syncing state
        Ok(())
    }

    fn handle_event(&mut self, _event: Event) -> Result<ComponentAction> {
        // Event handling is done in app.rs due to complex dependencies
        // Mouse support can be added here incrementally
        Ok(ComponentAction::None)
    }

}
