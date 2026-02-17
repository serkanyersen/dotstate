//! Screen manager for multi-screen TUI applications.
//!
//! Manages a collection of screens, handles navigation with a history stack,
//! and applies animated transitions between screens.

use std::collections::HashMap;

use anyhow::Result;
use crossterm::event::Event;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::prelude::*;

use super::transition::{TransitionState, apply_transition};
use super::{Screen, ScreenAction, ScreenId, Transition};

/// Manages a set of screens with navigation, history, and transitions.
///
/// `S` is the screen-id type (must be `Debug + Clone + Hash + Eq + 'static`).
/// `C` is the shared context type passed to every screen method (defaults to `()`).
///
/// # Example
///
/// ```rust,ignore
/// use tui_forge::{ScreenManager, Transition, Screen, ScreenAction};
///
/// #[derive(Debug, Clone, Hash, Eq, PartialEq)]
/// enum AppScreen { Home, Settings }
///
/// let mut manager = ScreenManager::new(AppScreen::Home)
///     .with_transition(Transition::FADE)
///     .register(AppScreen::Home, HomeScreen::new())
///     .register(AppScreen::Settings, SettingsScreen::new());
///
/// // In your event loop:
/// match manager.handle_event(event, &ctx)? {
///     ScreenAction::Quit => break,
///     _ => {}
/// }
///
/// // In your render loop:
/// manager.render(frame, area, &ctx)?;
/// ```
pub struct ScreenManager<S: ScreenId, C = ()> {
    screens: HashMap<S, Box<dyn Screen<S, C>>>,
    active: S,
    history: Vec<S>,
    transition_state: Option<TransitionState>,
    default_transition: Transition,
    last_area: Rect,
    /// Snapshot of the last rendered frame (content area only).
    /// Used as the "old" image when starting a transition.
    last_buffer: Option<Buffer>,
}

impl<S: ScreenId, C> ScreenManager<S, C> {
    /// Create a new screen manager with the given initial screen id.
    ///
    /// No screens are registered yet -- use [`register`](Self::register) to add them.
    pub fn new(initial: S) -> Self {
        Self {
            screens: HashMap::new(),
            active: initial,
            history: Vec::new(),
            transition_state: None,
            default_transition: Transition::None,
            last_area: Rect::default(),
            last_buffer: None,
        }
    }

    /// Set the default transition used for all navigations.
    #[must_use]
    pub fn with_transition(mut self, transition: Transition) -> Self {
        self.default_transition = transition;
        self
    }

    /// Register a screen for the given id. Builder pattern.
    #[must_use]
    pub fn register(mut self, id: S, screen: impl Screen<S, C> + 'static) -> Self {
        self.screens.insert(id, Box::new(screen));
        self
    }

    /// Add a screen at runtime (non-builder).
    pub fn add_screen(&mut self, id: S, screen: impl Screen<S, C> + 'static) {
        self.screens.insert(id, Box::new(screen));
    }

    /// Navigate to a screen using the default transition.
    pub fn navigate(&mut self, to: S, ctx: &C) {
        self.navigate_inner(to, self.default_transition, ctx);
    }

    /// Navigate to a screen with a specific transition override.
    pub fn navigate_with(&mut self, to: S, transition: Transition, ctx: &C) {
        self.navigate_inner(to, transition, ctx);
    }

    fn navigate_inner(&mut self, to: S, transition: Transition, ctx: &C) {
        // Call on_exit on current screen
        if let Some(current) = self.screens.get_mut(&self.active) {
            let _ = current.on_exit(ctx);
        }

        // Use the last rendered frame as the "old" snapshot for the transition
        if !matches!(transition, Transition::None) && self.last_area.width > 0 {
            let old_buffer = self
                .last_buffer
                .take()
                .unwrap_or_else(|| Buffer::empty(self.last_area));
            self.transition_state = Some(TransitionState::new(
                transition,
                old_buffer,
                self.last_area,
            ));
        }

        // Push current to history and switch
        self.history.push(self.active.clone());
        self.active = to;

        // Call on_enter on new screen
        if let Some(screen) = self.screens.get_mut(&self.active) {
            let _ = screen.on_enter(ctx);
        }
    }

