//! Toast notification widget.
//!
//! A non-blocking notification that appears in the corner of the screen
//! and auto-closes after a configurable duration. Does not shift UI elements
//! or block user interactions.

use crate::styles::theme;
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Widget, Wrap};
use std::time::{Duration, Instant};

/// Toast notification variant for styling
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastVariant {
    /// Success notification (green)
    Success,
    /// Info notification (blue)
    Info,
    /// Warning notification (yellow)
    Warning,
    /// Error notification (red)
    Error,
}

impl ToastVariant {
    /// Get the icon for this variant
    #[must_use]
    pub fn icon(&self) -> &'static str {
        match self {
            ToastVariant::Success => "\u{2714}", // ✔
            ToastVariant::Info => "\u{2139}",    // ℹ
            ToastVariant::Warning => "\u{26A0}", // ⚠
            ToastVariant::Error => "\u{2718}",   // ✘
        }
    }

    /// Get the border color for this variant
    #[must_use]
    pub fn color(&self) -> ratatui::style::Color {
        let t = theme();
        match self {
            ToastVariant::Success => t.success,
            ToastVariant::Info => t.primary,
            ToastVariant::Warning => t.warning,
            ToastVariant::Error => t.error,
        }
    }
}

/// Toast notification data
#[derive(Debug, Clone)]
pub struct Toast {
    /// The message to display
    pub message: String,
    /// The variant (success, info, warning, error)
    pub variant: ToastVariant,
    /// When the toast was created
    pub created_at: Instant,
    /// How long to show the toast
    pub duration: Duration,
}

impl Toast {
    /// Create a new toast notification
    pub fn new(message: impl Into<String>, variant: ToastVariant) -> Self {
        Self {
            message: message.into(),
            variant,
            created_at: Instant::now(),
            duration: Duration::from_secs(3),
        }
    }

    /// Create a success toast
    pub fn success(message: impl Into<String>) -> Self {
        Self::new(message, ToastVariant::Success)
    }

    /// Create an info toast
    pub fn info(message: impl Into<String>) -> Self {
        Self::new(message, ToastVariant::Info)
    }

    /// Create a warning toast
    pub fn warning(message: impl Into<String>) -> Self {
        Self::new(message, ToastVariant::Warning)
    }

    /// Create an error toast
    pub fn error(message: impl Into<String>) -> Self {
        Self::new(message, ToastVariant::Error)
    }

    /// Set a custom duration
    #[must_use]
    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.duration = duration;
        self
    }

    /// Check if the toast has expired
    #[must_use]
    pub fn is_expired(&self) -> bool {
        self.created_at.elapsed() >= self.duration
    }
}

/// Toast widget for rendering a toast notification
///
/// Renders in the bottom-right corner of the given area.
pub struct ToastWidget<'a> {
    toast: &'a Toast,
}

impl<'a> ToastWidget<'a> {
    /// Create a new toast widget
    #[must_use]
    pub fn new(toast: &'a Toast) -> Self {
        Self { toast }
    }

    /// Calculate the toast area (bottom-right corner)
    fn calculate_area(&self, area: Rect) -> Rect {
        let toast_width = 40u16.min(area.width.saturating_sub(4));
        let toast_height = 3u16;

        // Position in bottom-right corner with some padding
        let x = area.x + area.width.saturating_sub(toast_width + 2);
        let y = area.y + area.height.saturating_sub(toast_height + 3); // Above footer

        Rect::new(x, y, toast_width, toast_height)
    }
}

impl Widget for ToastWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let toast_area = self.calculate_area(area);
        let t = theme();

        // Clear the background
        Widget::render(Clear, toast_area, buf);

        // Create the message with icon
        let icon = self.toast.variant.icon();
        let message = format!(" {} {} ", icon, self.toast.message);

        // Create the block with colored border
        let color = self.toast.variant.color();
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(color))
            .style(Style::default().bg(t.background));

        // Create the paragraph
        let paragraph = Paragraph::new(message)
            .block(block)
            .style(Style::default().fg(t.text).add_modifier(Modifier::BOLD))
            .alignment(Alignment::Left)
            .wrap(Wrap { trim: true });

        Widget::render(paragraph, toast_area, buf);
    }
}

/// Toast manager for handling toast state
#[derive(Debug, Default)]
pub struct ToastManager {
    /// Current active toast (only one at a time)
    current: Option<Toast>,
}

impl ToastManager {
    /// Create a new toast manager
    #[must_use]
    pub fn new() -> Self {
        Self { current: None }
    }

    /// Add a toast, replacing any existing toast
    pub fn push(&mut self, toast: Toast) {
        self.current = Some(toast);
    }

    /// Add a success toast
    pub fn success(&mut self, message: impl Into<String>) {
        self.push(Toast::success(message));
    }

    /// Add an info toast
    pub fn info(&mut self, message: impl Into<String>) {
        self.push(Toast::info(message));
    }

    /// Add a warning toast
    pub fn warning(&mut self, message: impl Into<String>) {
        self.push(Toast::warning(message));
    }

    /// Add an error toast
    pub fn error(&mut self, message: impl Into<String>) {
        self.push(Toast::error(message));
    }

    /// Remove expired toasts and return whether any are still active
    pub fn tick(&mut self) -> bool {
        if let Some(ref toast) = self.current {
            if toast.is_expired() {
                self.current = None;
            }
        }
        self.current.is_some()
    }

    /// Get the current toast to display (if any)
    #[must_use]
    pub fn current(&self) -> Option<&Toast> {
        self.current.as_ref()
    }

    /// Check if there are any active toasts
    #[must_use]
    pub fn has_toast(&self) -> bool {
        self.current.is_some()
    }

    /// Render the current toast (if any) using Frame
    pub fn render(&self, frame: &mut ratatui::Frame, area: Rect) {
        if let Some(toast) = self.current() {
            frame.render_widget(ToastWidget::new(toast), area);
        }
    }

    /// Clear all toasts
    pub fn clear(&mut self) {
        self.current = None;
    }
}
