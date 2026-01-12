//! Package manager screen controller.
//!
//! This screen handles package CRUD operations (add, edit, delete, install).
//! Note: Event handling is still in app.rs (~500 lines).

use crate::components::package_manager::PackageManagerComponent;
use crate::config::Config;
use crate::screens::screen_trait::{RenderContext, Screen, ScreenAction, ScreenContext};
use crate::ui::PackageManagerState;
use crate::utils::profile_manifest::Package;
use anyhow::Result;
use crossterm::event::Event;
use ratatui::layout::Rect;
use ratatui::Frame;

/// Package manager screen controller.
///
/// # Migration Status
///
/// Event handling is still in app.rs (lines ~1918-2400+).
/// This screen wraps PackageManagerComponent.
pub struct PackageManagerScreen {
    component: PackageManagerComponent,
}

impl PackageManagerScreen {
    /// Create a new package manager screen.
    pub fn new() -> Self {
        Self {
            component: PackageManagerComponent::new(),
        }
    }

    /// Render with all required context and state.
    pub fn render_with_state(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        state: &mut PackageManagerState,
        config: &Config,
        packages: &[Package],
    ) -> Result<()> {
        self.component.render_with_state(frame, area, state, config, packages)
    }
}

impl Default for PackageManagerScreen {
    fn default() -> Self {
        Self::new()
    }
}

impl Screen for PackageManagerScreen {
    fn render(&mut self, _frame: &mut Frame, _area: Rect, _ctx: &RenderContext) -> Result<()> {
        // Note: Use render_with_state instead as this screen needs state and packages
        Ok(())
    }

    fn handle_event(&mut self, _event: Event, _ctx: &ScreenContext) -> Result<ScreenAction> {
        // TODO: Move event handling from app.rs here
        // Currently handled in app.rs (handle_event, Screen::ManagePackages case)
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
    fn test_package_manager_screen_creation() {
        let screen = PackageManagerScreen::new();
        // Just test that it can be created
        assert!(!screen.is_input_focused());
    }
}
