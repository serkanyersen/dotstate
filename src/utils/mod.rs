pub mod backup_manager;
pub mod layout;
pub mod package_installer;
pub mod package_manager;
pub mod path;
pub mod profile_manifest;
pub mod profile_validation;
pub mod style;
pub mod symlink_manager;
pub mod text;
pub mod text_input;

// Export utilities that are used
pub use backup_manager::BackupManager;
pub use layout::{center_popup, create_split_layout, create_standard_layout};
pub use path::{
    expand_path, get_config_dir, get_config_path, get_home_dir, get_repository_path, is_git_repo,
    is_safe_to_add,
};
pub use profile_manifest::{ProfileInfo, ProfileManifest};
pub use profile_validation::{sanitize_profile_name, validate_profile_name};
pub use style::{
    disabled_border_style, disabled_text_style, focused_border_style, input_placeholder_style,
    input_text_style, unfocused_border_style,
};
pub use symlink_manager::SymlinkManager;
pub use text_input::{
    handle_backspace, handle_char_insertion, handle_cursor_movement, handle_delete,
};
