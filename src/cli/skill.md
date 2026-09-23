---
name: dotstate
description: "Manage the user's dotfiles with DotState: add or remove tracked config files, switch profiles, share files across machines as common files, sync the storage repo with its git remote, manage per-profile packages, and diagnose broken symlinks. Use when the user mentions DotState or dotstate, or asks to track, sync, back up, or restore dotfiles on a machine where `dotstate` is installed."
---

# DotState

DotState is a dotfile manager. It moves the user's config files into a git repository (the storage repo) and replaces each original with a symlink that points into the repository. Syncing the repository with a git remote carries the configs to other machines.

The installed binary is the authority for command syntax. Check it before relying on this file:

```bash
dotstate --version
dotstate --help
dotstate <command> --help
```

Do not run bare `dotstate` with no arguments. That launches the interactive TUI, which an agent cannot drive.

## Core concepts

**Storage repo.** A git repository, by default at `~/.config/dotstate/storage`. Print the real location with `dotstate repository`. Every tracked file lives here. The file in the home directory is only a symlink.

**Editing a tracked file.** Edit the file through its normal path, for example `~/.zshrc`. The symlink writes through to the storage repo. There is no separate "update" step. Run `dotstate sync` afterwards to commit and push the change.

**Profiles.** A profile is a named set of tracked files and packages, usually one per machine or context (`work`, `personal`, `server`). Exactly one profile is active on a machine. Profile files live in `<storage>/<profile>/<path relative to home>`.

**Inheritance.** A profile can inherit from one parent profile. When a profile is activated, DotState resolves files in this order, highest priority first: the profile's own files, then the parent's files, then the grandparent's, and finally common files. A child overrides a parent file by tracking a file at the same path. Cycles are rejected.

**Common files.** Common files are shared by every profile and stay linked when the user switches profiles. They live in `<storage>/common/`. Use common for configs that are identical everywhere, such as `.gitconfig`. Use a profile for configs that differ per machine.

**Manifest.** `<storage>/.dotstate-profiles.toml` records the profiles, their parents, their files, the common files, and each profile's packages. DotState owns this file. Change it through commands, not by editing it.

**Symlink tracking.** `~/.config/dotstate/symlinks.json` records every symlink DotState created. `dotstate doctor` compares it against the disk.

**Backups.** Before DotState replaces a real file with a symlink, it copies the original to `~/.dotstate-backups/<timestamp>/`. This is the recovery source if a storage file is lost.

**Activation.** When a profile is active, its symlinks exist in the home directory. `dotstate deactivate` replaces every managed symlink with a real copy of the file, so the user's configs keep working without DotState. `dotstate activate` recreates the symlinks.

**Packages.** Each profile can list packages it needs (for example `ripgrep` from `brew`). DotState checks whether each package's binary exists and can install the missing ones. Supported managers: `brew`, `apt`, `yum`, `dnf`, `pacman`, `snap`, `cargo`, `npm`, `pip`, `pip3`, `gem`, and `custom` (a user-supplied install command).

**Repository modes.** In GitHub mode, DotState manages a `dotstate-storage` repository and authenticates with a token from `DOTSTATE_GITHUB_TOKEN` or the config file. In Local mode, the user brings any git repository, and DotState uses their existing git credentials (SSH keys work).

## First-time setup is interactive

Choosing a repository mode, creating the storage repo, and creating profiles happen only in the TUI. If `dotstate profile current` reports that no profile is set, or `dotstate repository` shows no configured repository, tell the user to run `dotstate` in their terminal to finish setup. Do not try to create the storage repo by hand.

## Commands

### Inspect state (read-only, always safe)

```bash
dotstate profile current        # active profile name
dotstate profile list           # all profiles, active one marked
dotstate list                   # tracked files for the active profile, inherited and common
dotstate list --verbose         # adds symlink and storage paths with their status
dotstate repository             # storage repo location
dotstate config                 # config file location
dotstate logs                   # log file location
dotstate doctor                 # full health check
dotstate doctor --json          # same, machine-readable
dotstate packages list          # packages for the active profile
dotstate packages check         # which of those packages are installed
```

Start with `dotstate doctor` when anything looks wrong. It checks the config, the repository, the manifest, every tracked symlink (including links whose storage file is missing), backups, and filesystem permissions.

