# prompt

We are building a command line tool that helps users manage their dotfiles. a nice, friendly interface that clearly lists all availale options and gives users the choice to take various actions (which we will list later)

Some requirements

- Use Rust for this, follow best practices for rust terminal apps.
- Keep an ongoing documentation for the features and flags we will add. keep it up to date.
- We need to have a TUI implemented for this tool to differentiate it from similar cli apps.
- we should be able to produce a universal library that can be installed easily on computers without requiring rust.

# Idea

This is a helper tool for users, when they first set-up a new computer they can install this app and pull their dotfiles easily. similar to dotsync or other tools. our difference will be a much friendlier UI, speed and reliability with Rust. plus other features I'll list.

# How it works.

when you install this app, you first need to login with github. the app will create it's own repo and push to remote. if there is already a repo existing, it will pull that instead. app only needs access to this repo, nothing else.

- for first time users, app will collect all known/common dotfiles and move them to the repo, then symlink them to their original spots.
- when on a new computer, it will pull the existing repo, and symlink the files where they are supposed to be.
- app should create a journal or a record of where the files were so it remembers where to put them.
- app should not blindly override files, always create a backup of a file before replacing it.
- We should let the user push their changes to the repo using the app

# features

- nice explainer for first time users, guides them through the github login process and repo creation.
- once repo is created, UI lists all found dotfiles, user can see their contents and chose if they want that file to be synced.
- for repeat users, UI will show already synced files, plus any additional unsynced file (to be added if needed)
- allow users to add any file they want, even if it's not a dotfile. they might have a custom dotfile, a script anything.
- nice way to navigate the app, mouse support would be nice.

# nice to haves

- syntax highlighting in previews would be nice.
- as a separate flow, maybe allow users define set of dependencies to be installed with brew on any new computer
- track if brew dependencies are installed on this computer
- help user to install brew for the first time.

# development method.

- I want to you start with a solid foundation before implementing the features.
- keep a file to track your progress, what's implemented, what's next etc. that way we can continue on multiple sessions if needed.
- we clearly define a feature before implementing, decide on all details first then start coding.
- unittest are vital, keep testing in mind when developing the app and add tests where needed.

# Technical Decisions

## Project Details

- **Project Name**: `dotstate`
- **TUI Library**: `ratatui` (formerly tui-rs)
- **Git Library**: `git2-rs`
- **Configuration Format**: TOML
- **Platform Focus**: macOS (code structured for easy platform extension)

## GitHub Authentication

- Primary: OAuth flow for first-time setup
- Fallback: Personal Access Token (PAT) support

## Repository Structure

- Preserve original file paths in repository
- All dotfiles stored under `main/` folder with full paths (e.g., `main/.config/nvim/init.vim`)
- Support for profiles/sets: work, personal, mac, linux, server, etc.
- Profiles can be renamed or new ones added as needed

## File Management

- **Backups**: Created at original location with `.bak` extension before replacement
- **Journal/Record**: TOML format if needed (may not be necessary since we preserve original paths)

## Default Dotfiles

- Strong default set including:
  - Shell configs: `.bashrc`, `.zshrc`, `.bash_profile`, `.zprofile`
  - Terminal customizations: powerlevel10k, oh-my-zsh configs
  - Editor configs: `.vimrc`, `.vim/`, `.config/nvim/`
  - Git: `.gitconfig`, `.gitignore_global`
  - Terminal: `.tmux.conf`, `.config/alacritty/`, etc.
  - Other common: `.ssh/config`, `.config/fish/`, etc.
- Default set should be configurable by user

## Distribution

- GitHub Releases with pre-built binaries
- Homebrew formula for easy installation