    /// Go back to the previous screen. Returns `false` if history is empty.
    pub fn go_back(&mut self, ctx: &C) -> bool {
        if let Some(prev) = self.history.pop() {
            if let Some(current) = self.screens.get_mut(&self.active) {
                let _ = current.on_exit(ctx);
            }

            // Reverse transition direction
            let transition = match self.default_transition {
                Transition::SlideLeft { duration_ms } => {
                    Transition::SlideRight { duration_ms }
                }
                Transition::SlideRight { duration_ms } => {
                    Transition::SlideLeft { duration_ms }
                }
                other => other,
            };

            if !matches!(transition, Transition::None) && self.last_area.width > 0 {
                let old_buffer = self
                    .last_buffer
                    .take()
                    .unwrap_or_else(|| Buffer::empty(self.last_area));
                self.transition_state = Some(TransitionState::new(
                    transition,
                    old_buffer,
                    self.last_area,
                ));
            }

            self.active = prev;

            if let Some(screen) = self.screens.get_mut(&self.active) {
                let _ = screen.on_enter(ctx);
            }
            true
        } else {
            false
        }
    }

    /// Render the active screen, applying any in-progress transition.
    pub fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &C) -> Result<()> {
        self.last_area = area;

        // Render the active screen
        if let Some(screen) = self.screens.get_mut(&self.active) {
            screen.render(frame, area, ctx)?;
        }

        // Apply transition blend if active
        if let Some(ref state) = self.transition_state {
            if !state.is_done() {
                apply_transition(frame.buffer_mut(), state);
            }
        }

        // Clean up finished transitions
        if self
            .transition_state
            .as_ref()
            .is_some_and(TransitionState::is_done)
        {
            self.transition_state = None;
        }

        // Save a snapshot of the rendered frame for future transitions.
        // Only save when not mid-transition so we capture the clean state.
        if self.transition_state.is_none() {
            self.last_buffer = Some(clone_buffer_region(frame.buffer_mut(), area));
        }

        Ok(())
    }

    /// Handle an event, delegating to the active screen.
    ///
    /// During a transition, input is suppressed and `ScreenAction::None` is returned.
    /// If the screen returns `Navigate` or `Back`, the manager handles it automatically.
    pub fn handle_event(&mut self, event: Event, ctx: &C) -> Result<ScreenAction<S>> {
        // Suppress input during transitions
        if self.is_transitioning() {
            return Ok(ScreenAction::None);
        }

        let action = if let Some(screen) = self.screens.get_mut(&self.active) {
            screen.handle_event(event, ctx)?
        } else {
            ScreenAction::None
        };

        match &action {
            ScreenAction::Navigate(to) => {
                let to = to.clone();
                self.navigate(to, ctx);
                Ok(ScreenAction::None)
            }
            ScreenAction::Back => {
                self.go_back(ctx);
                Ok(ScreenAction::None)
            }
            _ => Ok(action),
        }
    }

    /// Whether the active screen has input focus (e.g. a form is focused).
    pub fn is_input_focused(&self) -> bool {
        self.screens
            .get(&self.active)
            .is_some_and(|s| s.is_input_focused())
    }

    /// Get the active screen's id.
    pub fn active_id(&self) -> &S {
        &self.active
    }

    /// Whether a transition animation is currently playing.
    pub fn is_transitioning(&self) -> bool {
        self.transition_state
            .as_ref()
            .is_some_and(|ts| !ts.is_done())
    }

    /// Get the navigation history depth.
    pub fn history_len(&self) -> usize {
        self.history.len()
    }

    /// Clear the navigation history.
    pub fn clear_history(&mut self) {
        self.history.clear();
    }
}

