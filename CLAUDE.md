# CLAUDE.md

DotState is a terminal-based dotfile manager built with Rust (ratatui + git2). TUI and CLI for managing dotfiles through Git repos with profiles, symlink tracking, backups, and package management.

**For full development guidance, load the `dotstate-dev` skill.**

## Commands

```bash
cargo build                  # Build
cargo run                    # Run TUI
cargo run -- list            # Run CLI command
cargo test                   # Run all tests
cargo fmt && cargo clippy    # Format + lint (REQUIRED before committing)
```

## Architecture

```
src/
├── app.rs           # Main event loop, screen routing, ScreenAction dispatch
├── main.rs          # Entry point, CLI parsing, terminal setup
├── cli.rs           # CLI command definitions
├── ui.rs            # Screen enum, shared state structs
├── styles.rs        # Theme definitions and color system
├── git.rs           # Low-level git operations (git2 + system git for SSH)
├── screens/         # Screen controllers (implement Screen trait)
├── components/      # Reusable UI (header, footer, help_overlay)
├── services/        # Business logic (git, sync, profile, package)
├── utils/           # Infrastructure (symlink_manager, backup_manager, etc.)
├── widgets/         # Custom ratatui widgets
└── keymap/          # Keyboard config with presets (Standard, Vim, Emacs)
```

**Data flow:** `App::run()` polls crossterm events -> screen's `handle_event()` -> returns `ScreenAction` -> App processes action -> screen's `render()` draws UI.

## Critical Rules

1. **Use `SymlinkManager` for DotState-managed symlinks** - Do not call raw symlink APIs for home↔repo managed links (tracking + backups live in `SymlinkManager`). Exception: preserving internal content symlinks during recursive copy (`copy_dir_all`) is allowed.
2. **Never hardcode colors** - Always use `theme()` from `crate::styles`
3. **Never hardcode keys** - Always use the keymap system via `ctx.config.keymap.get_action()`
4. **Always use Services layer** for business operations (never bypass with direct file/symlink ops)
5. **Guard text input focus** - At the top of `handle_event`, check `is_text_input_focused()` before matching actions, or 'q' will quit instead of typing
6. **After modifying common files** in manifest, always call `ProfileService::ensure_common_symlinks()`
7. **Validate before syncing directories** - `validate_before_sync()` must run before `copy_dir_all()` (circular symlinks cause crashes)
8. **Mouse support is required** - All new screens, popups, and interactive components must support mouse click-to-focus/select and scroll. Store `Rect` areas during `render()`, hit-test in `handle_event()`. Block background interactions when popups are open.

## Post-Task Checklist

1. `cargo fmt`
2. `cargo clippy` - fix all warnings
3. `cargo test` (if applicable)
4. Update `CHANGELOG.md` under `[Unreleased]` with format: `- **Component**: Brief description`

## Git Workflow

**Never commit or push without explicit user permission.** No exceptions. The user will handle commits. Just finish the work.
if you are instructed to commit **do not add Co-Author line**

## Key Locations

| Purpose             | Path                                     |
| ------------------- | ---------------------------------------- |
| Config              | `~/.config/dotstate/config.toml`         |
| Storage             | `~/.config/dotstate/storage/` (default)  |
| Symlink tracking    | `~/.config/dotstate/symlinks.json`       |
| Package Check cache | `~/.config/dotstate/package_status.json` |
| Backups             | `~/.dotstate-backups/`                   |
| Profile manifest    | `<repo>/.dotstate-profiles.toml`         |
| Design decisions    | `docs/MEMORY.md`                         |
