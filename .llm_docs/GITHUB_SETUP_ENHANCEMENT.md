# GitHub Setup Enhancement - Implementation Notes

## Changes Made

### 1. Config Updates (`src/config.rs`)
- ✅ Changed default profile name from `"main"` to `"Personal"` to avoid conflict with git branch name
- ✅ Updated test to reflect new default

### 2. UI State Updates (`src/ui.rs`)
- ✅ Added new fields to `GitHubAuthState`:
  - `repo_name_input`: Repository name (default: "dotstate-storage")
  - `repo_location_input`: Local repo path (default: "~/.dotstate")
  - `is_private`: Whether repo should be private (default: true)
  - `focused_field`: Track which input field is focused
- ✅ Added `GitHubAuthField` enum: Token, RepoName, RepoLocation, IsPrivate
- ✅ Renamed `GitHubAuthStep::TokenInput` to `GitHubAuthStep::Input`

### 3. App Updates (`src/app.rs`)
- ✅ Updated all references from `GitHubAuthStep::TokenInput` to `GitHubAuthStep::Input`

## Next Steps - GitHub Auth Component

The following needs to be implemented in `src/components/github_auth.rs`:

### 1. Update Rendering
- Show 4 input fields instead of 1:
  1. **GitHub Token** (existing)
  2. **Repository Name** (text input with default)
  3. **Repository Location** (text input with default path)
  4. **Repository Visibility** (toggle: Private/Public)

### 2. Field Navigation
- Tab/Shift+Tab to navigate between fields
- Arrow keys to navigate between fields
- Each field should have visual focus indicator

### 3. Repository Visibility Toggle
- Space bar or Enter to toggle Private/Public
- Visual indicator: `[✓] Private` or `[ ] Public`

### 4. Input Handling Per Field
- Token: Password-style input (hidden or dots)
- Repo Name: Regular text input
- Repo Location: Path input (with tilde expansion)
- Visibility: Toggle control

### 5. Validation
- Token: Must start with `ghp_`, length >= 40
- Repo Name: Must not be empty, valid GitHub repo name
- Repo Location: Must be valid path
- All fields required before submission

### 6. Help Text Updates
Update help panel to explain all fields:
```
GitHub Personal Access Token
- Create at: https://github.com/settings/tokens
- Required scopes: repo (full control)

Repository Name
- Name for your dotfiles repository on GitHub
- Will be created if it doesn't exist

Local Repository Location
- Where dotfiles will be stored locally
- Default: ~/.dotstate

Repository Visibility
- Private: Only you can see the repository
- Public: Anyone can see the repository
```

### 7. Submission
When Enter is pressed on the last field or a "Submit" button:
- Validate all fields
- Use values from all input fields
- Pass repo visibility to GitHub API

## Layout Suggestion

```
┌─ dotstate - GitHub Setup ────────────────────────────┐
│                                                        │
│  GitHub Token:                                         │
│  ┌──────────────────────────────────────────────────┐ │
│  │ ghp_************************************         │ │
│  └──────────────────────────────────────────────────┘ │
│                                                        │
│  Repository Name:                                      │
│  ┌──────────────────────────────────────────────────┐ │
│  │ dotstate-storage                                  │ │
│  └──────────────────────────────────────────────────┘ │
│                                                        │
│  Local Path:                                           │
│  ┌──────────────────────────────────────────────────┐ │
│  │ ~/.dotstate                                       │ │
│  └──────────────────────────────────────────────────┘ │
│                                                        │
│  Repository Visibility:                                │
│  [✓] Private    [ ] Public                             │
│                                                        │
│  Tab: Next Field | Enter: Submit | ?: Help            │
└────────────────────────────────────────────────────────┘
```

## Implementation Priority

1. **High**: Update rendering to show all fields
2. **High**: Implement field navigation (Tab/arrows)
3. **High**: Implement input handling for each field type
4. **Medium**: Update validation logic
5. **Medium**: Update help text
6. **Low**: Add visual polish (colors, borders, icons)

