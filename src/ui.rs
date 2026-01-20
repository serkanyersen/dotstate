use ratatui::widgets::{ListState, ScrollbarState};

/// Application screens
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    MainMenu,
    DotfileSelection,
    GitHubAuth,
    SyncWithRemote,
    ManageProfiles,
    ProfileSelection, // For selecting which profile to activate after setup
    ManagePackages,
    Settings,
}

/// GitHub auth state (also handles local repo setup)
#[derive(Debug, Clone)]
pub struct GitHubAuthState {
    // Setup mode selection
    pub setup_mode: SetupMode, // Current setup mode (Choosing, GitHub, Local)
    pub mode_selection_index: usize, // 0 = Create for me (GitHub), 1 = Use own repo (Local)

    // GitHub mode fields
    pub token_input: crate::utils::TextInput,
    pub repo_name_input: crate::utils::TextInput,
    pub repo_location_input: crate::utils::TextInput,
    pub is_private: bool,
    pub step: GitHubAuthStep,
    pub error_message: Option<String>,
    pub status_message: Option<String>,
    pub help_scroll: usize,
    pub input_focused: bool,            // Whether input is currently focused
    pub focused_field: GitHubAuthField, // Which field is currently focused
    pub is_editing_token: bool,         // Whether we're in "edit token" mode
    pub repo_already_configured: bool,  // Whether repo was already set up
    /// Intermediate data stored during GitHub setup process
    pub setup_data: Option<GitHubSetupData>,

    // Local mode fields
    pub local_repo_path_input: crate::utils::TextInput, // Path to user's local repository
    #[allow(dead_code)]
    pub local_step: LocalSetupStep, // Current step in local setup flow (reserved for future async flow)
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

/// Setup mode for repository configuration
/// Determines which setup flow the user is in
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SetupMode {
    /// Initial screen - user chooses between GitHub and Local modes
    #[default]
    Choosing,
    /// GitHub mode - dotstate creates/manages the repository via GitHub API
    GitHub,
    /// Local mode - user provides their own pre-configured repository
    Local,
}

/// State machine for local setup process
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LocalSetupStep {
    #[default]
    Input,
    #[allow(dead_code)]
    Validating,
    #[allow(dead_code)]
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
            // Setup mode selection
            setup_mode: SetupMode::default(),
            mode_selection_index: 0, // Default to "Create for me (GitHub)"

            // GitHub mode fields
            token_input: crate::utils::TextInput::new(),
            repo_name_input: crate::utils::TextInput::with_text(crate::config::default_repo_name()),
            repo_location_input: crate::utils::TextInput::with_text(
                default_repo_path.to_string_lossy().to_string(),
            ),
            is_private: true, // Private by default
            step: GitHubAuthStep::Input,
            error_message: None,
            status_message: None,
            help_scroll: 0,
            input_focused: true, // Input starts focused
            focused_field: GitHubAuthField::Token,
            is_editing_token: false,
            repo_already_configured: false,
            setup_data: None,

            // Local mode fields
            local_repo_path_input: crate::utils::TextInput::with_text(
                default_repo_path.to_string_lossy().to_string(),
            ),
            local_step: LocalSetupStep::Input,
        }
    }
}

/// Sync with remote state
#[derive(Debug, Clone)]
pub struct SyncWithRemoteState {
    pub changed_files: Vec<String>,
    pub is_syncing: bool,
    pub sync_progress: Option<String>, // Current progress message (e.g., "Committing...", "Pulling...", "Pushing...")
    pub sync_result: Option<String>,   // Final result message
    pub show_result_popup: bool,       // Whether to show result popup
    pub pulled_changes_count: Option<usize>, // Number of changes pulled from remote
    pub list_state: ListState,
    pub scrollbar_state: ScrollbarState,
    pub diff_content: Option<String>, // Content of the diff for preview
    pub preview_scroll: usize,        // Scroll state for preview
    pub git_status: Option<crate::services::git_service::GitStatus>, // Detailed git status
}

impl Default for SyncWithRemoteState {
    fn default() -> Self {
        Self {
            changed_files: Vec::new(),
            is_syncing: false,
            sync_progress: None,
            sync_result: None,
            show_result_popup: false,
            pulled_changes_count: None,
            list_state: ListState::default(),
            scrollbar_state: ScrollbarState::new(0),
            diff_content: None,
            preview_scroll: 0,
            git_status: None,
        }
    }
}

