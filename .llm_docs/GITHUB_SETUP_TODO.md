# GitHub Setup Enhancement - Implementation Status

## ✅ Completed

### 1. Config & State
- ✅ Changed default profile from "main" to "Personal"
- ✅ Added new fields to `GitHubAuthState`:
  - `repo_name_input`
  - `repo_location_input`
  - `is_private`
  - `focused_field`
- ✅ Added `GitHubAuthField` enum
- ✅ Renamed `GitHubAuthStep::TokenInput` to `GitHubAuthStep::Input`

### 2. Component Updates
- ✅ Updated `GitHubAuthComponent` rendering:
  - Shows all 4 input fields
  - Each field has proper focus indication
  - Visibility field shows checkboxes
  - Help panel updated with all field descriptions
- ✅ Mouse support for all fields
- ✅ Visibility toggle on click

## 🔨 TODO - App.rs Event Handling

The component is ready, but `app.rs` needs updates to handle:

### 1. Field Navigation (Tab/Shift+Tab)
Need to update `handle_github_auth_keyboard` in `app.rs`:
- Tab: Move to next field
- Shift+Tab: Move to previous field
- Field order: Token → RepoName → RepoLocation → IsPrivate

### 2. Input Handling Per Field
Currently only handles token input. Need to route input to correct field based on `focused_field`:
- `GitHubAuthField::Token` → update `token_input`
- `GitHubAuthField::RepoName` → update `repo_name_input`
- `GitHubAuthField::RepoLocation` → update `repo_location_input`
- `GitHubAuthField::IsPrivate` → toggle on Space/Enter

### 3. Cursor Position Management
Reset cursor position when switching fields

### 4. Validation Updates
Update validation in `process_github_setup` to check all fields:
- Token: starts with "ghp_", length >= 40
- Repo Name: not empty, valid characters
- Repo Location: valid path (expand tilde)
- All fields required

### 5. Use New Values
When creating repo, use:
- `auth_state.repo_name_input` for repo name
- `auth_state.repo_location_input` for local path
- `auth_state.is_private` for repo visibility in GitHub API call

## Implementation Guide for app.rs

### Field Navigation Logic
```rust
KeyCode::Tab => {
    if key.modifiers.contains(KeyModifiers::SHIFT) {
        // Previous field
        auth_state.focused_field = match auth_state.focused_field {
            GitHubAuthField::Token => GitHubAuthField::IsPrivate,
            GitHubAuthField::RepoName => GitHubAuthField::Token,
            GitHubAuthField::RepoLocation => GitHubAuthField::RepoName,
            GitHubAuthField::IsPrivate => GitHubAuthField::RepoLocation,
        };
    } else {
        // Next field
        auth_state.focused_field = match auth_state.focused_field {
            GitHubAuthField::Token => GitHubAuthField::RepoName,
            GitHubAuthField::RepoName => GitHubAuthField::RepoLocation,
            GitHubAuthField::RepoLocation => GitHubAuthField::IsPrivate,
            GitHubAuthField::IsPrivate => GitHubAuthField::Token,
        };
    }
    // Reset cursor position
    auth_state.cursor_position = get_current_field_length(auth_state);
}
```

### Input Routing
```rust
KeyCode::Char(c) => {
    match auth_state.focused_field {
        GitHubAuthField::Token => {
            crate::utils::handle_char_insertion(
                &mut auth_state.token_input,
                &mut auth_state.cursor_position,
                c
            );
        }
        GitHubAuthField::RepoName => {
            crate::utils::handle_char_insertion(
                &mut auth_state.repo_name_input,
                &mut auth_state.cursor_position,
                c
            );
        }
        GitHubAuthField::RepoLocation => {
            crate::utils::handle_char_insertion(
                &mut auth_state.repo_location_input,
                &mut auth_state.cursor_position,
                c
            );
        }
        GitHubAuthField::IsPrivate => {
            // Ignore character input for toggle field
        }
    }
}

KeyCode::Backspace => {
    match auth_state.focused_field {
        GitHubAuthField::Token => {
            crate::utils::handle_backspace(
                &mut auth_state.token_input,
                &mut auth_state.cursor_position
            );
        }
        GitHubAuthField::RepoName => {
            crate::utils::handle_backspace(
                &mut auth_state.repo_name_input,
                &mut auth_state.cursor_position
            );
        }
        GitHubAuthField::RepoLocation => {
            crate::utils::handle_backspace(
                &mut auth_state.repo_location_input,
                &mut auth_state.cursor_position
            );
        }
        GitHubAuthField::IsPrivate => {}
    }
}

KeyCode::Char(' ') if auth_state.focused_field == GitHubAuthField::IsPrivate => {
    auth_state.is_private = !auth_state.is_private;
}
```

### Validation
```rust
// Validate all fields
let token = auth_state.token_input.trim();
let repo_name = auth_state.repo_name_input.trim();
let repo_location = auth_state.repo_location_input.trim();

if token.is_empty() || repo_name.is_empty() || repo_location.is_empty() {
    auth_state.error_message = Some("All fields are required".to_string());
    return Ok(());
}

if !token.starts_with("ghp_") {
    auth_state.error_message = Some("Token must start with 'ghp_'".to_string());
    return Ok(());
}

// Expand tilde in path
let expanded_path = crate::utils::expand_path(repo_location)?;
```

## Files to Modify
- `src/app.rs` - Main event handling and validation logic
- Possibly add helper function for field navigation

## Testing Checklist
- [ ] Tab navigates forward through fields
- [ ] Shift+Tab navigates backward
- [ ] Mouse click focuses correct field
- [ ] Input goes to correct field
- [ ] Backspace/Delete work in each field
- [ ] Space toggles visibility
- [ ] Cursor position updates correctly
- [ ] Validation checks all fields
- [ ] Values are used when creating repo
- [ ] Tilde expansion works for path
- [ ] Private/Public setting passed to GitHub API

