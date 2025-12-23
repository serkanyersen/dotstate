# Profile Management Feature Plan

## Overview
Profiles allow users to manage different sets of dotfiles for different computers (e.g., Personal Mac, Work Linux, Home Server).

---

## Terminology
**Profile** = A named set of dotfiles for a specific machine/environment
- Examples: "Personal-Mac", "Work-Linux", "Home-Server"

---

## Storage Structure

### Config File (`~/.config/dotstate/config.toml`)
```toml
active_profile = "Personal-Mac"
repo_path = "/Users/serkan/.dotstate"
repo_name = "dotstate-storage"
default_branch = "main"

[github]
owner = "username"
repo = "dotstate-storage"
token = "ghp_..."

[[profiles]]
name = "Personal-Mac"
description = "Personal MacBook Pro"
synced_files = [
    ".zshrc",
    ".vimrc",
    ".gitconfig",
    ".tmux.conf",
]

[[profiles]]
name = "Work-Linux"
description = "Work Ubuntu Desktop"
synced_files = [
    ".zshrc",
    ".bashrc",
    ".gitconfig",
]
```

**Key Changes:**
- `synced_files` moves from root to each `[[profiles]]` section
- Each profile has its own list of synced files
- Single config file, multiple profile sections

### Repository Structure
```
~/.dotstate/                    # repo_path
├── .git/
├── Personal-Mac/               # Profile folder
│   ├── .zshrc
│   ├── .vimrc
│   ├── .gitconfig
│   └── .tmux.conf
├── Work-Linux/                 # Profile folder
│   ├── .zshrc
│   ├── .bashrc
│   └── .gitconfig
└── Home-Server/                # Profile folder
    └── .bashrc
```

**Important:**
- Profiles are folders at repo root (e.g., `~/.dotstate/Personal-Mac/`)
- NOT nested under a "profiles" subfolder
- Each profile folder contains copies of dotfiles
- All committed to the same git repo
- Profile folder names must be filesystem-safe (alphanumeric, hyphens, underscores)

---

## Core Functionality

### 1. View Profiles
- List all profiles from config
- Show active profile (highlighted with indicator)
- Show file count for each profile
- Show last updated timestamp (from git)
- Display profile descriptions

### 2. Switch Profile
**Workflow:**
1. Select target profile
2. Show warning popup with:
   - Current profile and file count
   - Target profile and file count
   - List of files that will change
   - What will happen (remove old symlinks, create new ones)
3. Require confirmation
4. Execute switch:
   - Deactivate old profile symlinks
   - Create backups (.bak) of existing files
   - Activate new profile symlinks
   - Update config.toml (active_profile)
5. Show success/failure

### 3. Create Profile
**Workflow:**
1. Press `N` → Show popup
2. User enters profile name
3. Validate:
   - Alphanumeric, hyphens, underscores only
   - Max 50 characters
   - Unique (not already exists)
   - Not reserved ("backup", "temp", "main")
4. Ask: "Start with?"
   - Option A: Copy from existing profile (select which one)
   - Option B: Start blank (will go to Scan Dotfiles)
5. Create:
   - Create folder: `~/.dotstate/<profile-name>/`
   - Add to config.toml
   - If copying: Copy files from source profile
   - If blank: Navigate to Scan Dotfiles screen
6. Ask: "Switch to new profile now?"

### 4. Delete Profile
**Safety measures:**
- Cannot delete active profile (must switch first)
- Cannot delete last remaining profile
- Require typing profile name to confirm

**Workflow:**
1. Select profile, press `D`
2. Validate (not active, not last)
3. Show confirmation popup:
   - Profile name
   - Number of files
   - List of files that will be deleted
   - Warning: Cannot be undone
   - Input field: "Type profile name to confirm"
4. User types profile name exactly
5. Delete:
   - Remove profile folder from repo
   - Remove from config.toml
   - Commit deletion to git
6. Show success

### 5. Rename Profile
**Workflow:**
1. Select profile, press `R`
2. Show input popup with current name pre-filled
3. User enters new name
4. Validate (same rules as create)
5. Rename:
   - Rename folder in repo
   - Update config.toml
   - If active profile: Update symlink metadata/tracking
   - Commit change to git
6. Show success

### 6. Copy Profile
**Workflow:**
1. Select profile, press `C`
2. Enter new profile name
3. Validate name
4. Copy:
   - Create new folder
   - Copy all files from source
   - Add to config.toml
5. Ask: "Switch to new profile now?"

### 7. View Profile Details
- Show all files in selected profile
- Show file sizes
- Show last modified dates
- Option to view diff with active profile

---

## UI Layout - Fancy Split