/// State for profile selection screen (after GitHub setup)
#[derive(Debug, Default)]
pub struct ProfileSelectionState {
    pub profiles: Vec<String>, // List of profile names
    pub list_state: ListState,
    #[allow(dead_code)] // Reserved for future use
    pub selected_profile: Option<String>, // Selected profile to activate
    pub show_exit_warning: bool, // Show warning when user tries to exit without selecting
    pub show_create_popup: bool, // Show create new profile popup
    pub create_name_input: crate::utils::TextInput, // Input for new profile name
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
    pub add_name_input: crate::utils::TextInput,
    pub add_description_input: crate::utils::TextInput,
    pub add_manager: Option<crate::utils::profile_manifest::PackageManager>,
    pub add_manager_selected: usize, // Index in available managers list
    pub add_package_name_input: crate::utils::TextInput,
    pub add_binary_name_input: crate::utils::TextInput,
    pub add_install_command_input: crate::utils::TextInput, // For custom only
    pub add_existence_check_input: crate::utils::TextInput, // For custom only
    pub add_manager_check_input: crate::utils::TextInput,   // Optional fallback
    pub add_is_custom: bool,                                // Whether in custom mode
    pub add_focused_field: AddPackageField,
    pub add_editing_index: Option<usize>, // None for add, Some(index) for edit
    pub add_validation_error: Option<String>, // Validation error to display in popup
    pub newly_added_index: Option<usize>, // Track newly added package to prompt install after check
    pub available_managers: Vec<crate::utils::profile_manifest::PackageManager>, // OS-filtered list
    pub manager_list_state: ListState,    // For manager selection
    // Delete popup state
    pub delete_confirm_input: crate::utils::TextInput,
    pub delete_index: Option<usize>,
    pub cache: crate::utils::package_cache::PackageCache,
    pub active_profile: String,
}

/// Package popup types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackagePopupType {
    None,
    Add,
    Edit,
    Delete,
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
    PackageName, // For managed packages
    BinaryName,
    InstallCommand, // Custom only
    ExistenceCheck, // Custom only
    ManagerCheck,   // Optional fallback
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
            add_name_input: crate::utils::TextInput::new(),
            add_description_input: crate::utils::TextInput::new(),
            add_manager: None,
            add_manager_selected: 0,
            add_package_name_input: crate::utils::TextInput::new(),
            add_binary_name_input: crate::utils::TextInput::new(),
            add_install_command_input: crate::utils::TextInput::new(),
            add_existence_check_input: crate::utils::TextInput::new(),
            add_manager_check_input: crate::utils::TextInput::new(),
            add_is_custom: false,
            add_focused_field: AddPackageField::Name,
            add_editing_index: None,
            add_validation_error: None,
            newly_added_index: None,
            available_managers: Vec::new(),
            manager_list_state: ListState::default(),
            delete_confirm_input: crate::utils::TextInput::new(),
            delete_index: None,
            checking_delay_until: None,
            installation_step: InstallationStep::NotStarted,
            installation_output: Vec::new(),
            installation_delay_until: None,
            cache: crate::utils::package_cache::PackageCache::default(),
            active_profile: String::new(),
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
        installed: Vec<usize>,           // Successfully installed package indices
        failed: Vec<(usize, String)>,    // Failed package indices with error messages
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
    Complete {
        success: bool,
        error: Option<String>,
    }, // Installation complete
}

/// Application UI state
#[derive(Debug)]
pub struct UiState {
    pub current_screen: Screen,
    pub selected_index: usize,
    pub github_auth: GitHubAuthState,
    pub has_changes_to_push: bool, // Whether there are uncommitted or unpushed changes
    pub git_status: Option<crate::services::git_service::GitStatus>, // Detailed git status
    /// State for profile selection after GitHub setup
    pub profile_selection: ProfileSelectionState,
    /// Whether a text input is currently focused (blocks navigation keybindings)
    /// When true, keymap navigation is disabled so users can type freely
    pub input_mode_active: bool,
    /// Whether the help overlay is currently showing
    pub show_help_overlay: bool,
}

impl Default for UiState {
    fn default() -> Self {
        Self::new()
    }
}

impl UiState {
    pub fn new() -> Self {
        Self {
            current_screen: Screen::MainMenu,
            selected_index: 0,
            github_auth: GitHubAuthState::default(),
            has_changes_to_push: false,
            git_status: None,
            profile_selection: ProfileSelectionState::default(),
            input_mode_active: false,
            show_help_overlay: false,
        }
    }
}

// Legacy render functions removed - replaced by components:
// - render_welcome() -> WelcomeComponent
// - render_main_menu() -> MainMenuComponent
// - render_github_auth() -> GitHubAuthComponent
// - render_message() -> MessageComponent
// - render_synced_files() -> SyncedFilesComponent
// - render_dotfile_selection() -> DotfileSelectionScreen (self-contained)
// popup_area removed - use crate::utils::center_popup instead
