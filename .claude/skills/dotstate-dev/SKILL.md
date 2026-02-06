---
name: DotState-Development
description: This skill should be used when working on the DotState project (llm-tui or dotstate repository), "dotstate development", "add a screen to dotstate", "create a dotstate feature", "fix dotstate bug", "dotstate architecture", or any task involving the Rust TUI dotfile manager codebase.
---

# DotState Development Guide

## Screen Development

### Screen Trait

Every screen implements `Screen` from `src/screens/screen_trait.rs`:

```rust
impl Screen for MyScreen {
    fn render(&mut self, frame: &mut Frame, area: Rect, ctx: &RenderContext) -> Result<()>;
    fn handle_event(&mut self, event: Event, ctx: &ScreenContext) -> Result<ScreenAction>;
    fn is_input_focused(&self) -> bool;
    fn on_enter(&mut self, ctx: &ScreenContext) -> Result<()>;
    fn on_exit(&mut self, ctx: &ScreenContext) -> Result<()>;
}
```

**Screen lifecycle:** `on_enter()` -> `render()` loop -> `handle_event()` returns `ScreenAction` -> `tick()` every 250ms -> `on_exit()`.

### Creating a New Screen

1. Create `src/screens/my_screen.rs`
2. Implement `Screen` trait
3. Add to `src/screens/mod.rs`: `mod my_screen; pub use my_screen::MyScreen;`
4. Add variant to `Screen` enum in `src/ui.rs`
5. Add routing in `src/app.rs`: initialize screen, add render match arm, add event handling match arm

### Context Objects

```rust
// Available during render (read-only)
pub struct RenderContext<'a> {
    pub config: &'a Config,
    pub syntax_set: &'a SyntaxSet,   // For syntax highlighting
    pub theme_set: &'a ThemeSet,
    pub syntax_theme: &'a Theme,
}

// Available during event handling (read-only)
pub struct ScreenContext<'a> {
    pub config: &'a Config,
    pub config_path: &'a Path,
    pub repo_path: &'a Path,
    pub active_profile: &'a str,
}
```

### Layout and Components

```rust
use crate::utils::{create_standard_layout, create_split_layout};

// Standard layout returns (header_area, content_area, footer_area)
let (header, content, footer) = create_standard_layout(area, 5, 3);

// Split layout returns Vec<Rect> from percentages
let panes = create_split_layout(content, &[40, 60]);

// Header and Footer are static methods
Header::render(frame, header, "Title", "Description")?;
Footer::render(frame, footer, "key1: Action | key2: Action")?;
```

### ScreenAction (common variants)

Screens return `ScreenAction` from `handle_event()`. Key variants:

| Variant | Use |
|---------|-----|
| `None` | No action needed |
| `Navigate(ScreenId)` | Switch to another screen |
| `ShowMessage { title, content }` | Show a modal message popup |
| `ShowToast { message, variant }` | Non-blocking auto-dismiss notification |
| `Quit` | Exit the application |
| `Refresh` | Trigger a redraw |
| `SetHasChanges(bool)` | Mark unpushed changes exist |
| `ConfigUpdated` | Signal app to reload config |
| `ShowHelp` | Open help overlay |
| `UpdateSetting { setting, option_index }` | Apply a settings change |

Setup-specific: `SaveLocalRepoConfig`, `StartGitHubSetup`, `UpdateGitHubToken`, `ShowProfileSelection`, `CreateAndActivateProfile`, `ActivateProfile`.

File operations: `ScanDotfiles`, `RefreshFileBrowser`, `ToggleFileSync`, `AddCustomFileToSync`.

### Text Input Focus Guard

When a screen has text inputs, add this at the TOP of `handle_event`, BEFORE matching actions:

```rust
if self.is_text_input_focused() {
    if let Some(action) = action {
        if !TextInput::is_action_allowed_when_focused(&action) {
            if let KeyCode::Char(c) = key.code {
                if !key.modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER) {
                    self.text_input.insert_char(c);
                    return Ok(ScreenAction::Refresh);
                }
            }
        }
    }
}
```

Without this, keys like 'q' (bound to `Action::Quit`) exit the app instead of typing.

## Theme and Keymap

### Theme System

```rust
use crate::styles::theme;
let t = theme();

// Color fields: t.primary, t.secondary, t.text, t.text_muted, t.border, t.highlight, t.success, t.warning, t.error
// Style methods: t.text_style(), t.muted_style(), t.emphasis_style(), t.title_style(), t.highlight_style()
// Border: t.border_type(is_focused), t.border_focused_style()

use crate::utils::{focused_border_style, unfocused_border_style};
let style = if is_focused { focused_border_style() } else { unfocused_border_style() };
```

### Keymap System

