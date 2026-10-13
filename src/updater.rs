use std::path::PathBuf;
use std::process::{Command, Stdio};

pub const REPO_GIT_URL: &str = "https://github.com/jonecat/omaflac.git";
pub const RAW_CARGO_TOML_URL: &str =
    "https://raw.githubusercontent.com/jonecat/omaflac/main/Cargo.toml";
pub const GITHUB_COMMITS_API_URL: &str =
    "https://api.github.com/repos/jonecat/omaflac/commits/main";
pub const GITHUB_COMMITS_LIST_URL: &str =
    "https://api.github.com/repos/jonecat/omaflac/commits?sha=main&per_page=1";
pub const INSTALL_SCRIPT_URL: &str =
    "https://raw.githubusercontent.com/jonecat/omaflac/main/install.sh";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateStatus {
    Idle,
    Checking,
    UpToDate {
        version: String,
        commit: Option<String>,
    },
    UpdateAvailable {
        latest_version: String,
        latest_commit: Option<String>,
    },
    Updating {
        step: String,
    },
    UpdatedReadyToRestart {
        new_version: String,
    },
    Error(String),
}

pub struct Updater;

impl Updater {
    pub fn current_version() -> &'static str {
        option_env!("OMAFLAC_DYNAMIC_VERSION").unwrap_or(env!("CARGO_PKG_VERSION"))
    }

    pub fn current_commit_count() -> Option<usize> {
        option_env!("OMAFLAC_COMMIT_COUNT").and_then(|s| s.parse::<usize>().ok())
    }

    pub fn current_commit() -> &'static str {
        option_env!("OMAFLAC_GIT_HASH").unwrap_or("unknown")
    }

    pub fn display_version() -> String {
        let ver = Self::current_version();
        let commit = Self::current_commit();
        if commit != "unknown" && !commit.is_empty() {
            format!("v{} ({})", ver, commit)
        } else {
            format!("v{}", ver)
        }
    }

    pub fn parse_semver(ver: &str) -> (u32, u32, u32) {
        let clean = ver.trim().trim_start_matches('v');
        let mut parts = clean.split('.').map(|p| p.parse::<u32>().unwrap_or(0));
        (
            parts.next().unwrap_or(0),
            parts.next().unwrap_or(0),
            parts.next().unwrap_or(0),
        )
    }

    fn parse_version_from_cargo_toml(contents: &str) -> Option<String> {
        let mut in_package = false;
        for line in contents.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with('[') {
                in_package = trimmed == "[package]";
                continue;
            }
            if in_package && trimmed.starts_with("version") {
                if let Some((_, val)) = trimmed.split_once('=') {
                    let v = val.trim().trim_matches('"').trim_matches('\'');
                    if !v.is_empty() {
                        return Some(v.to_string());
                    }
                }
            }
        }
        None
    }

    fn parse_last_page_from_link_header(header: &str) -> Option<usize> {
        for part in header.split(',') {
            if part.contains("rel=\"last\"") {
                if let Some(start) = part.find("&page=") {
                    let slice = &part[start + 6..];
                    let num_str: String = slice.chars().take_while(|c| c.is_ascii_digit()).collect();
                    if let Ok(n) = num_str.parse::<usize>() {
                        return Some(n);
                    }
                }
            }
        }
        None
    }

    fn fetch_remote_commit_count_and_sha() -> (Option<usize>, Option<String>) {
        let out = Command::new("curl")
            .args([
                "-fsSI",
                "--max-time",
                "4",
                "-H",
                "Accept: application/vnd.github.v3+json",
                "-A",
                "OmaFLAC-Updater",
                GITHUB_COMMITS_LIST_URL,
            ])
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output();

        let mut remote_count = None;
        if let Ok(resp) = out {
            if resp.status.success() {
                let headers = String::from_utf8_lossy(&resp.stdout);
                for line in headers.lines() {
                    if line.to_ascii_lowercase().starts_with("link:") {
                        remote_count = Self::parse_last_page_from_link_header(line);
                        break;
                    }
                }
            }
        }

        let sha_out = Command::new("curl")
            .args([
                "-fsSL",
                "--max-time",
                "4",
                "-H",
                "Accept: application/vnd.github.v3+json",
                "-A",
                "OmaFLAC-Updater",
                GITHUB_COMMITS_API_URL,
            ])
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output();

        let remote_sha = sha_out
            .ok()
            .and_then(|out| {
                if out.status.success() {
                    serde_json::from_slice::<serde_json::Value>(&out.stdout).ok()
                } else {
                    None
                }
            })
            .and_then(|json| {
                json.get("sha")
                    .and_then(|v| v.as_str())
                    .map(|sha| sha.chars().take(7).collect::<String>())
            });

        (remote_count, remote_sha)
    }

    fn fetch_remote_version_and_commit() -> Result<(String, Option<String>, Option<usize>), String> {
        let (remote_commits, remote_sha) = Self::fetch_remote_commit_count_and_sha();

        if let Some(commits) = remote_commits {
            let ver = format!("0.1.{}", commits);
            return Ok((ver, remote_sha, Some(commits)));
        }

        // Fallback to checking Cargo.toml if GitHub commits list API is unavailable
        let cargo_out = Command::new("curl")
            .args(["-fsSL", "--max-time", "4", RAW_CARGO_TOML_URL])
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()
            .map_err(|e| format!("Failed to invoke curl: {}", e))?;

        if !cargo_out.status.success() {
            return Err("Could not reach GitHub to check for updates".to_string());
        }

        let cargo_str = String::from_utf8_lossy(&cargo_out.stdout);
        let remote_version = Self::parse_version_from_cargo_toml(&cargo_str)
            .ok_or_else(|| "Could not parse remote Cargo.toml version".to_string())?;

        Ok((remote_version, remote_sha, None))
    }

    /// Checks if `remote_commit` is already an ancestor of a local git checkout's HEAD
    /// so local development builds ahead of `origin/main` are not flagged as outdated.
    fn is_commit_ancestor_locally(remote_commit: &str) -> bool {
        Command::new("git")
            .args(["merge-base", "--is-ancestor", remote_commit, "HEAD"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    pub fn check_remote() -> UpdateStatus {
        match Self::fetch_remote_version_and_commit() {
            Ok((remote_ver, remote_commit, remote_commits_opt)) => {
                let local_ver = Self::current_version();
                let local_commit = Self::current_commit();
                let local_commits_opt = Self::current_commit_count();

                // If both remote and local have commit counts on 0.1.x, compare commit counts directly
                if let (Some(r_count), Some(l_count)) = (remote_commits_opt, local_commits_opt) {
                    if r_count > l_count {
                        return UpdateStatus::UpdateAvailable {
                            latest_version: remote_ver,
                            latest_commit: remote_commit,
                        };
                    } else {
                        return UpdateStatus::UpToDate {
                            version: local_ver.to_string(),
                            commit: if local_commit != "unknown" {
                                Some(local_commit.to_string())
                            } else {
                                remote_commit
                            },
                        };
                    }
                }

                let remote_tuple = Self::parse_semver(&remote_ver);
                let local_tuple = Self::parse_semver(local_ver);

                if remote_tuple > local_tuple {
                    return UpdateStatus::UpdateAvailable {
                        latest_version: remote_ver,
                        latest_commit: remote_commit,
                    };
                }

                if remote_tuple == local_tuple {
                    if let Some(ref r_sha) = remote_commit {
                        if local_commit != "unknown"
                            && !local_commit.is_empty()
                            && !r_sha.starts_with(local_commit)
                            && !local_commit.starts_with(r_sha)
                            && !Self::is_commit_ancestor_locally(r_sha)
                        {
                            return UpdateStatus::UpdateAvailable {
                                latest_version: remote_ver,
                                latest_commit: remote_commit,
                            };
                        }
                    }
                }

                UpdateStatus::UpToDate {
                    version: local_ver.to_string(),
                    commit: if local_commit != "unknown" {
                        Some(local_commit.to_string())
                    } else {
                        remote_commit
                    },
                }
            }
            Err(err) => UpdateStatus::Error(err),
        }
    }

    pub fn spawn_check() -> crossbeam_channel::Receiver<UpdateStatus> {
        let (tx, rx) = crossbeam_channel::unbounded();
        let _ = tx.send(UpdateStatus::Checking);
        std::thread::spawn(move || {
            let status = Self::check_remote();
            let _ = tx.send(status);
        });
        rx
    }

    pub fn install_binary_path() -> PathBuf {
        if let Ok(custom) = std::env::var("OMAFLAC_INSTALL_DIR") {
            return PathBuf::from(custom).join("omaflac");
        }
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        home.join(".local").join("bin").join("omaflac")
    }

    pub fn spawn_in_app_update() -> crossbeam_channel::Receiver<UpdateStatus> {
        let (tx, rx) = crossbeam_channel::unbounded();
        std::thread::spawn(move || {
            let _ = tx.send(UpdateStatus::Updating {
                step: "Cloning latest OmaFLAC source from GitHub...".to_string(),
            });

            let pid = std::process::id();
            let tmp_dir = std::env::temp_dir().join(format!("omaflac-update-{}", pid));
            let _ = std::fs::remove_dir_all(&tmp_dir);

            let clone_status = Command::new("git")
                .args([
                    "clone",
                    REPO_GIT_URL,
                    &tmp_dir.to_string_lossy(),
                ])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();

            if !clone_status.map(|s| s.success()).unwrap_or(false) {
                let _ = std::fs::remove_dir_all(&tmp_dir);
                let _ = tx.send(UpdateStatus::Error(
                    "Failed to clone OmaFLAC repository from GitHub".to_string(),
                ));
                return;
            }

            let new_count = Command::new("git")
                .args(["rev-list", "--count", "HEAD"])
                .current_dir(&tmp_dir)
                .stdin(Stdio::null())
                .stderr(Stdio::null())
                .output()
                .ok()
                .and_then(|o| String::from_utf8(o.stdout).ok())
                .and_then(|s| s.trim().parse::<usize>().ok());

            let new_ver = if let Some(count) = new_count {
                format!("0.1.{}", count)
            } else {
                std::fs::read_to_string(tmp_dir.join("Cargo.toml"))
                    .ok()
                    .and_then(|c| Self::parse_version_from_cargo_toml(&c))
                    .unwrap_or_else(|| Self::current_version().to_string())
            };

            let new_sha = Command::new("git")
                .args(["rev-parse", "--short", "HEAD"])
                .current_dir(&tmp_dir)
                .stdin(Stdio::null())
                .stderr(Stdio::null())
                .output()
                .ok()
                .and_then(|o| String::from_utf8(o.stdout).ok())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());

            let _ = tx.send(UpdateStatus::Updating {
                step: "Compiling release binary (cargo build --release)...".to_string(),
            });

            // Ensure ~/.cargo/bin is on PATH for background thread
            let mut path_env = std::env::var("PATH").unwrap_or_default();
            if let Some(home) = dirs::home_dir() {
                let cargo_bin = home.join(".cargo").join("bin");
                if cargo_bin.exists() {
                    path_env = format!("{}:{}", cargo_bin.display(), path_env);
                }
            }

            let build_status = Command::new("cargo")
                .args(["build", "--release"])
                .current_dir(&tmp_dir)
                .env("PATH", &path_env)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();

            if !build_status.map(|s| s.success()).unwrap_or(false) {
                let _ = std::fs::remove_dir_all(&tmp_dir);
                let _ = tx.send(UpdateStatus::Error(
                    "Release build (cargo build --release) failed".to_string(),
                ));
                return;
            }

            let _ = tx.send(UpdateStatus::Updating {
                step: "Installing binary to ~/.local/bin/omaflac...".to_string(),
            });

            let built_bin = tmp_dir.join("target").join("release").join("omaflac");
            let dest_bin = Self::install_binary_path();
            if let Some(parent) = dest_bin.parent() {
                let _ = std::fs::create_dir_all(parent);
            }

            // Unlink existing binary first so running executable does not cause ETXTBSY ("Text file busy")
            let _ = std::fs::remove_file(&dest_bin);
            if let Err(e) = std::fs::copy(&built_bin, &dest_bin) {
                let _ = std::fs::remove_dir_all(&tmp_dir);
                let _ = tx.send(UpdateStatus::Error(format!(
                    "Failed to install binary: {}",
                    e
                )));
                return;
            }

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Ok(meta) = std::fs::metadata(&dest_bin) {
                    let mut perms = meta.permissions();
                    perms.set_mode(0o755);
                    let _ = std::fs::set_permissions(&dest_bin, perms);
                }
            }

            let _ = std::fs::remove_dir_all(&tmp_dir);

            let version_label = match new_sha {
                Some(sha) => format!("v{} ({})", new_ver, sha),
                None => format!("v{}", new_ver),
            };
            let _ = tx.send(UpdateStatus::UpdatedReadyToRestart {
                new_version: version_label,
            });
        });
        rx
    }

    pub fn run_cli_check() -> i32 {
        println!("OmaFLAC {}", Self::display_version());
        println!("Checking for updates from GitHub...");
        match Self::check_remote() {
            UpdateStatus::UpToDate { version, commit } => {
                let c = commit.map(|s| format!(" ({})", s)).unwrap_or_default();
                println!("✓ You are running the latest version: v{}{}", version, c);
                0
            }
            UpdateStatus::UpdateAvailable {
                latest_version,
                latest_commit,
            } => {
                let c = latest_commit
                    .map(|s| format!(" ({})", s))
                    .unwrap_or_default();
                println!(
                    "↑ Update available: v{}{} (current: {})",
                    latest_version,
                    c,
                    Self::display_version()
                );
                println!("Run 'omaflac update' to install the latest version.");
                0
            }
            UpdateStatus::Error(e) => {
                eprintln!("✗ Update check failed: {}", e);
                1
            }
            _ => 0,
        }
    }

    pub fn run_cli_update(force: bool) -> i32 {
        println!("OmaFLAC {}", Self::display_version());
        if !force {
            println!("Checking for updates...");
            match Self::check_remote() {
                UpdateStatus::UpToDate { version, commit } => {
                    let c = commit.map(|s| format!(" ({})", s)).unwrap_or_default();
                    println!(
                        "✓ OmaFLAC v{}{} is already up to date! (Use 'omaflac update --force' to reinstall)",
                        version, c
                    );
                    return 0;
                }
                UpdateStatus::UpdateAvailable {
                    latest_version,
                    latest_commit,
                } => {
                    let c = latest_commit
                        .map(|s| format!(" ({})", s))
                        .unwrap_or_default();
                    println!("↑ Updating OmaFLAC to v{}{}...", latest_version, c);
                }
                UpdateStatus::Error(e) => {
                    eprintln!("! Could not verify remote version ({}), proceeding with update...", e);
                }
                _ => {}
            }
        }

        let cmd = format!(
            "curl -fsSL {} | OMAFLAC_FORCE_CLONE=1 bash",
            INSTALL_SCRIPT_URL
        );
        match Command::new("bash").args(["-c", &cmd]).status() {
            Ok(status) => status.code().unwrap_or(1),
            Err(e) => {
                eprintln!("✗ Failed to launch installer: {}", e);
                1
            }
        }
    }
}
