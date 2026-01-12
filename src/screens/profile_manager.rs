//! Profile manager screen controller.
//!
//! This screen handles profile CRUD operations (create, switch, rename, delete).
//! Note: Event handling is still in app.rs (~400 lines).

use crate::components::profile_manager::{ProfileManagerComponent, ProfileManagerState};
use crate::config::Config;
use crate::screens::screen_trait::{RenderContext, Screen, ScreenAction, ScreenContext};
use crate::utils::ProfileInfo;
use anyhow::Result;
use crossterm::event::Event;
use ratatui::layout::Rect;
use ratatui::Frame;

/// Profile manager screen controller.
///
/// # Migration Status
///
/// Event handling is still in app.rs (lines ~2421-2800+).
/// This screen wraps ProfileManagerComponent.
pub struct ProfileManagerScreen {
    component: ProfileManagerComponent,
}

impl ProfileManagerScreen {
    /// Create a new profile manager screen.
    pub fn new() -> Self {
        Self {
            component: ProfileManagerComponent::new(),
        }
    }

    /// Render with all required context and state.
    pub fn render_with_config(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        config: &Config,
        profiles: &[ProfileInfo],
        state: &mut ProfileManagerState,
    ) -> Result<()> {
        self.component.render_with_config(frame, area, config, profiles, state)
    }
}

impl Default for ProfileManagerScreen {
    fn default() -> Self {
        Self::new()
    }
}

impl Screen for ProfileManagerScreen {
    fn render(&mut self, _frame: &mut Frame, _area: Rect, _ctx: &RenderContext) -> Result<()> {
        // Note: Use render_with_config instead as this screen needs profiles and state
        Ok(())
    }

    fn handle_event(&mut self, _event: Event, _ctx: &ScreenContext) -> Result<ScreenAction> {
        // TODO: Move event handling from app.rs here
        // Currently handled in app.rs (handle_event, Screen::ManageProfiles case)
        Ok(ScreenAction::None)
    }

    fn is_input_focused(&self) -> bool {
        // TODO: Check state when this becomes the owner
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profile_manager_screen_creation() {
        let screen = ProfileManagerScreen::new();
        // Just test that it can be created
        assert!(!screen.is_input_focused());
    }
}
