# Publishing Guide

This document outlines the steps to publish DotState to various package managers and platforms.

## Pre-Release Checklist

- [ ] Update version in `Cargo.toml`
- [ ] Update `CHANGELOG.md` with release notes
- [ ] Update `README.md` if needed
- [ ] Update Nix lockfiles: `./nix/rebuild.sh`
- [ ] Run full test suite: `cargo test`
- [ ] Run clippy: `cargo clippy -- -D warnings`
- [ ] Check formatting: `cargo fmt -- --check`
- [ ] Build release: `cargo build --release`
- [ ] Test release binary locally
- [ ] Create git tag: `git tag -a v0.1.0 -m "Release v0.1.0"`
- [ ] Push tag: `git push origin v0.1.0`

## Publishing to crates.io

1. **Create account** on [crates.io](https://crates.io)
2. **Get API token** from account settings
3. **Login**:
   ```bash
   cargo login YOUR_API_TOKEN
   ```
4. **Publish**:
   ```bash
   cargo publish
   ```

Note: Publishing to crates.io is permanent. Make sure everything is correct!

## Publishing to Homebrew

### Initial Setup

1. **Fork** [homebrew-core](https://github.com/Homebrew/homebrew-core) (or create your own tap)
2. **Create formula** in `Formula/dotstate.rb`
3. **Calculate SHA256**:
   ```bash
   # After creating a release on GitHub
   wget https://github.com/serkanyersen/dotstate/archive/v0.1.0.tar.gz
   shasum -a 256 v0.1.0.tar.gz
   ```
4. **Update formula** with the SHA256
5. **Test locally**:
   ```bash
   brew install --build-from-source Formula/dotstate.rb
   ```
6. **Submit PR** to homebrew-core (or merge to your tap)

### Creating Your Own Tap

1. **Create repository** `homebrew-dotstate` on GitHub
2. **Add formula** to the repository
3. **Users install** with:
   ```bash
   brew tap serkanyersen/dotstate
   brew install dotstate
   ```

   Or use the direct install:
   ```bash
   brew install serkanyersen/dotstate/dotstate
   ```

## Publishing to Nix

The project provides a Nix flake at the repository root. No separate publishing step is needed — users consume it directly from GitHub:

```bash
# Run directly
nix run github:serkanyersen/dotstate

# Build locally
nix build github:serkanyersen/dotstate

# Install via nix profile
nix profile install github:serkanyersen/dotstate
```

### Installing Nix on Any Linux Distro

Nix is **not tied to NixOS** — it works as a package manager on any Linux distribution (Ubuntu, Arch, Fedora, etc.):

1. Install Nix: `sh <(curl -L https://nixos.org/nix/install)`
2. Enable flakes in `~/.config/nix/nix.conf`:
   ```
   experimental-features = nix-command flakes
   ```
3. Run the flake: `nix run github:serkanyersen/dotstate`

### Flake Input

```nix
inputs = {
  dotstate.url = "github:serkanyersen/dotstate";
};
```

### Using the Overlay

```nix
nixpkgs.overlays = [ inputs.dotstate.overlays.default ];
environment.systemPackages = [ pkgs.dotstate ];
```

### Using the Home Manager Module

```nix
imports = [ inputs.dotstate.homeModules.default ];
programs.dotstate.enable = true;
programs.dotstate.settings = {
  repo_mode = "Local";
  repo_path = "~/dotfiles";
};
```

### Non-Flake Usage

```bash
nix-build default.nix
```

### Updating Lockfiles (for maintainers)

After source changes, run this from the repo root to regenerate `Cargo.lock` and `flake.lock`:

```bash
./nix/rebuild.sh
```

Then commit:

```bash
git add Cargo.lock flake.lock
git commit -m "chore: update lockfiles"
```

> **Non-NixOS maintainers can still run this** — just install Nix on your distro first (see "Installing Nix on Any Linux Distro" above). The rebuild script only needs `cargo generate-lockfile`, `nix flake update`, and `nix build` — all available once Nix is installed.

## GitHub Releases

1. **Create release** on GitHub:
   - Go to Releases → Draft a new release
   - Tag: `v0.1.0`
   - Title: `v0.1.0`
   - Description: Copy from `CHANGELOG.md`
2. **Upload binaries** (optional, for direct downloads):
   - Build for multiple platforms
   - Upload `.tar.gz` or `.zip` files
3. **Publish release**

## Building Binaries for Distribution

### macOS

```bash
# Build for Apple Silicon
cargo build --release --target aarch64-apple-darwin

# Build for Intel
cargo build --release --target x86_64-apple-darwin
```

### Linux

```bash
# Build for x86_64
cargo build --release --target x86_64-unknown-linux-gnu

# Or use Docker for consistent builds
docker run --rm -v $(pwd):/app -w /app rust:latest cargo build --release
```

### Cross-compilation

Install cross-compilation targets:
```bash
rustup target add aarch64-apple-darwin
rustup target add x86_64-unknown-linux-gnu
```

## Version Bumping

When releasing a new version:

1. **Update `Cargo.toml`**:
   ```toml
   version = "0.1.1"
   ```

2. **Update `CHANGELOG.md`**:
   - Move items from `[Unreleased]` to new version section
   - Add release date

3. **Commit and tag**:
   ```bash
   git add Cargo.toml CHANGELOG.md
   git commit -m "Bump version to 0.1.1"
   git tag -a v0.1.1 -m "Release v0.1.1"
   git push origin main --tags
   ```

## Post-Release

- [ ] Announce on social media (if desired)
- [ ] Update documentation if needed
- [ ] Monitor issues and feedback
- [ ] Plan next release

