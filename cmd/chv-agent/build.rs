use std::process::Command;

fn main() {
    let crate_root = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());

    let version = std::fs::read_to_string(format!("{}/../../VERSION", crate_root))
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "unknown".to_string());

    let git_sha = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .current_dir(&crate_root)
        .output()
        .ok()
        .and_then(|o| if o.status.success() { Some(o) } else { None })
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let build_date = Command::new("date")
        .args(["+%Y-%m-%d"])
        .output()
        .ok()
        .and_then(|o| if o.status.success() { Some(o) } else { None })
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let channel = std::env::var("CHV_RELEASE_CHANNEL").unwrap_or_else(|_| "stable".to_string());

    println!("cargo:rustc-env=CHV_VERSION={}", version);
    println!("cargo:rustc-env=CHV_GIT_SHA={}", git_sha);
    println!("cargo:rustc-env=CHV_BUILD_DATE={}", build_date);
    println!("cargo:rustc-env=CHV_RELEASE_CHANNEL={}", channel);
}
