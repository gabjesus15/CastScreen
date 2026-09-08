//! Auto-Updater system connected to GitHub Releases.
//!
//! Checks the latest release tag on GitHub, compares semantic versions,
//! and notifies the streamer when an update is available with direct download links.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GithubAsset {
    pub name: String,
    pub browser_download_url: String,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GithubRelease {
    pub tag_name: String,
    pub name: Option<String>,
    pub body: Option<String>,
    pub html_url: String,
    pub published_at: Option<String>,
    pub assets: Vec<GithubAsset>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateStatus {
    /// Running the latest available version
    UpToDate,
    /// A newer version was found on GitHub Releases
    UpdateAvailable {
        new_version: String,
        download_url: Option<String>,
        release_notes: String,
    },
    /// Could not connect to GitHub API
    CheckFailed(String),
}

pub struct AutoUpdater {
    pub current_version: String,
    pub repo_owner: String,
    pub repo_name: String,
}

impl AutoUpdater {
    pub fn new(current_version: &str, repo_owner: &str, repo_name: &str) -> Self {
        Self {
            current_version: current_version.trim_start_matches('v').to_string(),
            repo_owner: repo_owner.to_string(),
            repo_name: repo_name.to_string(),
        }
    }

    /// Returns the GitHub API URL for the latest release.
    pub fn api_url(&self) -> String {
        format!(
            "https://api.github.com/repos/{}/{}/releases/latest",
            self.repo_owner, self.repo_name
        )
    }

    /// Evaluates whether the remote tag represents a newer semantic version.
    pub fn compare_versions(local: &str, remote_tag: &str) -> bool {
        let remote = remote_tag.trim_start_matches('v');
        let local_parts: Vec<u32> = local.split('.').filter_map(|s| s.parse().ok()).collect();
        let remote_parts: Vec<u32> = remote.split('.').filter_map(|s| s.parse().ok()).collect();

        if local_parts.len() == 3 && remote_parts.len() == 3 {
            if remote_parts[0] > local_parts[0] {
                return true;
            }
            if remote_parts[0] == local_parts[0] && remote_parts[1] > local_parts[1] {
                return true;
            }
            if remote_parts[0] == local_parts[0]
                && remote_parts[1] == local_parts[1]
                && remote_parts[2] > local_parts[2]
            {
                return true;
            }
        }
        false
    }

    /// Parses a JSON response from GitHub Releases and checks for updates.
    pub fn parse_release_response(&self, json_str: &str) -> UpdateStatus {
        match serde_json::from_str::<GithubRelease>(json_str) {
            Ok(release) => {
                if Self::compare_versions(&self.current_version, &release.tag_name) {
                    let installer_asset = release
                        .assets
                        .iter()
                        .find(|a| a.name.ends_with(".exe") || a.name.contains("Setup"))
                        .map(|a| a.browser_download_url.clone())
                        .or_else(|| Some(release.html_url.clone()));

                    UpdateStatus::UpdateAvailable {
                        new_version: release.tag_name,
                        download_url: installer_asset,
                        release_notes: release.body.unwrap_or_default(),
                    }
                } else {
                    UpdateStatus::UpToDate
                }
            }
            Err(e) => UpdateStatus::CheckFailed(e.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_comparison() {
        assert!(AutoUpdater::compare_versions("0.1.0", "v0.1.1"));
        assert!(AutoUpdater::compare_versions("0.1.0", "v0.2.0"));
        assert!(AutoUpdater::compare_versions("0.1.0", "v1.0.0"));
        assert!(!AutoUpdater::compare_versions("0.1.1", "v0.1.0"));
        assert!(!AutoUpdater::compare_versions("0.1.0", "v0.1.0"));
    }

    #[test]
    fn test_parse_release_json() {
        let updater = AutoUpdater::new("0.1.0", "gabriel", "CastScreen");
        let sample_json = r#"{
            "tag_name": "v0.2.0",
            "html_url": "https://github.com/gabriel/CastScreen/releases/tag/v0.2.0",
            "body": "Nuevas mejoras de audio y latencia",
            "assets": [
                {
                    "name": "CastScreen_Setup.exe",
                    "browser_download_url": "https://github.com/gabriel/CastScreen/releases/download/v0.2.0/CastScreen_Setup.exe",
                    "size": 15000000
                }
            ]
        }"#;

        let status = updater.parse_release_response(sample_json);
        match status {
            UpdateStatus::UpdateAvailable {
                new_version,
                download_url,
                ..
            } => {
                assert_eq!(new_version, "v0.2.0");
                assert_eq!(
                    download_url,
                    Some("https://github.com/gabriel/CastScreen/releases/download/v0.2.0/CastScreen_Setup.exe".to_string())
                );
            }
            _ => panic!("Expected update available"),
        }
    }
}
