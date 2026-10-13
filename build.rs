use std::process::Command;

fn main() {
    let git_hash = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|out| {
            if out.status.success() {
                String::from_utf8(out.stdout).ok()
            } else {
                None
            }
        })
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_string());

    let commit_count = Command::new("git")
        .args(["rev-list", "--count", "HEAD"])
        .output()
        .ok()
        .and_then(|out| {
            if out.status.success() {
                String::from_utf8(out.stdout).ok()
            } else {
                None
            }
        })
        .and_then(|s| s.trim().parse::<usize>().ok());

    let major = env!("CARGO_PKG_VERSION_MAJOR");
    let minor = env!("CARGO_PKG_VERSION_MINOR");

    if let Some(count) = commit_count {
        let dynamic_version = format!("{}.{}.{}", major, minor, count);
        println!("cargo:rustc-env=OMAFLAC_DYNAMIC_VERSION={}", dynamic_version);
        println!("cargo:rustc-env=OMAFLAC_COMMIT_COUNT={}", count);
    }

    println!("cargo:rustc-env=OMAFLAC_GIT_HASH={}", git_hash);
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs/heads");
    println!("cargo:rerun-if-changed=.git/index");
}