```rust
use crate::keymap::Action;

if let Event::Key(key) = event {
    if key.kind != KeyEventKind::Press { return Ok(ScreenAction::None); }

    let action = ctx.config.keymap.get_action(key.code, key.modifiers);
    match action {
        Some(Action::Confirm) => { /* Enter */ }
        Some(Action::Cancel) => { /* Esc */ }
        Some(Action::MoveUp) => { /* Up/k */ }
        Some(Action::MoveDown) => { /* Down/j */ }
        Some(Action::Quit) => { /* q */ }
        _ => {}
    }
}

// For footer key display:
let k = |a| ctx.config.keymap.get_key_display_for_action(a);
let nav = ctx.config.keymap.navigation_display();
```

## Symlink Management

**NEVER use `std::os::unix::fs::symlink` directly.**

```rust
use crate::utils::SymlinkManager;

let mut symlink_mgr = SymlinkManager::new_with_backup(repo_path.clone(), backup_enabled)?;
symlink_mgr.create_symlink(&source_path, &target_path)?;
symlink_mgr.remove_symlink(&target_path)?;
symlink_mgr.save_tracking()?;  // REQUIRED after modifications
```

Tracks all symlinks in `~/.config/dotstate/symlinks.json`. Handles backup creation before overwriting.

## Services Layer

Always use services for business operations:

```rust
use crate::services::{ProfileService, SyncService, PackageService};

// Profiles
ProfileService::activate_profile(repo_path, name, backup_enabled)?;
ProfileService::switch_profile(repo_path, from, to, backup_enabled)?;
ProfileService::create_profile(repo_path, name, description, copy_from)?;
ProfileService::delete_profile(repo_path, name)?;
ProfileService::ensure_common_symlinks(repo_path, backup_enabled)?;  // After common file changes

// File sync
SyncService::add_file_to_sync(config, full_path, relative_path, backup_enabled)?;
SyncService::remove_file_from_sync(config, file_path)?;

// Packages
PackageService::get_available_managers();
PackageService::check_package(package)?;
PackageService::install_package(package)?;
```

### Common Files

Common files live in `common/` and are symlinked for ALL profiles:

```rust
let mut manifest = ProfileManifest::load(repo_path)?;
manifest.common.synced_files.push(relative_path.to_string());
manifest.save(repo_path)?;
ProfileService::ensure_common_symlinks(repo_path, backup_enabled)?;  // REQUIRED
```

## Sync Validation

Before adding files/directories, `validate_before_sync()` in `src/utils/sync_validation.rs` checks:

1. **Already synced** - Not a duplicate
2. **Inside synced directory** - Not inside an already-synced parent
3. **Contains synced files** - Directory doesn't contain already-synced children
4. **Git repositories** - No `.git` folders (direct or nested)
5. **Symlink issues** (for directories) - Broken, circular, external, or very large (>100MB)

Circular symlinks cause `copy_dir_all()` to crash (stack overflow). Always validate first.

**Note:** `copy_dir_all()` in FileManager **dereferences symlinks** - symlinks become regular files/dirs. Internal symlinks within dotfile content (e.g., `~/.config/app/current -> versions/v1`) are user content, not DotState-managed.

## File Versioning

Config/data files use schema versioning (`version: u32` field with `#[serde(default)]`):

| File | Struct |
|------|--------|
| `config.toml` | `Config` |
| `.dotstate-profiles.toml` | `ProfileManifest` |
| `symlinks.json` | `SymlinkTracking` |
| `package_status.json` | `PackageCacheData` |

Missing version = v0, auto-migrates on load. Adding a migration:

1. Increment `CURRENT_VERSION` in the module
2. Add `migrate_vN_to_vM()` function setting `data.version = M`
3. Add to the `migrate()` chain: `if data.version == N { data = Self::migrate_vN_to_vM(data)?; }`
4. Use `crate::utils::migrate_file()` for backup/save/cleanup
5. Add tests

## Git Operations

### RepoMode

`Config.repo_mode` determines authentication:

- **`RepoMode::GitHub`** - Repo created via GitHub API. Uses stored token for auth. Token can optionally be embedded in remote URL (see `embed_credentials_in_url` config).
- **`RepoMode::Local`** - User-provided repo. Uses system git credentials (SSH agent, credential helper).

### SSH vs HTTPS

