//! # tui-forge
//!
//! Batteries-included toolkit for building [ratatui](https://ratatui.rs) TUI applications.
//! Provides themes, forms, widgets, keymap presets, layout helpers, and navigation utilities
//! so you can focus on your application logic instead of UI boilerplate.
//!
//! ## Features
//!
//! - **12 built-in themes** with runtime switching and custom palette support
//! - **Form system** with 6 field types, validation, and keymap conflict resolution
//! - **Widgets**: `Popup`, `Dialog`, `Toast`, `Menu`, `Header`, `Footer`, `HelpOverlay`
//! - **Keymap presets** (Standard / Vim / Emacs) with per-action overrides (requires `keymap` feature)
//! - **Layout helpers**: `create_standard_layout`, `create_split_layout`, `center_popup`
//! - **Navigation**: `ListStateExt` trait, `MouseRegions<T>` click tracker
//! - **Screen manager** with animated transitions, navigation history, and lifecycle hooks
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use tui_forge::{init_theme, ThemeType, Header, Footer, create_standard_layout};
//!
//! init_theme(ThemeType::CatppuccinMocha);
//!
//! // In your render closure:
//! let (header, content, footer) = create_standard_layout(frame.area(), 6, 2);
//! frame.render_widget(Header::new("My App").subtitle("v0.1"), header);
//! frame.render_widget(Footer::new("Quit: q"), footer);
//! ```
//!
//! See the [README](https://github.com/user/tui-forge) for full documentation and examples.

// Core
pub mod icons;
pub mod layout;
pub mod mouse;
pub mod theme;

// Widgets
pub mod widgets;

// Input handling
pub mod input;

// Form system
pub mod form;

// Keymap (behind feature flag, default on)
#[cfg(feature = "keymap")]
pub mod keymap;

// Screen architecture
pub mod screen;

// Components
pub mod components;

// Convenience re-exports at crate root
pub use icons::{IconSet, Icons};
pub use input::list_nav::{ListStateExt, DEFAULT_PAGE_SIZE};
pub use layout::{center_popup, create_split_layout, create_standard_layout};
pub use mouse::MouseRegions;
pub use theme::{init_theme, set_custom_theme, theme, Theme, ThemeType};

pub use form::fields::{Checkbox, Password, RadioBox, TextArea, TextInput, ToggleSwitch};
pub use form::validators;
pub use form::{FieldConfig, FieldValue, Form, FormAction, FormLayout, FormValues, ValidateOn};

#[cfg(feature = "keymap")]
pub use keymap::{Action, KeyBinding, Keymap, KeymapPreset};

pub use screen::{Screen, ScreenAction, ScreenId, ScreenManager, Transition};

pub use widgets::{
    Dialog, DialogVariant, Menu, MenuItem, MenuState, Popup, Toast, ToastManager, ToastPosition,
    ToastVariant, ToastWidget,
};

pub use components::{Footer, Header, HeaderWithWidget, HelpOverlay};
