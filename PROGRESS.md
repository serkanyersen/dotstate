# dotzz Development Progress

## Project Status

### Foundation ✅

- [x] Project structure initialized
- [x] Cargo.toml with dependencies configured
- [x] Core module structure created
- [x] Configuration management (TOML)
- [x] File manager module (scanning, backup, symlink)
- [x] Git operations module (git2-rs)
- [x] Basic TUI setup (ratatui)
- [x] Initial unit tests

### Phase 1: Core Functionality ✅

- [x] GitHub client module (PAT-based authentication)
- [x] TUI event loop with keyboard support
- [x] Welcome screen
- [x] Main menu/navigation
- [x] Repository creation/initialization
- [x] First-time user onboarding flow (GitHub auth UI)
- [x] Dotfile scanning and selection UI
- [x] File syncing implementation

#### Phase 2: TUI Features ✅

- [x] Main menu/navigation
- [x] Dotfile list view with preview
- [x] File content preview (basic, syntax highlighting pending)
- [x] Interactive selection interface
- [x] Mouse support (basic structure ready)
- [x] View Synced Files screen
- [x] Custom file addition feature

#### Phase 3: Advanced Features (In Progress)

- [ ] Profile/set management (work, personal, etc.)
- [x] Push/pull operations UI
- [x] Status tracking (synced/unsynced files)
- [x] Custom file addition

#### Phase 4: Nice-to-Haves

- [ ] Syntax highlighting in previews (syntect)
- [ ] Brew dependency management
- [ ] Brew installation helper
- [ ] Dependency tracking

#### Phase 5: Distribution

- [ ] GitHub Actions for releases
- [ ] Homebrew formula
- [ ] Binary distribution setup

## Current Implementation Details

### Modules

#### `config.rs`

- Configuration management with TOML
- Default dotfile list (configurable)
- Profile/set support structure
- GitHub configuration
- Synced files tracking
- **Secure file permissions** (600: owner read/write only) on Unix systems
- Backward compatibility for config migration

#### `file_manager.rs`

- Dotfile scanning
- Backup creation (.bak extension)
- Symlink creation and resolution
- File/directory operations
- Recursive directory copying
- Symlink detection and resolution
- File restoration from backup or repo

#### `git.rs`

- Repository initialization
- Commit operations
- Push/pull operations
- Remote management

#### `tui.rs`

- Terminal setup/teardown
- Raw mode management
- Mouse capture

#### `app.rs`

- Main application state
- Application lifecycle
- Dotfile scanning and selection
- File syncing/unsyncing with state tracking
- Custom file addition
- Push/pull operations
- GitHub repository setup flow

#### `ui.rs`

- Welcome screen rendering
- Main menu rendering
- GitHub authentication screen with input fields
- **Comprehensive help panel** with:
  - Step-by-step token creation instructions
  - Security best practices
  - Token scope requirements
  - Fine-grained token setup guide
  - Token storage information
  - Repository access limitations
- Scrollable help panel (toggle with 'h')
- Message screen rendering
- Dotfile selection screen with:
  - File list with selection indicators
  - Automatic file preview
  - Scrollable preview panel
  - Custom file addition input
  - Status messages with background
- View Synced Files screen
- Screen state management
- Input state management (token, custom files)
- Error and status message display

#### `github.rs`

- GitHub API client (PAT-based)
- User authentication
- Repository existence checking
- Repository creation
- Personal Access Token authentication helper

#### `app.rs` (Updated)

- GitHub authentication flow with input handling
- Repository initialization (create or clone if exists)
- Async operations with tokio runtime
- Config persistence after GitHub setup
- Multi-step authentication UI (token → processing)
- Fixed repository name (`dotzz-storage`)
- Configurable repository clone location

#### `app.rs`

- Main event loop
- Screen navigation
- Keyboard event handling
- Application state management

## Testing

- Unit tests added for:
  - Config save/load
  - File manager creation
  - Git initialization

## Recent Features Completed

### File Syncing

- ✅ Symlink detection and resolution (handles existing symlinks)
- ✅ Copy original files to repo, create new symlinks
- ✅ Unsync functionality (restore original, remove from repo)
- ✅ Folder syncing (recursive, folder-level selection)
- ✅ State persistence (remembers synced files)
- ✅ Sync summary with counts

### Custom Files

- ✅ Add custom files from selection screen (press 'a')
- ✅ Supports relative paths, absolute paths, and tilde expansion
- ✅ File existence validation
- ✅ Persists custom files in config
- ✅ Auto-selects newly added files

### Push/Pull Operations

- ✅ Push changes to GitHub (commit + push)
- ✅ Pull changes from GitHub
- ✅ Automatic branch detection
- ✅ Error handling and status messages

### UI Improvements

- ✅ All screens have title and description
- ✅ Status box with background
- ✅ Preview always shows selected file
- ✅ Scrollable preview (PgUp/PgDn)
- ✅ Visual cursor in input fields
- ✅ Focus management for inputs

## Notes

- Platform focus: macOS (structured for easy extension)
- All paths preserved in repository structure
- Backups created with .bak extension at original location
- Default profile is "main", can be extended with work/personal/etc.
- Repository name is fixed: `dotzz-storage`
- Repository location is configurable (default: `~/.dotzz`)