```
┌──────────────────────────────────────────────────────────────────┐
│  dotstate - Manage Profiles                                      │
├────────────────────────┬─────────────────────────────────────────┤
│                        │  📦 Personal-MacBook-Pro                │
│  💻 Profiles           │  ════════════════════════════════════   │
│                        │                                         │
│  ╔════════════════╗    │  ✓ Active Profile                       │
│  ║ Personal-Mac   ║ ◄  │  📂 12 files synced                     │
│  ╚════════════════╝    │  📅 Last updated: 2 days ago            │
│                        │                                         │
│  ┌────────────────┐    │  Files:                                 │
│  │ Work-Linux     │    │  • ~/.zshrc                             │
│  └────────────────┘    │  • ~/.vimrc                             │
│                        │  • ~/.gitconfig                         │
│  ┌────────────────┐    │  • ~/.config/nvim/init.lua             │
│  │ Personal-Linux │    │  • ... 8 more                           │
│  └────────────────┘    │                                         │
│                        │  Actions:                               │
│  [N]ew  [D]elete      │  [S] Switch to this profile             │
│  [R]ename  [C]opy     │  [V] View all files                     │
│                        │                                         │
├────────────────────────┴─────────────────────────────────────────┤
│  ↑↓: Navigate  Enter: Switch  Esc: Back                         │
└──────────────────────────────────────────────────────────────────┘
```

**Visual Design:**
- Active profile: Thick double-line border (╔═╗) + arrow + checkmark
- Other profiles: Single-line border (┌─┐)
- Color coding:
  - Active: LightGreen border, White text
  - Selected (not active): Cyan border, White text
  - Others: DarkGray border, Gray text
- Icons: 💻 for section header, 📦 for profile details, ✓ for active
- Mouse support: Click to select, double-click to switch
- Scrollable lists on both sides

---

## Symlink Manager Utility

### Create: `src/utils/symlink_manager.rs`

```rust
pub struct SymlinkOperation {
    pub source: PathBuf,      // File in profile folder
    pub target: PathBuf,      // Symlink location (home dir)
    pub backup: Option<PathBuf>, // Backup of original file
    pub status: OperationStatus,
}

pub struct SwitchReport {
    pub removed: Vec<SymlinkOperation>,  // Old profile symlinks removed
    pub created: Vec<SymlinkOperation>,  // New profile symlinks created
    pub errors: Vec<(PathBuf, String)>,  // Any errors
    pub rollback_performed: bool,
}

pub struct SwitchPreview {
    pub will_remove: Vec<PathBuf>,
    pub will_create: Vec<PathBuf>,
    pub conflicts: Vec<PathBuf>,  // Files that exist and aren't our symlinks
}

pub struct SymlinkManager {
    repo_path: PathBuf,
    tracking_file: PathBuf,  // Track which symlinks we created
}

impl SymlinkManager {
    /// Activate a profile (create all its symlinks)
    pub fn activate_profile(&self, profile: &Profile) -> Result<Vec<SymlinkOperation>>;

    /// Deactivate a profile (remove its symlinks)
    pub fn deactivate_profile(&self, profile: &Profile) -> Result<Vec<SymlinkOperation>>;

    /// Switch from one profile to another
    pub fn switch_profile(&self, from: &Profile, to: &Profile) -> Result<SwitchReport>;

    /// Preview what would happen (dry run, no changes)
    pub fn preview_switch(&self, from: &Profile, to: &Profile) -> Result<SwitchPreview>;

    /// Check if a path is a symlink we created
    fn is_our_symlink(&self, path: &Path) -> Result<bool>;

    /// Create a backup of a file
    fn backup_file(&self, path: &Path) -> Result<PathBuf>;

    /// Rollback a failed switch operation
    fn rollback(&self, operations: &[SymlinkOperation]) -> Result<()>;
}
```

**Safety Measures:**
1. **Never delete user files** - Only remove symlinks we created
2. **Always create backups** - Original files backed up to `.bak`
3. **Track our symlinks** - Maintain a file listing symlinks we manage
4. **Atomic operations** - All or nothing; rollback on failure
5. **Extensive logging** - Log every operation for debugging
6. **Validate before execute** - Check what would happen first
7. **Preserve permissions** - Maintain file permissions on symlinks

**Tracking File:** `~/.config/dotstate/symlinks.json`
```json
{
  "version": 1,
  "active_profile": "Personal-Mac",
  "symlinks": [
    {
      "target": "/Users/serkan/.zshrc",
      "source": "/Users/serkan/.dotstate/Personal-Mac/.zshrc",
      "created_at": "2024-01-15T10:30:00Z",
      "backup": "/Users/serkan/.zshrc.bak"
    }
  ]
}
```

---

## Profile Validation Rules

### Profile Name Constraints:
- **Characters:** Alphanumeric, hyphens (-), underscores (_) only
- **Length:** 1-50 characters
- **Case:** Preserve user's case (but check uniqueness case-insensitively)
- **Reserved names:** Cannot use: "backup", "temp", "main", ".git", "node_modules"
- **Must be unique:** No duplicate names (case-insensitive check)

