/// A candidate dotfile that can be scanned and synced
#[derive(Debug, Clone)]
pub struct DotfileCandidate {
    pub path: &'static str,
    pub description: &'static str,
}

impl DotfileCandidate {
    /// Get the path as a String
    #[must_use]
    pub fn path_string(&self) -> String {
        self.path.to_string()
    }
}

/// Default list of dotfiles to scan, with descriptions
pub const DEFAULT_DOTFILES: &[DotfileCandidate] = &[
    // --- Shell & environment ---
    DotfileCandidate {
        path: ".profile",
        description: "Login shell initialization file used by POSIX-compatible shells. Common place for environment variables.",
    },
    DotfileCandidate {
        path: ".bashrc",
        description: "Bash configuration for interactive shells. Aliases, functions, and shell behavior live here.",
    },
    DotfileCandidate {
        path: ".bash_profile",
        description: "Bash login shell configuration, commonly used on macOS.",
    },
    DotfileCandidate {
        path: ".bash_logout",
        description: "Commands executed when a Bash login shell exits.",
    },
    DotfileCandidate {
        path: ".zshrc",
        description: "Zsh configuration for interactive shells. Aliases, prompt setup, plugins.",
    },
    DotfileCandidate {
        path: ".zprofile",
        description: "Zsh login shell configuration. Often used for PATH and environment setup.",
    },
    DotfileCandidate {
        path: ".zshenv",
        description: "Zsh environment configuration executed for all shell invocations.",
    },
    DotfileCandidate {
        path: ".p10k.zsh",
        description: "Powerlevel10k prompt configuration for Zsh.",
    },
    DotfileCandidate {
        path: ".oh-my-zsh",
        description: "Oh My Zsh framework directory containing themes and plugins.",
    },
    DotfileCandidate {
        path: ".inputrc",
        description: "Readline configuration affecting Bash, Python REPL, and other readline-based tools.",
    },
    DotfileCandidate {
        path: ".dircolors",
        description: "Color configuration for `ls` and other GNU coreutils.",
    },

    // --- Editors ---
    DotfileCandidate {
        path: ".vimrc",
        description: "Vim editor configuration file.",
    },
    DotfileCandidate {
        path: ".config/nvim",
        description: "Neovim configuration directory.",
    },
    DotfileCandidate {
        path: ".emacs.d",
        description: "Emacs configuration directory.",
    },
    DotfileCandidate {
        path: ".config/emacs",
        description: "Alternative Emacs configuration directory.",
    },
    DotfileCandidate {
        path: ".config/helix",
        description: "Helix editor configuration directory.",
    },
    DotfileCandidate {
        path: ".config/nano",
        description: "Nano editor configuration directory.",
    },

    // --- Git & version control ---
    DotfileCandidate {
        path: ".gitconfig",
        description: "Global Git configuration: user info, aliases, and defaults.",
    },
    DotfileCandidate {
        path: ".gitconfig.d",
        description: "Directory for modular Git configuration includes.",
    },
    DotfileCandidate {
        path: ".gitattributes",
        description: "Git attributes controlling diffing, merging, and file behavior.",
    },
    DotfileCandidate {
        path: ".gitignore_global",
        description: "Global Git ignore rules applied to all repositories.",
    },
    DotfileCandidate {
        path: ".gitmessage",
        description: "Git commit message template.",
    },

    // --- Terminal & multiplexers ---
    DotfileCandidate {
        path: ".tmux.conf",
        description: "tmux terminal multiplexer configuration.",
    },
    DotfileCandidate {
        path: ".config/zellij",
        description: "Zellij terminal multiplexer configuration.",
    },
    DotfileCandidate {
        path: ".config/screen",
        description: "GNU screen configuration directory.",
    },
    DotfileCandidate {
        path: ".config/less",
        description: "Configuration for the `less` pager.",
    },

    // --- Terminal emulators ---
    DotfileCandidate {
        path: ".config/alacritty",
        description: "Alacritty terminal emulator configuration.",
    },
    DotfileCandidate {
        path: ".config/kitty",
        description: "Kitty terminal emulator configuration.",
    },
    DotfileCandidate {
        path: ".config/wezterm",
        description: "WezTerm terminal emulator configuration.",
    },
    DotfileCandidate {
        path: ".config/foot",
        description: "Foot terminal emulator configuration.",
    },

    // --- CLI UX tools ---
    DotfileCandidate {
        path: ".config/starship.toml",
        description: "Starship cross-shell prompt configuration.",
    },
    DotfileCandidate {
        path: ".config/bat",
        description: "Configuration for `bat`, a syntax-highlighted `cat` replacement.",
    },
    DotfileCandidate {
        path: ".config/ripgrep",
        description: "Default flags and settings for ripgrep (`rg`).",
    },
    DotfileCandidate {
        path: ".config/fd",
        description: "Default behavior for `fd`, a modern `find` replacement.",
    },
    DotfileCandidate {
        path: ".config/eza",
        description: "Configuration for `eza`, a modern `ls` replacement.",
    },
    DotfileCandidate {
        path: ".config/direnv",
        description: "direnv configuration directory.",
    },
    DotfileCandidate {
        path: ".envrc",
        description: "Per-directory environment variables managed by direnv.",
    },

    // --- SSH & crypto (config only) ---
    DotfileCandidate {
        path: ".ssh/config",
        description: "SSH client configuration: host aliases, keys, and options.",
    },
    DotfileCandidate {
        path: ".sshconfig",
        description: "SSH client configuration: host aliases, keys, and options.",
    },
    DotfileCandidate {
        path: ".ssh/known_hosts",
        description: "Known SSH host keys. Does not contain private keys.",
    },
    DotfileCandidate {
        path: ".gnupg/gpg.conf",
        description: "GnuPG configuration file.",
    },
    DotfileCandidate {
        path: ".gnupg/gpg-agent.conf",
        description: "GnuPG agent configuration.",
    },

    // --- Language & package managers ---
    DotfileCandidate {
        path: ".npmrc",
        description: "npm configuration file.",
    },
    DotfileCandidate {
        path: ".yarnrc",
        description: "Yarn (classic) configuration file.",
    },
    DotfileCandidate {
        path: ".yarnrc.yml",
        description: "Yarn Berry (modern) configuration file.",
    },
    DotfileCandidate {
        path: ".pnpmrc",
        description: "pnpm configuration file.",
    },
    DotfileCandidate {
        path: ".cargo/config.toml",
        description: "Cargo (Rust) configuration: registries, aliases, build flags.",
    },
    DotfileCandidate {
        path: ".rustfmt.toml",
        description: "Rust code formatting configuration.",
    },
    DotfileCandidate {
        path: ".tool-versions",
        description: "asdf-managed language version definitions.",
    },
    DotfileCandidate {
        path: ".config/asdf",
        description: "asdf version manager configuration directory.",
    },
    DotfileCandidate {
        path: ".pyenvrc",
        description: "pyenv shell integration configuration.",
    },

    // --- OS / desktop ---
    DotfileCandidate {
        path: ".config/fontconfig",
        description: "Font rendering and selection configuration.",
    },
    DotfileCandidate {
        path: ".config/mimeapps.list",
        description: "Default application associations for MIME types.",
    },
    DotfileCandidate {
        path: ".config/systemd/user",
        description: "User-level systemd services and timers.",
    },

    // --- AI tools ---
    // Antigravity
    DotfileCandidate {
        path: ".agent/skills",
        description: "Antigravity global skills directory (usable across projects).",
    },
    // claude
    DotfileCandidate {
        path: ".claude/skills",
        description: "Claude global skills directory (usable across projects).",
    },
    DotfileCandidate {
        path: ".claude/commands",
        description: "Claude global slash-commands directory.",
    },
    // copilot
    DotfileCandidate {
        path: ".github/skills",
        description: "GitHub Copilot / VS Code personal skills directory (global).",
    },
    // Gemini
    DotfileCandidate {
        path: ".gemini/skills",
        description: "Gemini global skills directory (usable across projects).",
    },
    // cursor
    DotfileCandidate {
        path: ".cursor/skills",
        description: "Cursor global skills directory (Cursor stores global skills here).",
    },
    DotfileCandidate {
        path: ".cursor/commands",
        description: "Cursor global commands directory (Cursor stores global commands here).",
    },
    // aider
    DotfileCandidate {
        path: ".aider.conf.yml",
        description: "Aider global config (aider loads this from home).",
    },
    DotfileCandidate {
        path: ".aider.conventions.md",
        description: "Aider global conventions file (commonly used for shared instructions).",
    },
    // opencode
    DotfileCandidate {
        path: ".config/opencode/skills/",
        description: "OpenCode global skills directory.",
    },
    DotfileCandidate {
        path: ".opencode/skills/",
        description: "OpenCode global skills directory.",
    },
    DotfileCandidate {
        path: ".agents/skills/",
        description: "Agent global skills directory.",
    }
];

/// Get default dotfile paths as a Vec<String>
#[must_use]
pub fn get_default_dotfile_paths() -> Vec<String> {
    DEFAULT_DOTFILES
        .iter()
        .map(DotfileCandidate::path_string)
        .collect()
}

/// Find a dotfile candidate by path
#[must_use]
pub fn find_candidate(path: &str) -> Option<&DotfileCandidate> {
    DEFAULT_DOTFILES.iter().find(|c| c.path == path)
}
