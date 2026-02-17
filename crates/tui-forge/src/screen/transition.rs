//! Screen transition effects.
//!
//! Provides animated transitions between screens using buffer blending.
//! Supports fade (cross-dissolve) and slide (left/right) effects.

use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::Color;
use std::time::Instant;

/// A transition effect applied when navigating between screens.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Transition {
    /// No transition -- instant switch.
    #[default]
    None,
    /// Cross-fade between old and new screens.
    Fade { duration_ms: u64 },
    /// Old screen slides out left, new screen slides in from right.
    SlideLeft { duration_ms: u64 },
    /// Old screen slides out right, new screen slides in from left.
    SlideRight { duration_ms: u64 },
}

impl Transition {
    /// Default fade transition (200ms).
    pub const FADE: Self = Self::Fade { duration_ms: 200 };
    /// Default slide-left transition (200ms).
    pub const SLIDE_LEFT: Self = Self::SlideLeft { duration_ms: 200 };
    /// Default slide-right transition (200ms).
    pub const SLIDE_RIGHT: Self = Self::SlideRight { duration_ms: 200 };

    fn duration_ms(&self) -> u64 {
        match self {
            Self::None => 0,
            Self::Fade { duration_ms }
            | Self::SlideLeft { duration_ms }
            | Self::SlideRight { duration_ms } => *duration_ms,
        }
    }
}

/// Tracks the state of an in-progress transition.
pub(crate) struct TransitionState {
    pub transition: Transition,
    pub started_at: Instant,
    pub old_buffer: Buffer,
    pub area: Rect,
}

impl TransitionState {
    pub fn new(transition: Transition, old_buffer: Buffer, area: Rect) -> Self {
        Self {
            transition,
            started_at: Instant::now(),
            old_buffer,
            area,
        }
    }

    /// Returns progress from 0.0 (start) to 1.0 (complete).
    pub fn progress(&self) -> f32 {
        let duration_ms = self.transition.duration_ms();
        if duration_ms == 0 {
            return 1.0;
        }
        let elapsed = self.started_at.elapsed().as_millis() as f32;
        (elapsed / duration_ms as f32).min(1.0)
    }

    pub fn is_done(&self) -> bool {
        self.progress() >= 1.0
    }
}

/// Apply the transition effect by blending the old buffer snapshot with the
/// current (new) frame buffer contents.
pub(crate) fn apply_transition(buf: &mut Buffer, state: &TransitionState) {
    let t = state.progress();
    if t >= 1.0 {
        return;
    }

    let area = state.area;

    match state.transition {
        Transition::None => {}
        Transition::Fade { .. } => apply_fade(buf, &state.old_buffer, area, t),
        Transition::SlideLeft { .. } => apply_slide(buf, &state.old_buffer, area, t, true),
        Transition::SlideRight { .. } => apply_slide(buf, &state.old_buffer, area, t, false),
    }
}

/// Cross-fade: interpolate colors cell-by-cell between old and new buffers.
fn apply_fade(buf: &mut Buffer, old: &Buffer, area: Rect, t: f32) {
    for y in area.y..area.y + area.height {
        for x in area.x..area.x + area.width {
            let pos = Position { x, y };
            let old_cell = match old.cell(pos) {
                Some(c) => c,
                None => continue,
            };
            if let Some(new_cell) = buf.cell_mut(pos) {
                new_cell.fg = lerp_color(old_cell.fg, new_cell.fg, t);
                new_cell.bg = lerp_color(old_cell.bg, new_cell.bg, t);
                // Blend symbol: show old symbol in first half, new in second half
                if t < 0.5 {
                    new_cell.set_symbol(old_cell.symbol());
                }
            }
        }
    }
}