### Validation Function:
```rust
pub fn validate_profile_name(name: &str, existing_profiles: &[String]) -> Result<(), String> {
    // Check length
    if name.is_empty() || name.len() > 50 {
        return Err("Profile name must be 1-50 characters".to_string());
    }

    // Check characters
    if !name.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
        return Err("Profile name can only contain letters, numbers, hyphens, and underscores".to_string());
    }

    // Check reserved names
    let reserved = ["backup", "temp", "main", ".git", "node_modules"];
    if reserved.contains(&name.to_lowercase().as_str()) {
        return Err(format!("'{}' is a reserved name", name));
    }

    // Check uniqueness (case-insensitive)
    if existing_profiles.iter().any(|p| p.eq_ignore_ascii_case(name)) {
        return Err("A profile with this name already exists".to_string());
    }

    Ok(())
}
```

---

## Implementation Phases

### Phase 1: Foundation (Week 1)
**Goal:** Create core utilities and data structures

- [ ] Create `SymlinkManager` utility
- [ ] Write comprehensive tests for `SymlinkManager`
- [ ] Add profile validation utilities
- [ ] Update `Config` structure to support profile-specific synced_files
- [ ] Create `Profile` struct with helper methods
- [ ] Add git operations for profile management

**Files to create/modify:**
- `src/utils/symlink_manager.rs` (new)
- `src/utils/profile.rs` (new)
- `src/config.rs` (modify - update structure)
- `tests/symlink_manager_tests.rs` (new)

### Phase 2: UI Component (Week 2)
**Goal:** Build the profile management screen

- [ ] Create `ManageProfilesComponent`
- [ ] Implement split layout with fancy styling
- [ ] Add mouse navigation
- [ ] Add keyboard navigation
- [ ] Create popup components (confirm, input, warning)
- [ ] Add profile list rendering with icons
- [ ] Add profile details rendering

**Files to create:**
- `src/components/manage_profiles.rs` (new)
- `src/components/profile_popup.rs` (new)

### Phase 3: Core Operations (Week 3)
**Goal:** Implement view and switch functionality

- [ ] View all profiles (read-only)
- [ ] Select profile and view details
- [ ] Switch profile with confirmation
- [ ] Show switch preview
- [ ] Implement progress indicators
- [ ] Handle switch errors gracefully
- [ ] Add success/failure notifications

### Phase 4: Profile CRUD (Week 4)
**Goal:** Full create, rename, delete operations

- [ ] Create new profile (blank)
- [ ] Create new profile (copy from existing)
- [ ] Delete profile with confirmation
- [ ] Rename profile
- [ ] Update existing sync flow to use `SymlinkManager`
- [ ] Ensure all operations update git repo

### Phase 5: Polish & Testing (Week 5)
**Goal:** Make it bulletproof

- [ ] Comprehensive error handling
- [ ] Better error messages
- [ ] Edge case handling (no profiles, one profile, etc.)
- [ ] Integration tests
- [ ] Manual testing scenarios
- [ ] Performance optimization
- [ ] Documentation

---

## Safety Checklist

Before ANY destructive operation:
- ✅ Validate profile name/state
- ✅ Check if active profile (for delete/switch)
- ✅ Show preview of what will change
- ✅ Require explicit confirmation
- ✅ Create backups of all files
- ✅ Log all operations to file
- ✅ Test rollback mechanism
- ✅ Handle partial failures gracefully

---

## Testing Strategy

### Unit Tests
- Profile name validation (valid, invalid, edge cases)
- SymlinkManager operations (create, remove, switch)
- Config parsing with multiple profiles
- Backup creation and restoration

### Integration Tests
- Create profile → Add files → Switch → Verify symlinks
- Switch profile → Check old removed → Check new created
- Delete profile → Verify folder gone → Verify config updated
- Rename profile → Verify folder renamed → Verify symlinks still work

### Manual Testing Scenarios
1. **Happy Path:** Create → Add files → Switch → Works
2. **Edge Case:** Delete last profile (should fail)
3. **Edge Case:** Delete active profile (should fail)
4. **Error Case:** Switch fails halfway (should rollback)
5. **Edge Case:** Profile with no files
6. **Error Case:** Name collision
7. **Error Case:** Invalid characters in name

---

## Future Enhancements (Not in MVP)

- Export profile as tarball
- Import profile from tarball
- Diff between two profiles
- Merge files from another profile
- Profile templates/presets
- Cloud sync profiles (beyond git)
- Profile-specific git branches
- Shared files between profiles (symlinks to common folder)

---

## Questions Resolved

1. **Config structure?** → Single config.toml with multiple `[[profiles]]` sections
2. **Where do profile files live?** → `~/.dotstate/ProfileName/` (repo root, not nested)
3. **How to track symlinks?** → Separate JSON tracking file
4. **Can delete active profile?** → No, must switch first
5. **Can rename active profile?** → Yes, with extra care for symlinks
6. **What happens on switch?** → Deactivate old, activate new, create backups

---

## Notes

- This is a critical feature - losing user's dotfiles is unacceptable
- Take time to test thoroughly
- The `SymlinkManager` utility is the foundation - get it right first
- All file operations should be reversible
- Always show the user what will happen before doing it
- Log everything for debugging

