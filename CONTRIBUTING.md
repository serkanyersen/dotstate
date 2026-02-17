# Contributing to DotState

Thank you for your interest in contributing to DotState! This document provides guidelines and instructions for contributing.

## Code of Conduct

By participating in this project, you agree to maintain a respectful and inclusive environment for everyone.

## How to Contribute

### Reporting Bugs

Before reporting a bug, please:
1. Check if the issue already exists in the [GitHub Issues](https://github.com/serkanyersen/dotstate/issues)
2. Try to reproduce the issue with the latest version
3. Check the logs (run `dotstate logs` to see log location)

When reporting a bug, please include:
- **Description**: Clear description of the bug
- **Steps to Reproduce**: Detailed steps to reproduce the issue
- **Expected Behavior**: What you expected to happen
- **Actual Behavior**: What actually happened
- **Environment**: OS, Rust version, DotState version
- **Logs**: Relevant log output (if applicable)

### Suggesting Features

We welcome feature suggestions! Please:
1. Check if the feature has already been suggested
2. Open an issue with the `enhancement` label
3. Describe the feature clearly and explain why it would be useful
4. Consider implementation complexity and maintenance burden

### Pull Requests

1. **Fork the repository** and create a branch from `main`
2. **Make your changes** following our coding standards
3. **Test your changes** thoroughly
4. **Update documentation** if needed
5. **Write clear commit messages**
6. **Open a pull request** with a clear description

#### Pull Request Guidelines

- **One feature per PR**: Keep pull requests focused on a single feature or bug fix
- **Descriptive title**: Use clear, descriptive titles
- **Description**: Explain what the PR does and why
- **Tests**: Include tests for new features or bug fixes
- **Documentation**: Update README or other docs if needed
- **Breaking changes**: Clearly mark any breaking changes

## Development Setup

### Prerequisites

- Rust (latest stable version)
- Git
- A terminal that supports TUI (most modern terminals work)

### Building

```bash
# Clone your fork
git clone https://github.com/serkanyersen/dotstate.git
cd dotstate

# Build in debug mode
cargo build

# Build in release mode
cargo build --release

# Run tests
cargo test

# Run the application
cargo run
```

### Project Structure

```
dotstate/
├── src/
│   ├── main.rs              # Binary entry point (CLI/TUI launcher)
│   ├── lib.rs               # Library exports and module wiring
│   ├── app.rs               # TUI event loop and screen routing
│   ├── cli/                 # CLI command modules
│   ├── screens/             # Screen controllers (Screen trait implementations)
│   ├── components/          # Reusable UI components
│   ├── services/            # Business logic services
│   ├── utils/               # Infrastructure/utilities (symlink, validation, etc.)
│   ├── config.rs            # Configuration management
│   ├── git.rs               # Git operations
│   ├── github.rs            # GitHub API integration
│   ├── tui.rs               # TUI setup and terminal integration
│   ├── ui.rs                # Shared UI state and enums
│   └── keymap/              # Keybinding presets and overrides
├── Cargo.toml
├── README.md
└── CONTRIBUTING.md
```

## Coding Standards

### Rust Style

- Follow [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- Use `rustfmt` for formatting (run `cargo fmt`)
- Use `clippy` for linting (run `cargo clippy`)
- Prefer explicit error handling over panics
- Use meaningful variable and function names

### Code Organization

- Keep functions focused and small
- Use modules to organize related functionality
- Document public APIs with doc comments
- Add comments for complex logic

### Error Handling

- Use `Result<T>` for operations that can fail
- Provide clear, actionable error messages
- Use `anyhow::Context` for error context
- Log errors appropriately (use `tracing` crate)

### Testing

- Write unit tests for utility functions
- Test error cases, not just happy paths
- Use descriptive test names
- Keep tests fast and isolated

## Areas for Contribution

We're always looking for help in these areas:

- **Documentation**: Improving README, adding examples, writing guides
- **Testing**: Adding tests, improving test coverage
- **UI/UX**: Improving the TUI interface, adding features
- **Performance**: Optimizing operations, reducing memory usage
- **Platform Support**: Improving cross-platform compatibility
- **Package Managers**: Adding support for more package managers
- **Bug Fixes**: Fixing reported issues

## Commit Messages

Write clear, descriptive commit messages:

```
Good: "Add support for custom file paths in config"
Good: "Fix symlink creation on Windows"
Bad: "fix"
Bad: "updates"
```

Use imperative mood ("Add" not "Added", "Fix" not "Fixed").

## Review Process

1. All PRs require at least one review
2. Maintainers will review for:
   - Code quality and style
   - Test coverage
   - Documentation updates
   - Breaking changes
3. Address review comments promptly
4. Once approved, maintainers will merge

## Questions?

If you have questions about contributing:
- Open a [GitHub Discussion](https://github.com/serkanyersen/dotstate/discussions)
- Check existing issues and discussions
- Reach out to maintainers

Thank you for contributing to DotState! 🎉