/// Slide: move old buffer out in one direction, new buffer in from the other.
fn apply_slide(buf: &mut Buffer, old: &Buffer, area: Rect, t: f32, left: bool) {
    let width = area.width as f32;
    let offset = (width * t) as u16;

    for y in area.y..area.y + area.height {
        for x in area.x..area.x + area.width {
            let rel_x = x - area.x;
            let pos = Position { x, y };

            if left {
                // Old content slides left: old_x = rel_x + offset
                // New content slides in from right: visible when rel_x >= width - offset
                if rel_x < area.width.saturating_sub(offset) {
                    // Still showing old content (shifted)
                    let old_x = area.x + rel_x + offset;
                    let old_pos = Position { x: old_x, y };
                    if let (Some(old_cell), Some(new_cell)) =
                        (old.cell(old_pos), buf.cell_mut(pos))
                    {
                        new_cell.set_symbol(old_cell.symbol());
                        new_cell.fg = old_cell.fg;
                        new_cell.bg = old_cell.bg;
                        new_cell.modifier = old_cell.modifier;
                    }
                }
                // else: new content is already in the buffer at this position
            } else {
                // Old content slides right: visible when rel_x >= offset
                // New content slides in from left: visible when rel_x < offset
                if rel_x >= offset {
                    // Still showing old content (shifted)
                    let old_x = area.x + rel_x - offset;
                    let old_pos = Position { x: old_x, y };
                    if let (Some(old_cell), Some(new_cell)) =
                        (old.cell(old_pos), buf.cell_mut(pos))
                    {
                        new_cell.set_symbol(old_cell.symbol());
                        new_cell.fg = old_cell.fg;
                        new_cell.bg = old_cell.bg;
                        new_cell.modifier = old_cell.modifier;
                    }
                }
                // else: new content is already in the buffer at this position
            }
        }
    }
}

/// Linearly interpolate between two colors.
///
/// For `Rgb` colors, each component is interpolated independently.
/// For non-RGB colors (named ANSI, Indexed), snaps to `to` at t >= 0.5.
fn lerp_color(from: Color, to: Color, t: f32) -> Color {
    if from == to {
        return to;
    }
    match (from, to) {
        (Color::Rgb(r1, g1, b1), Color::Rgb(r2, g2, b2)) => {
            Color::Rgb(lerp_u8(r1, r2, t), lerp_u8(g1, g2, t), lerp_u8(b1, b2, t))
        }
        _ => {
            if t >= 0.5 {
                to
            } else {
                from
            }
        }
    }
}

fn lerp_u8(a: u8, b: u8, t: f32) -> u8 {
    let result = a as f32 + (b as f32 - a as f32) * t;
    result.round() as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lerp_color_rgb() {
        let from = Color::Rgb(0, 0, 0);
        let to = Color::Rgb(100, 200, 50);
        let mid = lerp_color(from, to, 0.5);
        assert_eq!(mid, Color::Rgb(50, 100, 25));
    }

    #[test]
    fn lerp_color_same() {
        let c = Color::Rgb(42, 42, 42);
        assert_eq!(lerp_color(c, c, 0.5), c);
    }

    #[test]
    fn lerp_color_named_snaps() {
        assert_eq!(lerp_color(Color::Red, Color::Blue, 0.3), Color::Red);
        assert_eq!(lerp_color(Color::Red, Color::Blue, 0.5), Color::Blue);
        assert_eq!(lerp_color(Color::Red, Color::Blue, 0.8), Color::Blue);
    }

    #[test]
    fn lerp_u8_boundaries() {
        assert_eq!(lerp_u8(0, 255, 0.0), 0);
        assert_eq!(lerp_u8(0, 255, 1.0), 255);
        assert_eq!(lerp_u8(100, 200, 0.5), 150);
    }

    #[test]
    fn transition_state_progress() {
        let buf = Buffer::empty(Rect::new(0, 0, 10, 5));
        let state = TransitionState::new(
            Transition::Fade { duration_ms: 1000 },
            buf,
            Rect::new(0, 0, 10, 5),
        );
        assert!(state.progress() < 0.1);
        assert!(!state.is_done());
    }

    #[test]
    fn transition_none_is_instant() {
        let buf = Buffer::empty(Rect::new(0, 0, 10, 5));
        let state = TransitionState::new(Transition::None, buf, Rect::new(0, 0, 10, 5));
        assert_eq!(state.progress(), 1.0);
        assert!(state.is_done());
    }
}