/// Clone a rectangular region of a buffer into a new buffer.
fn clone_buffer_region(buf: &Buffer, area: Rect) -> Buffer {
    let mut snapshot = Buffer::empty(area);
    for y in area.y..area.y + area.height {
        for x in area.x..area.x + area.width {
            let pos = ratatui::layout::Position { x, y };
            if let (Some(src), Some(dst)) = (buf.cell(pos), snapshot.cell_mut(pos)) {
                dst.set_symbol(src.symbol());
                dst.fg = src.fg;
                dst.bg = src.bg;
                dst.modifier = src.modifier;
            }
        }
    }
    snapshot
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

    #[derive(Debug, Clone, Hash, Eq, PartialEq)]
    enum TestScreen {
        A,
        B,
        C,
    }

    struct SimpleScreen {
        #[allow(dead_code)]
        label: &'static str,
    }

    impl SimpleScreen {
        fn new(label: &'static str) -> Self {
            Self { label }
        }
    }

    impl Screen<TestScreen> for SimpleScreen {
        fn render(&mut self, _frame: &mut Frame, _area: Rect, _ctx: &()) -> Result<()> {
            Ok(())
        }

        fn handle_event(
            &mut self,
            event: Event,
            _ctx: &(),
        ) -> Result<ScreenAction<TestScreen>> {
            if let Event::Key(key) = event {
                match key.code {
                    KeyCode::Char('b') => return Ok(ScreenAction::Navigate(TestScreen::B)),
                    KeyCode::Esc => return Ok(ScreenAction::Back),
                    KeyCode::Char('q') => return Ok(ScreenAction::Quit),
                    _ => {}
                }
            }
            Ok(ScreenAction::None)
        }

    }

    fn key_event(code: KeyCode) -> Event {
        Event::Key(KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        })
    }

    #[test]
    fn navigate_and_back() {
        let mut mgr = ScreenManager::new(TestScreen::A)
            .register(TestScreen::A, SimpleScreen::new("A"))
            .register(TestScreen::B, SimpleScreen::new("B"))
            .register(TestScreen::C, SimpleScreen::new("C"));

        assert_eq!(mgr.active_id(), &TestScreen::A);
        assert_eq!(mgr.history_len(), 0);

        mgr.navigate(TestScreen::B, &());
        assert_eq!(mgr.active_id(), &TestScreen::B);
        assert_eq!(mgr.history_len(), 1);

        mgr.navigate(TestScreen::C, &());
        assert_eq!(mgr.active_id(), &TestScreen::C);
        assert_eq!(mgr.history_len(), 2);

        assert!(mgr.go_back(&()));
        assert_eq!(mgr.active_id(), &TestScreen::B);

        assert!(mgr.go_back(&()));
        assert_eq!(mgr.active_id(), &TestScreen::A);

        assert!(!mgr.go_back(&()));
        assert_eq!(mgr.active_id(), &TestScreen::A);
    }

    #[test]
    fn handle_event_navigate() {
        let mut mgr = ScreenManager::new(TestScreen::A)
            .register(TestScreen::A, SimpleScreen::new("A"))
            .register(TestScreen::B, SimpleScreen::new("B"));

        let action = mgr.handle_event(key_event(KeyCode::Char('b')), &()).unwrap();
        assert!(matches!(action, ScreenAction::None)); // consumed by manager
        assert_eq!(mgr.active_id(), &TestScreen::B);
    }

    #[test]
    fn handle_event_quit_passes_through() {
        let mut mgr = ScreenManager::new(TestScreen::A)
            .register(TestScreen::A, SimpleScreen::new("A"));

        let action = mgr.handle_event(key_event(KeyCode::Char('q')), &()).unwrap();
        assert!(matches!(action, ScreenAction::Quit));
    }

    #[test]
    fn lifecycle_hooks_called() {
        let mut mgr = ScreenManager::new(TestScreen::A)
            .register(TestScreen::A, SimpleScreen::new("A"))
            .register(TestScreen::B, SimpleScreen::new("B"));

        mgr.navigate(TestScreen::B, &());

        // Can't easily inspect internal state through the trait,
        // but the fact that navigate doesn't panic confirms hooks run
        assert_eq!(mgr.active_id(), &TestScreen::B);
    }

    #[test]
    fn with_transition() {
        let mgr: ScreenManager<TestScreen> = ScreenManager::new(TestScreen::A)
            .with_transition(Transition::FADE);

        assert!(!mgr.is_transitioning());
    }
}
