# dotzz

A friendly TUI (Terminal User Interface) tool for managing dotfiles with GitHub sync, built with Rust.

## Features

- 🎨 Beautiful TUI interface with mouse support
- 🔄 GitHub sync for your dotfiles
- 📁 Profile/set support (work, personal, mac, linux, etc.)
- 🔒 Safe backups before any file operations
- ⚡ Fast and reliable (built with Rust)
- 🎯 Smart dotfile detection

## Installation

*Installation instructions will be added once binaries are available.*

## Development

### Prerequisites

- Rust (latest stable version)
- Cargo

### Building

```bash
cargo build --release
```

### Running

```bash
cargo run
```

### Testing

```bash
cargo test
```

## Project Structure

```
dotzz/
├── src/
│   ├── main.rs          # Entry point
│   ├── lib.rs           # Library exports
│   ├── app.rs           # Main application state
│   ├── config.rs        # Configuration management
│   ├── file_manager.rs  # File operations
│   ├── git.rs           # Git operations
│   ├── tui.rs           # TUI setup
│   └── ui.rs            # UI components
├── Cargo.toml           # Dependencies
├── PROGRESS.md          # Development progress
└── README.md            # This file
```

## License

MIT OR Apache-2.0


