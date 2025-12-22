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
    ManagePackages,
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
    /// Intermediate data stored during GitHub setup process
    pub setup_data: Option<GitHubSetupData>,
}

/// Intermediate data stored during GitHub setup process
#[derive(Debug, Clone)]
pub struct GitHubSetupData {
    pub token: String,
    pub repo_name: String,
    pub username: Option<String>,
    pub repo_exists: Option<bool>,
    pub is_private: bool, // Repository visibility (true = private, false = public)
    pub delay_until: Option<std::time::Instant>, // For delays between steps
    pub is_new_repo: bool, // Whether we're creating a new repo (vs cloning existing)
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
    /// State machine for processing setup steps
    SetupStep(GitHubSetupStep),
}

/// State machine for GitHub setup process
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitHubSetupStep {
    Connecting,
    ValidatingToken,
    CheckingRepo,
    CloningRepo,
    CreatingRepo,
    InitializingRepo,
    DiscoveringProfiles,
    Complete,
}

impl Default for GitHubAuthState {
    fn default() -> Self {
        let default_repo_path = dirs::home_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join(".config")
            .join("dotstate")
            .join("storage");

        Self {
            token_input: String::new(),
            repo_name_input: crate::config::default_repo_name(),
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
            setup_data: None,
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
    #[allow(dead_code)] // Reserved for future use
    pub selected_profile: Option<String>, // Selected profile to activate
    pub show_exit_warning: bool, // Show warning when user tries to exit without selecting
}

impl Default for ProfileSelectionState {
    fn default() -> Self {
        Self {
            profiles: Vec::new(),
            list_state: ListState::default(),
            selected_profile: None,
            show_exit_warning: false,
        }
    }
}

/// Package manager state
#[derive(Debug)]
pub struct PackageManagerState {
    pub list_state: ListState,
    pub packages: Vec<crate::utils::profile_manifest::Package>, // From active profile
    pub popup_type: PackagePopupType,
    // Checking state
    pub is_checking: bool,
    pub checking_index: Option<usize>,
    pub package_statuses: Vec<PackageStatus>, // Installed/NotInstalled/Error
    pub checking_delay_until: Option<std::time::Instant>, // Delay between checks for UI responsiveness
    // Installation state
    pub installation_step: InstallationStep,
    pub installation_output: Vec<String>, // Live output from installation
    pub installation_delay_until: Option<std::time::Instant>, // Delay between installation steps
    // Add/Edit popup state
    pub add_name_input: String,
    pub add_name_cursor: usize,
    pub add_description_input: String,
    pub add_description_cursor: usize,
    pub add_manager: Option<crate::utils::profile_manifest::PackageManager>,
    pub add_manager_selected: usize, // Index in available managers list
    pub add_package_name_input: String,
    pub add_package_name_cursor: usize,
    pub add_binary_name_input: String,
    pub add_binary_name_cursor: usize,
    pub add_install_command_input: String, // For custom only
    pub add_install_command_cursor: usize,
    pub add_existence_check_input: String,  // For custom only
    pub add_existence_check_cursor: usize,
    pub add_manager_check_input: String,   // Optional fallback
    pub add_manager_check_cursor: usize,
    pub add_is_custom: bool, // Whether in custom mode
    pub add_focused_field: AddPackageField,
    pub add_editing_index: Option<usize>, // None for add, Some(index) for edit
    pub available_managers: Vec<crate::utils::profile_manifest::PackageManager>, // OS-filtered list
    pub manager_list_state: ListState, // For manager selection
    // Delete popup state
    pub delete_confirm_input: String,
    pub delete_confirm_cursor: usize,
    pub delete_index: Option<usize>,
}

/// Package popup types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackagePopupType {
    None,
    Add,
    Edit,
    Delete,
    #[allow(dead_code)] // Reserved for future use
    InstallMissing, // Prompt to install missing packages
}

/// Package status
#[derive(Debug, Clone)]
pub enum PackageStatus {
    Unknown,
    Installed,
    NotInstalled,
    Error(String), // Error message if check failed
}

/// Which field is focused in the add/edit package popup
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddPackageField {
    Name,
    Description,
    Manager,
    PackageName,      // For managed packages
    BinaryName,
    InstallCommand,   // Custom only
    ExistenceCheck,   // Custom only
    ManagerCheck,     // Optional fallback
}

impl Default for PackageManagerState {
    fn default() -> Self {
        Self {
            list_state: ListState::default(),
            packages: Vec::new(),
            popup_type: PackagePopupType::None,
            is_checking: false,
            checking_index: None,
            package_statuses: Vec::new(),
            add_name_input: String::new(),
            add_name_cursor: 0,
            add_description_input: String::new(),
            add_description_cursor: 0,
            add_manager: None,
            add_manager_selected: 0,
            add_package_name_input: String::new(),
            add_package_name_cursor: 0,
            add_binary_name_input: String::new(),
            add_binary_name_cursor: 0,
            add_install_command_input: String::new(),
            add_install_command_cursor: 0,
            add_existence_check_input: String::new(),
            add_existence_check_cursor: 0,
            add_manager_check_input: String::new(),
            add_manager_check_cursor: 0,
            add_is_custom: false,
            add_focused_field: AddPackageField::Name,
            add_editing_index: None,
            available_managers: Vec::new(),
            manager_list_state: ListState::default(),
            delete_confirm_input: String::new(),
            delete_confirm_cursor: 0,
            delete_index: None,
            checking_delay_until: None,
            installation_step: InstallationStep::NotStarted,
            installation_output: Vec::new(),
            installation_delay_until: None,
        }
    }
}

/// Installation state machine
#[derive(Debug)]
pub enum InstallationStep {
    NotStarted,
    Installing {
        package_index: usize,
        package_name: String,
        total_packages: usize,
        packages_to_install: Vec<usize>, // Indices of packages that need installation
        installed: Vec<usize>, // Successfully installed package indices
        failed: Vec<(usize, String)>, // Failed package indices with error messages
        status_rx: Option<std::sync::mpsc::Receiver<InstallationStatus>>, // Channel receiver for status updates
    },
    Complete {
        installed: Vec<usize>,
        failed: Vec<(usize, String)>, // (index, error message)
    },
}

/// Installation status message from background thread
#[derive(Debug, Clone)]
pub enum InstallationStatus {
    Output(String), // Output line
    Complete { success: bool, error: Option<String> }, // Installation complete
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
    /// State for package manager
    pub package_manager: PackageManagerState,
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
            package_manager: PackageManagerState::default(),
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
