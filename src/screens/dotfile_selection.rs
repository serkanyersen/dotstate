//! Dotfile selection screen controller.
//!
//! This screen handles adding/removing files to sync with the repository.
//! Note: Event handling is still in app.rs (~700 lines).

use crate::components::DotfileSelectionComponent;
use crate::config::Config;
use crate::screens::screen_trait::{RenderContext, Screen, ScreenAction, ScreenContext};
use crate::ui::UiState;
use anyhow::Result;
use crossterm::event::Event;
use ratatui::layout::Rect;
use ratatui::Frame;
use syntect::highlighting::Theme;
use syntect::parsing::SyntaxSet;

/// Dotfile selection screen controller.
///
/// # Migration Status
///
/// Event handling is still in app.rs (lines ~1063-1700+).
/// This screen wraps DotfileSelectionComponent which uses UiState directly.
pub struct DotfileSelectionScreen {
    component: DotfileSelectionComponent,
}

impl DotfileSelectionScreen {
    /// Create a new dotfile selection screen.
    pub fn new() -> Self {
        Self {
            component: DotfileSelectionComponent::new(),
        }
    }

    /// Render with all required context and state.
    pub fn render_with_state(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        ui_state: &mut UiState,
        config: &Config,
        syntax_set: &SyntaxSet,
        theme: &Theme,
    ) -> Result<()> {
        self.component.render_with_state(frame, area, ui_state, config, syntax_set, theme)
    }
}

impl Default for DotfileSelectionScreen {
    fn default() -> Self {
        Self::new()
    }
}

impl Screen for DotfileSelectionScreen {
    fn render(&mut self, _frame: &mut Frame, _area: Rect, _ctx: &RenderContext) -> Result<()> {
        // Note: Use render_with_state instead as this screen uses UiState directly
        Ok(())
    }

    fn handle_event(&mut self, _event: Event, _ctx: &ScreenContext) -> Result<ScreenAction> {
        // TODO: Move event handling from app.rs here
        // Currently handled in app.rs (handle_event, Screen::DotfileSelection case)
        Ok(ScreenAction::None)
    }

    fn is_input_focused(&self) -> bool {
        // TODO: Check UiState when this becomes the owner
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dotfile_selection_screen_creation() {
        let screen = DotfileSelectionScreen::new();
        // Just test that it can be created
        assert!(!screen.is_input_focused());
    }
}