### Track and untrack files

```bash
dotstate add ~/.zshrc --yes               # track in the active profile
dotstate add ~/.gitconfig --common --yes  # track as a common file
dotstate remove .zshrc --yes              # stop tracking, restore the real file
dotstate remove .gitconfig --common --yes
```

`add` moves the file (or directory) into the storage repo and leaves a symlink behind. `remove` takes the path relative to the home directory and puts a real file back. Without `--yes`, both commands wait for a `[y/N]` answer on stdin.

Before adding a file, consider:

- Secrets. Files like `~/.ssh/id_*`, `.netrc`, `.aws/credentials`, and anything holding tokens should not go into a git repository. Warn the user instead of adding them.
- Scope. Choose `--common` only if the file should be identical on every machine.
- Directories. Adding a directory tracks the whole tree. Check its size first and avoid caches and nested git repositories.

### Sync with the remote

```bash
dotstate sync                        # commit local changes, pull with rebase, push
dotstate sync -m "Update tmux keys"  # custom commit message
```

Sync commits only when there are changes. If the pull hits a real conflict, sync stops and prints the manual steps. Do not run `git reset --hard` or `git checkout` inside the storage repo to recover. The repo holds the targets of the user's live config symlinks, so those commands can break a running desktop. Report the conflict to the user.

### Profiles

```bash
dotstate profile switch work
```

Switching removes the old profile's symlinks and creates the new profile's symlinks, including inherited ones. Common files stay linked. Run `dotstate doctor` afterwards if the switch reported errors. Creating, renaming, and deleting profiles, and setting a parent, are TUI-only.

### Activate and deactivate

```bash
dotstate activate     # recreate symlinks for the active profile (for example on a freshly cloned machine)
dotstate deactivate   # replace symlinks with real copies of the files
```

Use `deactivate` before uninstalling DotState, or when the user wants plain files back.

### Packages

```bash
dotstate packages add --name ripgrep --manager brew --binary rg \
  --package-name ripgrep --description "Fast grep"
dotstate packages add --name mytool --manager custom --binary mytool \
  --install-command "curl -fsSL https://example.com/install.sh | sh" \
  --existence-check "command -v mytool" --description ""
dotstate packages remove ripgrep --yes
dotstate packages install           # install everything missing for the active profile
dotstate packages install --verbose # show the package manager's output
```

Every packages command takes `--profile <name>` to target a profile other than the active one. `packages add` prompts on stdin for every value you leave out, including optional ones such as `--package-name`, `--description`, and (for `custom`) `--existence-check`. Pass them all, using an empty string where there is nothing to say. `packages install` never prompts for a sudo password. If a manager needs sudo and passwordless sudo is not available, the install is reported as blocked. Tell the user to run it themselves.

### Maintenance

```bash
dotstate doctor --fix     # auto-fix what doctor can repair
dotstate upgrade --check  # report whether a newer version exists
dotstate omarchy --install    # Omarchy only: add DotState to the app launcher as a floating window
dotstate omarchy --uninstall  # remove what --install added
```

## Common tasks

**Track a new config and push it.** `dotstate add <path> --yes`, then `dotstate sync`.

**Change a tracked config.** Edit it at its normal path, then run `dotstate sync`.

**Set up a new machine that already has a storage repo.** The user runs `dotstate` once to connect the repo and pick a profile. After that, `dotstate activate` and `dotstate packages install` bring the machine up to date.

**A config stopped loading.** Run `dotstate doctor`. If it reports a symlink pointing to a missing storage file, look for the original in `~/.dotstate-backups/`, show the user what you found, and restore it only with their approval.

**Move a file between a profile and common.** This is TUI-only today. Alternatively, run `dotstate remove <path> --yes` followed by `dotstate add <path> --common --yes`.

## Rules

- Never edit `symlinks.json` or `.dotstate-profiles.toml` by hand.
- Never replace a managed symlink with a raw `ln -s` or `rm`. Use `add`, `remove`, `activate`, and `deactivate`.
- Never commit or push the storage repo with plain git while a sync might be running. Prefer `dotstate sync`.
- Treat anything in the storage repo as potentially private, and do not paste its contents into public places.
