pub mod backup_manager;
pub mod layout;
pub mod path;
pub mod profile_validation;
pub mod style;
pub mod symlink_manager;
pub mod text;
pub mod text_input;

// Export utilities that are used
pub use backup_manager::BackupManager;
pub use layout::{center_popup, create_standard_layout, create_split_layout};
pub use path::{expand_path, get_config_path, get_config_dir, get_home_dir};
pub use profile_validation::{validate_profile_name, sanitize_profile_name, is_safe_profile_name};
pub use style::{focused_border_style, unfocused_border_style, disabled_border_style, disabled_text_style, input_placeholder_style, input_text_style};
pub use symlink_manager::SymlinkManager;
pub use text_input::{handle_backspace, handle_char_insertion, handle_cursor_movement, handle_delete};

