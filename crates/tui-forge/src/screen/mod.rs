//! Generic screen trait, navigation types, and screen manager.
//!
//! This module provides:
//!
//! 1. A fully generic [`Screen`] trait for building multi-screen TUI applications
//! 2. A [`ScreenManager`] that handles navigation, history, and animated transitions
//! 3. [`Transition`] effects (fade, slide) for smooth screen changes
//!
//! The `Screen` trait is parameterized over a user-defined screen-id enum (`S`)
//! and an optional shared context type (`C`), with zero domain-specific variants.

pub mod manager;
pub mod transition;

use std::any::Any;

use anyhow::Result;
use crossterm::event::Event;
use ratatui::prelude::*;

pub use manager::ScreenManager;
pub use transition::Transition;

/// Screen identifier marker trait.
///
/// Users define their own enum (e.g. `enum AppScreen { Home, Settings }`)
/// and it automatically satisfies this bound as long as it derives the
/// required traits.
pub trait ScreenId: std::fmt::Debug + Clone + std::hash::Hash + Eq + 'static {}
impl<T: std::fmt::Debug + Clone + std::hash::Hash + Eq + 'static> ScreenId for T {}

/// Actions a screen can return after handling an event.
pub enum ScreenAction<S: ScreenId> {
    /// No action needed, stay on current screen.
    None,
    /// Navigate to a different screen.
    Navigate(S),
    /// Go back to the previous screen in history.
    Back,
    /// Request to quit the application.
    Quit,
    /// Arbitrary user-defined action, down-castable via `Any`.
    Custom(Box<dyn Any>),
}

/// The Screen trait -- implement for each screen in your app.
///
/// `S` is the screen-id type (any `Debug + Clone + Hash + Eq + 'static` enum).
/// `C` is the shared context type passed to every method (defaults to `()`).
///
/// # Example
///
/// ```rust,ignore
/// #[derive(Debug, Clone, Hash, Eq, PartialEq)]
/// enum MyScreen { Home, Settings }
///
/// struct HomeScreen { /* ... */ }
///
/// impl Screen<MyScreen> for HomeScreen {
///     fn render(&mut self, frame: &mut Frame, area: Rect, _ctx: &()) -> Result<()> {
///         // draw widgets
///         Ok(())
///     }
///
///     fn handle_event(&mut self, event: Event, _ctx: &()) -> Result<ScreenAction<MyScreen>> {
///         Ok(ScreenAction::None)
///     }
/// }
/// ```
pub trait Screen<S: ScreenId, C = ()> {
    /// Render the screen into the given area.
    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &C) -> Result<()>;

    /// Handle an input event and return the resulting action.
    fn handle_event(&mut self, event: Event, ctx: &C) -> Result<ScreenAction<S>>;

    /// Called when this screen becomes the active screen.
    fn on_enter(&mut self, _ctx: &C) -> Result<()> {
        Ok(())
    }

    /// Called when navigating away from this screen.
    fn on_exit(&mut self, _ctx: &C) -> Result<()> {
        Ok(())
    }

    /// Whether this screen currently owns keyboard focus (e.g. a form is active).
    ///
    /// When `true`, the [`ScreenManager`] reports this to the app so it can
    /// suppress global keybindings that would conflict with the screen's input.
    fn is_input_focused(&self) -> bool {
        false
    }
}
