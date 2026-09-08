//! Native In-App Updater for CastScreen.
//!
//! Automatically checks GitHub Releases for new updates in a non-blocking background thread,
//! provides streaming download with real-time byte progress, and executes the installer
//! with `/CLOSEAPPLICATIONS` for seamless 1-click upgrades without opening a web browser.

use parking_lot::Mutex;
use serde::Deserialize;
use std::fs::File;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;

pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const GITHUB_REPO: &str = "gabjesus15/CastScreen";

pub type AutoUpdater = AppUpdater;
pub type UpdateStatus = UpdateState;

/// Lifecycle state of the auto-updater.
#[derive(Clone, Debug, PartialEq)]
pub enum UpdateState {
    /// Initial state or no action pending.
    Idle,
    /// Actively contacting GitHub Releases API in background.
    Checking,
    /// Current version is up to date.
    UpToDate,
    /// A newer release was found on GitHub.
    UpdateAvailable {
        version: String,
        title: String,
        notes: String,
        download_url: String,
        asset_size: u64,
    },
    /// Currently downloading installer binary with byte progress.
    Downloading {
        version: String,
        downloaded_bytes: u64,
        total_bytes: u64,
        progress_pct: f32,
    },
    /// Installer is downloaded and ready to execute.
    ReadyToInstall {
        version: String,
        installer_path: PathBuf,
    },
    /// An error occurred during check or download.
    Error(String),
}

#[derive(Deserialize)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
    size: u64,
}

#[derive(Deserialize)]
struct GitHubRelease {
    tag_name: String,
    name: Option<String>,
    body: Option<String>,
    assets: Vec<GitHubAsset>,
}

/// Thread-safe controller for checking, downloading, and installing updates.
#[derive(Clone)]
pub struct AppUpdater {
    state: Arc<Mutex<UpdateState>>,
    repo: String,
}

impl Default for AppUpdater {
    fn default() -> Self {
        Self::new()
    }
}

