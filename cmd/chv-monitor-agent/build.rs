// Build script: embed version metadata (VERSION file, git sha, build date,
// release channel) into the binary via cargo:rustc-env.
//
// WHY the git re-run triggers below exist (stale-sha defect):
//
// This script embeds `git rev-parse --short HEAD` into CHV_GIT_SHA, but it
// used to declare only `rerun-if-changed=../../VERSION` and
// `rerun-if-env-changed=CHV_RELEASE_CHANNEL`. Cargo decides whether to
// re-run a build script from its declared triggers (mtime/fingerprint
// based), never from the fact that the script shells out to `git`. So when
// a rebuild happened after a commit that touched only other crates —
// exactly the target-dir reuse that CI gets from Swatinem/rust-cache —
// the script did not re-run and the binaries printed a stale commit sha.
//
// The fix declares re-run triggers on the git state itself:
//   - <gitdir>/HEAD                — changes on checkout/rebase/detached-HEAD commits
//   - <commondir>/refs/heads/<b>   — changes when a commit lands on branch <b>
//                                    (HEAD itself is a static `ref:` pointer)
//   - <commondir>/packed-refs      — where the ref may live after gc/fetch repacks
//
// Release packaging can instead pin the sha authoritatively by exporting
// CHV_GIT_SHA; when set and non-empty it overrides `git rev-parse`.

use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let crate_root = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());

    let version = std::fs::read_to_string(format!("{}/../../VERSION", crate_root))
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "unknown".to_string());

    let git_sha = match std::env::var("CHV_GIT_SHA") {
        Ok(sha) if !sha.trim().is_empty() => sha.trim().to_string(),
        _ => Command::new("git")
            .args(["rev-parse", "--short", "HEAD"])
            .current_dir(&crate_root)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|| "unknown".to_string()),
    };

    let build_date = Command::new("date")
        .args(["+%Y-%m-%d"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let channel = std::env::var("CHV_RELEASE_CHANNEL").unwrap_or_else(|_| "stable".to_string());

    println!("cargo:rerun-if-changed=../../VERSION");
    println!("cargo:rerun-if-env-changed=CHV_RELEASE_CHANNEL");
    println!("cargo:rerun-if-env-changed=CHV_GIT_SHA");

    emit_git_rerun_triggers(&crate_root);

    println!("cargo:rustc-env=CHV_VERSION={}", version);
    println!("cargo:rustc-env=CHV_GIT_SHA={}", git_sha);
    println!("cargo:rustc-env=CHV_BUILD_DATE={}", build_date);
    println!("cargo:rustc-env=CHV_RELEASE_CHANNEL={}", channel);
}

/// Resolve the git directory governing this checkout.
///
/// `<crate>/../../.git` is:
///   - a FILE in a linked worktree, containing a `gitdir: <path>` line
///     (the path may be relative, resolved against the .git file's dir);
///   - a DIRECTORY in a plain clone.
///
/// Returns None when neither resolves (e.g., building from a source
/// tarball); the caller then emits no git triggers and the sha falls back
/// to CHV_GIT_SHA or "unknown".
fn resolve_git_dir(crate_root: &str) -> Option<PathBuf> {
    let dot_git = Path::new(crate_root).join("../../.git");
    if dot_git.is_file() {
        let content = std::fs::read_to_string(&dot_git).ok()?;
        let gitdir_line = content.lines().find(|l| l.starts_with("gitdir:"))?;
        let gitdir = gitdir_line["gitdir:".len()..].trim();
        let gitdir = Path::new(gitdir);
        let gitdir = if gitdir.is_absolute() {
            gitdir.to_path_buf()
        } else {
            dot_git.parent()?.join(gitdir)
        };
        if gitdir.is_dir() {
            Some(gitdir)
        } else {
            None
        }
    } else if dot_git.is_dir() {
        Some(dot_git)
    } else {
        None
    }
}

/// Linked worktrees share most refs with the main repository: everything
/// except per-worktree state (HEAD, refs/bisect, ...) lives in the "common
/// dir" named by `<gitdir>/commondir`. Resolve it so the refs/heads and
/// packed-refs triggers point at the files git actually mutates.
fn resolve_common_dir(git_dir: &Path) -> PathBuf {
    match std::fs::read_to_string(git_dir.join("commondir")) {
        Ok(content) => {
            let common = Path::new(content.trim());
            if common.is_absolute() {
                common.to_path_buf()
            } else {
                git_dir.join(common)
            }
        }
        Err(_) => git_dir.to_path_buf(),
    }
}

/// Emit `cargo:rerun-if-changed` triggers for the git state so this script
/// re-runs (and the embedded sha refreshes) whenever HEAD moves.
///
/// Triggers are only emitted for paths that exist: a nonexistent
/// rerun-if-changed path can never fire and cargo may warn about it.
fn emit_git_rerun_triggers(crate_root: &str) {
    let Some(git_dir) = resolve_git_dir(crate_root) else {
        return;
    };
    let common_dir = resolve_common_dir(&git_dir);

    let head_path = git_dir.join("HEAD");
    if !head_path.exists() {
        return;
    }
    println!("cargo:rerun-if-changed={}", head_path.display());

    let head = match std::fs::read_to_string(&head_path) {
        Ok(head) => head,
        Err(_) => return,
    };
    if let Some(branch) = head.trim().strip_prefix("ref: refs/heads/") {
        let branch = branch.trim();
        let ref_path = common_dir.join("refs/heads").join(branch);
        if ref_path.exists() {
            println!("cargo:rerun-if-changed={}", ref_path.display());
        }
        let packed_refs = common_dir.join("packed-refs");
        if packed_refs.exists() {
            println!("cargo:rerun-if-changed={}", packed_refs.display());
        }
    }
}
