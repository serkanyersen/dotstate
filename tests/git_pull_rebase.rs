//! Regression tests for `GitManager::pull_with_rebase` and `commit_all`.
//!
//! Each test uses a local bare repository as the remote.

use anyhow::Result;
use dotstate::git::GitManager;
use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .expect("git should run");
    assert!(
        out.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// Creates a bare remote and a clone of it with one commit on `main`, pushed.
fn setup(base: &Path) -> Result<(std::path::PathBuf, std::path::PathBuf)> {
    let remote = base.join("remote.git");
    fs::create_dir_all(&remote)?;
    git(&remote, &["init", "--bare", "-b", "main"]);

    let local = base.join("local");
    git(base, &["clone", remote.to_str().unwrap(), "local"]);
    git(&local, &["config", "user.name", "Test"]);
    git(&local, &["config", "user.email", "test@example.com"]);
    git(&local, &["symbolic-ref", "HEAD", "refs/heads/main"]);
    fs::write(local.join("a.txt"), "one\n")?;
    git(&local, &["add", "."]);
    git(&local, &["commit", "-m", "first"]);
    git(&local, &["push", "-u", "origin", "main"]);
    Ok((remote, local))
}

fn commit_count(repo: &Path) -> usize {
    git(repo, &["rev-list", "--count", "HEAD"]).parse().unwrap()
}

#[test]
fn pull_is_noop_when_local_is_ahead_of_remote() -> Result<()> {
    let tmp = TempDir::new()?;
    let (_remote, local) = setup(tmp.path())?;

    // Local gets a commit the remote does not have, touching a different file set
    // so a checkout of the remote tree would conflict with the local tree.
    fs::write(local.join("a.txt"), "two\n")?;
    fs::write(local.join("b.txt"), "new\n")?;
    let mgr = GitManager::open_or_init(&local)?;
    assert!(mgr.commit_all("local change")?);
    let head_before = git(&local, &["rev-parse", "HEAD"]);

    let pulled = mgr.pull_with_rebase("origin", "main", None)?;

    assert_eq!(pulled, 0);
    // No rebase may start: a spurious rebase checks out the older remote tree and
    // fails with "conflicts prevent checkout" on real storage repos.
    let reflog = git(&local, &["reflog", "--format=%gs"]);
    assert!(!reflog.contains("rebase"), "unexpected rebase: {reflog}");
    assert_eq!(git(&local, &["rev-parse", "HEAD"]), head_before);
    assert_eq!(git(&local, &["symbolic-ref", "HEAD"]), "refs/heads/main");
    Ok(())
}

#[test]
fn commit_all_skips_empty_commit_on_clean_repo() -> Result<()> {
    let tmp = TempDir::new()?;
    let (_remote, local) = setup(tmp.path())?;
    let mgr = GitManager::open_or_init(&local)?;
    // open_or_init may write a .gitignore; commit that first so the tree is clean.
    mgr.commit_all("setup")?;
    let before = commit_count(&local);

    assert!(!mgr.commit_all("Update dotfiles")?);
    assert!(!mgr.commit_all("Update dotfiles")?);

    assert_eq!(commit_count(&local), before);
    Ok(())
}

#[test]
fn failed_rebase_leaves_head_attached_to_branch() -> Result<()> {
    let tmp = TempDir::new()?;
    let (remote, local) = setup(tmp.path())?;

    // Another machine pushes a conflicting change to a.txt.
    let other = tmp.path().join("other");
    git(tmp.path(), &["clone", remote.to_str().unwrap(), "other"]);
    git(&other, &["config", "user.name", "Other"]);
    git(&other, &["config", "user.email", "other@example.com"]);
    fs::write(other.join("a.txt"), "remote edit\n")?;
    git(&other, &["commit", "-am", "remote edit"]);
    git(&other, &["push", "origin", "main"]);

    // Local edits the same line.
    fs::write(local.join("a.txt"), "local edit\n")?;
    let mgr = GitManager::open_or_init(&local)?;
    assert!(mgr.commit_all("local edit")?);
    let local_head = git(&local, &["rev-parse", "HEAD"]);

    assert!(mgr.pull_with_rebase("origin", "main", None).is_err());

    assert_eq!(git(&local, &["symbolic-ref", "HEAD"]), "refs/heads/main");
    assert_eq!(git(&local, &["rev-parse", "main"]), local_head);
    assert_eq!(fs::read_to_string(local.join("a.txt"))?, "local edit\n");
    Ok(())
}