- **SSH URLs** (`git@...`, `ssh://...`): Uses system `git` CLI for fetch/push/clone (bypasses git2's libssh2 for compatibility with 1Password, YubiKey, etc.)
- **HTTPS URLs**: Uses git2 library directly with token auth

### Sync Flow

`GitService::sync_with_remote()`: commit all -> fetch -> pull with rebase -> push

Pull-with-rebase: fetch -> compare HEAD with FETCH_HEAD -> fast-forward or rebase -> update branch ref -> checkout.

## Async Operations

For long-running operations, use thread + channel:

```rust
use std::sync::mpsc;

let (tx, rx) = mpsc::channel();
std::thread::spawn(move || {
    let result = expensive_operation();
    let _ = tx.send(result);
});
self.state.operation_rx = Some(rx);

// Poll in tick() (runs every 250ms)
fn tick(&mut self) -> Result<ScreenAction> {
    if let Some(rx) = &self.state.operation_rx {
        if let Ok(result) = rx.try_recv() {
            self.state.operation_rx = None;
            // Handle result
        }
    }
    Ok(ScreenAction::None)
}
```

## Mouse Support

All screens, popups, and interactive components **must** support mouse interactions. The pattern: store `Rect` areas during `render()`, hit-test them in `handle_event()` on `Event::Mouse`.

### MouseRegions Utility

```rust
use crate::utils::MouseRegions;

// In your screen struct:
mouse_regions: MouseRegions<usize>,  // value type = what a click resolves to

// In render():
self.mouse_regions.clear();
for (i, item) in items.iter().enumerate() {
    let row_area = Rect::new(inner.x, inner.y + i as u16, inner.width, 1);
    self.mouse_regions.add(row_area, i);
}

// In handle_event():
Event::Mouse(mouse) => {
    if let MouseEventKind::Down(MouseButton::Left) = mouse.kind {
        if let Some(&idx) = self.mouse_regions.hit_test(mouse.column, mouse.row) {
            self.state.list_state.select(Some(idx));
            return Ok(ScreenAction::Refresh);
        }
    }
}
```

### Popup Field Click-to-Focus

For popups with multiple input fields, store each field's `Rect` during render and set focus on click:

```rust
// In render (store areas):
self.field_areas.clear();
self.field_areas.push((chunks[1], MyField::Name));
self.field_areas.push((chunks[2], MyField::Description));

// In handle_event (hit-test):
let pos = Position::new(mouse.column, mouse.row);
for &(area, field) in &self.field_areas {
    if area.contains(pos) {
        self.state.focused_field = field;
        return Ok(ScreenAction::Refresh);
    }
}
```

### Critical: Block Background When Popups Are Open

When a popup/overlay is active, **all mouse events on the background must be blocked** — no clicks on background lists, no scroll on background content:

```rust
fn handle_mouse_event(&mut self, mouse: MouseEvent) -> ScreenAction {
    let popup_open = self.state.popup_type != PopupType::None;

    match mouse.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            // Handle popup clicks FIRST
            if popup_open {
                // ... popup field hit-testing ...
                return ScreenAction::None; // consume click, don't fall through
            }
            // Background list clicks only when no popup
            if let Some(&idx) = self.mouse_regions.hit_test(mouse.column, mouse.row) { ... }
        }
        MouseEventKind::ScrollUp if !popup_open => { /* background scroll */ }
        MouseEventKind::ScrollDown if !popup_open => { /* background scroll */ }
        _ => {}
    }
}
```

### Scroll Support

For scrollable areas, handle `ScrollUp`/`ScrollDown` with area hit-testing:

```rust
MouseEventKind::ScrollDown => {
    if let Some(area) = self.list_area {
        let pos = Position::new(mouse.column, mouse.row);
        if area.contains(pos) {
            // Scroll 3 items per tick for comfortable speed
            let current = self.state.list_state.selected().unwrap_or(0);
            let new_idx = (current + 3).min(total.saturating_sub(1));
            self.state.list_state.select(Some(new_idx));
        }
    }
}
```

### Mouse Event Routing in handle_event

Ensure `Event::Mouse` is routed for both main screen and popup states:

```rust
fn handle_event(&mut self, event: Event, ctx: &ScreenContext) -> Result<ScreenAction> {
    if self.state.popup_type != PopupType::None {
        match event {
            Event::Key(key) => { /* popup key handling */ }
            Event::Mouse(mouse) => { /* popup mouse handling */ }
            _ => {}
        }
        return Ok(ScreenAction::None);
    }
    match event {
        Event::Key(key) => { /* main key handling */ }
        Event::Mouse(mouse) => { /* main mouse handling */ }
        _ => {}
    }
}
```

## Error Handling

```rust
use anyhow::{Context, Result};

let data = fs::read_to_string(path).context("Failed to read config")?;

// User-facing errors:
return Ok(ScreenAction::ShowMessage {
    title: "Error".to_string(),
    content: format!("Failed to save: {}", e),
});
```

## Common Pitfalls

1. Using raw symlinks instead of SymlinkManager (bypasses tracking)
2. Hardcoding colors/keys instead of theme/keymap
3. Missing text input focus guard (global keys interfere with typing)
4. Direct manifest edits without calling `ensure_common_symlinks`
5. Syncing directories without validation (circular symlinks crash)
6. Forgetting CHANGELOG updates for user-visible changes
7. Skipping `cargo fmt && cargo clippy` before committing
8. Missing mouse support on new screens/popups (all interactive elements need click + scroll)
9. Not blocking background mouse events when a popup is open (scroll/click bleeds through)
10. Not routing `Event::Mouse` in popup event handlers (only handling `Event::Key`)
