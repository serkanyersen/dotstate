# Open Source Readiness Checklist

Use this checklist to ensure everything is ready for open sourcing.

## Documentation

- [x] **README.md** - User-friendly, highlights features and benefits
- [x] **LICENSE** - MIT License file
- [x] **CONTRIBUTING.md** - Contribution guidelines
- [x] **CHANGELOG.md** - Version history
- [x] **CODE_OF_CONDUCT.md** - Community standards
- [x] **SECURITY.md** - Security policy and reporting
- [x] **INSTALL.md** - Installation instructions
- [x] **PUBLISHING.md** - Release and publishing guide

## GitHub Setup

- [x] **Issue Templates** - Bug report and feature request templates
- [x] **Pull Request Template** - PR guidelines
- [x] **CI/CD Workflow** - Automated testing and building
- [x] **Dependabot** - Automated dependency updates
- [x] **Funding** - Optional funding configuration

## Package Managers

- [x] **Cargo.toml** - Updated with proper metadata
- [x] **Homebrew Formula** - Formula for macOS installation
- [ ] **crates.io** - Ready to publish (run `cargo publish --dry-run` first)

## Code Quality

- [ ] **All tests passing** - Run `cargo test`
- [ ] **No clippy warnings** - Run `cargo clippy -- -D warnings`
- [ ] **Code formatted** - Run `cargo fmt`
- [ ] **Documentation complete** - Public APIs documented

## Pre-Release Tasks

- [ ] **Update version** in `Cargo.toml`
- [ ] **Update CHANGELOG.md** with release notes
- [ ] **Test installation** from source
- [ ] **Test CLI commands** work correctly
- [ ] **Test TUI** works on different terminals
- [ ] **Create release tag** on GitHub
- [ ] **Update repository URL** in all files (replace `yourusername`)

## Post-Release Tasks

- [ ] **Publish to crates.io**
- [ ] **Submit Homebrew formula** (or create tap)
- [ ] **Create GitHub release** with binaries
- [ ] **Announce release** (if desired)

## Additional Recommendations

### Optional but Recommended

- [ ] **Logo/Branding** - Add a logo to README
- [ ] **Screenshots** - Add TUI screenshots to README
- [ ] **Demo GIF/Video** - Show the tool in action
- [ ] **Badges** - Add status badges (build, version, license)
- [ ] **Examples** - Add usage examples
- [ ] **Troubleshooting** - Common issues and solutions
- [ ] **Roadmap** - Future plans (optional)

### Community

- [ ] **Discussions enabled** - For Q&A and general discussion
- [ ] **Wiki enabled** - For extended documentation (optional)
- [ ] **Discord/Slack** - Community chat (optional)

## Notes

- Replace `yourusername` with your actual GitHub username in:
  - `Cargo.toml`
  - `README.md`
  - `Formula/dotstate.rb`
  - All `.github` templates
  - `CONTRIBUTING.md`
  - `SECURITY.md`

- Update `PLACEHOLDER_SHA256` in `Formula/dotstate.rb` after first release

- Consider adding a `.github/CODEOWNERS` file for automatic review assignments

