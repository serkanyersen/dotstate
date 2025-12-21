use ratatui::widgets::{ListState, ScrollbarState};
use crate::file_manager::Dotfile;
use crate::components::profile_manager::ProfileManagerState;
use std::path::PathBuf;

/// Application screens
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Welcome,
    MainMenu,
    DotfileSelection,
    GitHubAuth,
    ViewSyncedFiles,
    PushChanges,
    PullChanges,
    ManageProfiles,
    ProfileSelection, // For selecting which profile to activate after setup
}

/// GitHub auth state
#[derive(Debug, Clone)]
pub struct GitHubAuthState {
    pub token_input: String,
    pub repo_name_input: String,
    pub repo_location_input: String,
    pub is_private: bool,
    pub step: GitHubAuthStep,
    pub error_message: Option<String>,
    pub status_message: Option<String>,
    pub help_scroll: usize,
    pub cursor_position: usize, // For current input
    pub input_focused: bool, // Whether input is currently focused
    pub focused_field: GitHubAuthField, // Which field is currently focused
    pub is_editing_token: bool, // Whether we're in "edit token" mode
    pub repo_already_configured: bool, // Whether repo was already set up
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitHubAuthField {
    Token,
    RepoName,
    RepoLocation,
    IsPrivate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitHubAuthStep {
    Input,
    Processing,
}

impl Default for GitHubAuthState {
    fn default() -> Self {
        let default_repo_path = dirs::home_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join(".dotstate");

        Self {
            token_input: String::new(),
            repo_name_input: "dotstate-storage".to_string(),
            repo_location_input: default_repo_path.to_string_lossy().to_string(),
            is_private: true, // Private by default
            step: GitHubAuthStep::Input,
            error_message: None,
            status_message: None,
            help_scroll: 0,
            cursor_position: 0,
            input_focused: true, // Input starts focused
            focused_field: GitHubAuthField::Token,
            is_editing_token: false,
            repo_already_configured: false,
        }
    }
}

/// Focus area in dotfile selection screen
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DotfileSelectionFocus {
    FilesList,           // Files list pane is focused
    Preview,             // Preview pane is focused
    FileBrowserList,     // File browser list pane is focused
    FileBrowserPreview,  // File browser preview pane is focused
    FileBrowserInput,    // File browser path input is focused
    CustomInput,         // Custom file input is focused
}

/// Dotfile selection state
#[derive(Debug)]
pub struct DotfileSelectionState {
    pub dotfiles: Vec<Dotfile>,
    pub selected_index: usize, // Deprecated, using dotfile_list_state now
    pub preview_index: Option<usize>,
    pub scroll_offset: usize, // Deprecated, using dotfile_list_state now
    pub preview_scroll: usize,
    pub selected_for_sync: std::collections::HashSet<usize>, // Indices of selected files
    pub dotfile_list_scrollbar: ScrollbarState, // Scrollbar state for dotfile list
    pub dotfile_list_state: ListState, // ListState for main dotfile list (handles selection and scrolling)
    pub status_message: Option<String>, // For sync summary
    pub adding_custom_file: bool, // Whether we're in "add custom file" mode
    pub custom_file_input: String, // Input for custom file path
    pub custom_file_cursor: usize, // Cursor position for custom file input
    pub custom_file_focused: bool, // Whether custom file input is focused
    pub file_browser_mode: bool, // Whether we're in file browser mode
    pub file_browser_path: PathBuf, // Current directory in file browser
    pub file_browser_selected: usize, // Selected file index in browser
    pub file_browser_entries: Vec<PathBuf>, // Files/dirs in current directory
    #[allow(dead_code)]
    pub file_browser_scroll: usize, // Scroll offset for file browser list (deprecated, using ListState now)
    pub file_browser_scrollbar: ScrollbarState, // Scrollbar state for file browser
    pub file_browser_list_state: ListState, // ListState for file browser (handles selection and scrolling)
    pub file_browser_preview_scroll: usize, // Scroll offset for file browser preview
    pub file_browser_path_input: String, // Path input for file browser
    pub file_browser_path_cursor: usize, // Cursor position for path input
    pub file_browser_path_focused: bool, // Whether path input is focused
    pub focus: DotfileSelectionFocus, // Which pane currently has focus
    pub show_unsaved_warning: bool, // Whether to show unsaved changes warning popup
    pub backup_enabled: bool, // Whether backups are enabled (tracks config value)
}

impl Default for DotfileSelectionState {
    fn default() -> Self {
        Self {
            dotfiles: Vec::new(),
            selected_index: 0,
            preview_index: None,
            scroll_offset: 0,
            preview_scroll: 0,
            selected_for_sync: std::collections::HashSet::new(),
            dotfile_list_scrollbar: ScrollbarState::new(0),
            dotfile_list_state: ListState::default(),
            status_message: None,
            adding_custom_file: false,
            custom_file_input: String::new(),
            custom_file_cursor: 0,
            custom_file_focused: true,
            file_browser_mode: false,
            file_browser_path: dirs::home_dir().unwrap_or_else(|| PathBuf::from("/")),
            file_browser_selected: 0,
            file_browser_entries: Vec::new(),
            file_browser_scroll: 0,
            file_browser_scrollbar: ScrollbarState::new(0),
            file_browser_list_state: ListState::default(),
            file_browser_preview_scroll: 0,
            file_browser_path_input: String::new(),
            file_browser_path_cursor: 0,
            file_browser_path_focused: false,
            focus: DotfileSelectionFocus::FilesList, // Start with files list focused
            show_unsaved_warning: false,
            backup_enabled: true, // Default to enabled
        }
    }
}

/// Push changes state
#[derive(Debug, Clone)]
pub struct PushChangesState {
    pub changed_files: Vec<String>,
    pub is_pushing: bool,
    pub push_progress: Option<String>, // Current progress message (e.g., "Committing...", "Pushing...")
    pub push_result: Option<String>, // Final result message
    pub show_result_popup: bool, // Whether to show result popup
    pub list_state: ListState,
    pub scrollbar_state: ScrollbarState,
}

impl Default for PushChangesState {
    fn default() -> Self {
        Self {
            changed_files: Vec::new(),
            is_pushing: false,
            push_progress: None,
            push_result: None,
            show_result_popup: false,
            list_state: ListState::default(),
            scrollbar_state: ScrollbarState::new(0),
        }
    }
}

/// State for profile selection screen (after GitHub setup)
#[derive(Debug)]
pub struct ProfileSelectionState {
    pub profiles: Vec<String>, // List of profile names
    pub list_state: ListState,
    pub selected_profile: Option<String>, // Selected profile to activate
}

impl Default for ProfileSelectionState {
    fn default() -> Self {
        Self {
            profiles: Vec::new(),
            list_state: ListState::default(),
            selected_profile: None,
        }
    }
}

/// Application UI state
#[derive(Debug)]
pub struct UiState {
    pub current_screen: Screen,
    pub selected_index: usize,
    pub github_auth: GitHubAuthState,
    pub dotfile_selection: DotfileSelectionState,
    pub push_changes: PushChangesState,
    pub profile_manager: ProfileManagerState,
    pub has_changes_to_push: bool, // Whether there are uncommitted or unpushed changes
    /// State for profile selection after GitHub setup
    pub profile_selection: ProfileSelectionState,
}

impl UiState {
    pub fn new() -> Self {
        Self {
            current_screen: Screen::Welcome,
            selected_index: 0,
            github_auth: GitHubAuthState::default(),
            dotfile_selection: DotfileSelectionState::default(),
            push_changes: PushChangesState::default(),
            profile_manager: ProfileManagerState::default(),
            has_changes_to_push: false,
            profile_selection: ProfileSelectionState::default(),
        }
    }
}

// Legacy render functions removed - replaced by components:
// - render_welcome() -> WelcomeComponent
// - render_main_menu() -> MainMenuComponent
// - render_github_auth() -> GitHubAuthComponent
// - render_message() -> MessageComponent
// - render_synced_files() -> SyncedFilesComponent
// - render_dotfile_selection() -> DotfileSelectionComponent::render_with_state()
// popup_area removed - use crate::utils::center_popup instead