impl AppUpdater {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(UpdateState::Idle)),
            repo: GITHUB_REPO.to_string(),
        }
    }

    /// Returns the current state of the updater.
    pub fn get_state(&self) -> UpdateState {
        self.state.lock().clone()
    }

    /// Spawns a background thread to check GitHub Releases for a newer version.
    pub fn check_for_updates(&self) {
        let state_clone = self.state.clone();
        let repo = self.repo.clone();

        {
            let mut state = state_clone.lock();
            *state = UpdateState::Checking;
        }

        thread::spawn(move || {
            let url = format!("https://api.github.com/repos/{}/releases/latest", repo);

            let resp = match ureq::get(&url)
                .set("User-Agent", "CastScreen-AutoUpdater")
                .set("Accept", "application/vnd.github.v3+json")
                .timeout(std::time::Duration::from_secs(8))
                .call()
            {
                Ok(r) => r,
                Err(e) => {
                    let mut state = state_clone.lock();
                    *state = UpdateState::Error(format!("Error de conexión con GitHub: {}", e));
                    return;
                }
            };

            let release: GitHubRelease = match resp.into_json() {
                Ok(json) => json,
                Err(e) => {
                    let mut state = state_clone.lock();
                    *state = UpdateState::Error(format!("Error decodificando respuesta de GitHub: {}", e));
                    return;
                }
            };

            let remote_version = release.tag_name.trim_start_matches('v').trim_start_matches('V');
            if is_newer_version(CURRENT_VERSION, remote_version) {
                // Find installer asset (CastScreen_Setup.exe or any .exe)
                let installer_asset = release
                    .assets
                    .iter()
                    .find(|a| a.name.to_lowercase().contains("setup") && a.name.ends_with(".exe"))
                    .or_else(|| release.assets.iter().find(|a| a.name.ends_with(".exe")));

                if let Some(asset) = installer_asset {
                    let mut state = state_clone.lock();
                    *state = UpdateState::UpdateAvailable {
                        version: release.tag_name.clone(),
                        title: release.name.unwrap_or_else(|| release.tag_name.clone()),
                        notes: release.body.unwrap_or_else(|| "Actualización recomendada.".to_string()),
                        download_url: asset.browser_download_url.clone(),
                        asset_size: asset.size,
                    };
                } else {
                    let mut state = state_clone.lock();
                    *state = UpdateState::Error("Se encontró una nueva versión pero no contiene instalador ejecutable.".to_string());
                }
            } else {
                let mut state = state_clone.lock();
                *state = UpdateState::UpToDate;
            }
        });
    }

    /// Starts streaming download of the installer in background.
    pub fn start_download(&self) {
        let (version, download_url, expected_size) = {
            let state = self.state.lock();
            match &*state {
                UpdateState::UpdateAvailable {
                    version,
                    download_url,
                    asset_size,
                    ..
                } => (version.clone(), download_url.clone(), *asset_size),
                _ => return,
            }
        };

        let state_clone = self.state.clone();

        {
            let mut state = state_clone.lock();
            *state = UpdateState::Downloading {
                version: version.clone(),
                downloaded_bytes: 0,
                total_bytes: expected_size,
                progress_pct: 0.0,
            };
        }

        thread::spawn(move || {
            let temp_dir = std::env::temp_dir();
            let sanitized_ver = version.replace(['/', '\\', ':'], "_");
            let target_path = temp_dir.join(format!("CastScreen_Setup_{}.exe", sanitized_ver));

            let resp = match ureq::get(&download_url)
                .set("User-Agent", "CastScreen-AutoUpdater")
                .timeout(std::time::Duration::from_secs(120))
                .call()
            {
                Ok(r) => r,
                Err(e) => {
                    let mut state = state_clone.lock();
                    *state = UpdateState::Error(format!("Error iniciando descarga: {}", e));
                    return;
                }
            };

            let total_len: u64 = resp
                .header("Content-Length")
                .and_then(|h| h.parse().ok())
                .unwrap_or(expected_size);

            let mut reader = resp.into_reader();
            let mut file = match File::create(&target_path) {
                Ok(f) => f,
                Err(e) => {
                    let mut state = state_clone.lock();
                    *state = UpdateState::Error(format!("Error creando archivo temporal: {}", e));
                    return;
                }
            };

            let mut buffer = [0u8; 65536]; // 64 KB chunks
            let mut downloaded: u64 = 0;

            loop {
                match reader.read(&mut buffer) {
                    Ok(0) => break, // EOF
                    Ok(n) => {
                        if let Err(e) = file.write_all(&buffer[..n]) {
                            let mut state = state_clone.lock();
                            *state = UpdateState::Error(format!("Error escribiendo datos: {}", e));
                            let _ = std::fs::remove_file(&target_path);
                            return;
                        }
                        downloaded += n as u64;

                        let progress = if total_len > 0 {
                            (downloaded as f32 / total_len as f32).clamp(0.0, 1.0)
                        } else {
                            0.5
                        };

                        let mut state = state_clone.lock();
                        *state = UpdateState::Downloading {
                            version: version.clone(),
                            downloaded_bytes: downloaded,
                            total_bytes: total_len,
                            progress_pct: progress,
                        };
                    }
                    Err(e) => {
                        let mut state = state_clone.lock();
                        *state = UpdateState::Error(format!("Descarga interrumpida: {}", e));
                        let _ = std::fs::remove_file(&target_path);
                        return;
                    }
                }
            }

            let _ = file.flush();

            let mut state = state_clone.lock();
            *state = UpdateState::ReadyToInstall {
                version,
                installer_path: target_path,
            };
        });
    }

    /// Executes the downloaded installer and terminates the current application cleanly.
    pub fn install_and_restart(&self) -> Result<(), String> {
        let installer_path = {
            let state = self.state.lock();
            match &*state {
                UpdateState::ReadyToInstall { installer_path, .. } => installer_path.clone(),
                _ => return Err("El instalador aún no está listo para ejecutar.".to_string()),
            }
        };

        if !installer_path.exists() {
            return Err("No se encontró el instalador descargado en disco.".to_string());
        }

        // Launch installer with /CLOSEAPPLICATIONS so Inno Setup replaces running binaries
        let spawn_res = std::process::Command::new(&installer_path)
            .arg("/CLOSEAPPLICATIONS")
            .spawn();

        match spawn_res {
            Ok(_) => {
                // Exit current process cleanly to release file locks
                std::process::exit(0);
            }
            Err(e) => Err(format!("No se pudo ejecutar el instalador: {}", e)),
        }
    }
}

/// Compares two semantic versions. Returns true if `remote` is strictly newer than `current`.
pub fn is_newer_version(current: &str, remote: &str) -> bool {
    let parse = |v: &str| -> Vec<u32> {
        let clean = v.trim().trim_start_matches(['v', 'V']);
        clean
            .split('.')
            .filter_map(|p| {
                let num_part: String = p.chars().take_while(|c| c.is_ascii_digit()).collect();
                num_part.parse::<u32>().ok()
            })
            .collect()
    };

    let mut cur = parse(current);
    let mut rem = parse(remote);

    // Normalize length by padding with zeros (e.g. 0.2.0 vs 0.2.0.1)
    let max_len = cur.len().max(rem.len());
    cur.resize(max_len, 0);
    rem.resize(max_len, 0);

    rem > cur
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_comparison() {
        assert!(is_newer_version("0.2.0", "v0.2.1"));
        assert!(is_newer_version("0.2.0", "0.3.0"));
        assert!(is_newer_version("0.2.0", "1.0.0"));
        assert!(is_newer_version("0.2.0", "v0.2.0.1"));
        assert!(is_newer_version("0.2", "0.2.1"));
        assert!(is_newer_version("0.2.0.0", "v0.2.0.1"));

        assert!(!is_newer_version("0.2.0", "v0.2.0"));
        assert!(!is_newer_version("0.2.0", "0.2"));
        assert!(!is_newer_version("0.2.0", "v0.2.0.0"));
        assert!(!is_newer_version("0.2.1", "0.2.0"));
        assert!(!is_newer_version("1.0.0", "0.9.9"));
    }
}
