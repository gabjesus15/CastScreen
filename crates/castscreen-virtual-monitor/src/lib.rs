use anyhow::{Context, Result};
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::process::Command;
use tracing::info;
use directories::ProjectDirs;

const ZIP_URL: &str = "https://github.com/MolotovCherry/virtual-display-rs/releases/download/v0.3.1/virtual-desktop-driver-portable-x64.zip";
const NEFCONC_URL: &str = "https://github.com/MolotovCherry/virtual-display-rs/raw/master/installer/files/nefconc.exe";

pub fn is_installed() -> bool {
    let dir = get_driver_dir();
    dir.join("VirtualDisplayDriver.dll").exists() && dir.join("nefconc.exe").exists()
    // To be fully robust, we would also check if the driver is installed via devcon/pnputil
}

fn get_driver_dir() -> PathBuf {
    if let Some(proj_dirs) = ProjectDirs::from("com", "CastScreen", "CastScreen") {
        proj_dirs.data_local_dir().join("VirtualMonitor")
    } else {
        std::env::temp_dir().join("CastScreenVirtualMonitor")
    }
}

pub fn install() -> Result<()> {
    let dir = get_driver_dir();
    if !dir.exists() {
        fs::create_dir_all(&dir).context("Failed to create driver directory")?;
    }

    info!("Downloading driver zip...");
    let mut zip_data = ureq::get(ZIP_URL).call()?.into_reader();
    let mut zip_bytes = Vec::new();
    std::io::copy(&mut zip_data, &mut zip_bytes)?;

    info!("Extracting driver zip...");
    let mut archive = zip::ZipArchive::new(Cursor::new(zip_bytes))?;
    archive.extract(&dir)?;

    info!("Downloading nefconc.exe...");
    let mut nefconc_file = fs::File::create(dir.join("nefconc.exe"))?;
    let mut nefconc_data = ureq::get(NEFCONC_URL).call()?.into_reader();
    std::io::copy(&mut nefconc_data, &mut nefconc_file)?;

    install_system_components(&dir)?;
    
    Ok(())
}

fn install_system_components(dir: &Path) -> Result<()> {
    info!("Writing install script...");
    let bat_path = dir.join("install_vm.bat");
    let script = 
        "@echo off\r\n\
        cd /d \"%~dp0\"\r\n\
        certutil -addstore -f root DriverCertificate.cer\r\n\
        certutil -addstore -f TrustedPublisher DriverCertificate.cer\r\n\
        reg import install.reg\r\n\
        .\\nefconc.exe --create-device-node --class-name Display --class-guid \"4D36E968-E325-11CE-BFC1-08002BE10318\" --hardware-id Root\\VirtualDisplayDriver\r\n\
        .\\nefconc.exe --install-driver --inf-path VirtualDisplayDriver.inf\r\n\
        reg add \"HKLM\\SOFTWARE\\VirtualDisplayDriver\" /v data /t REG_SZ /d \"[{\\\"id\\\":0,\\\"name\\\":\\\"Virtual Display\\\",\\\"modes\\\":[{\\\"width\\\":1920,\\\"height\\\":1080,\\\"refresh_rates\\\":[60]}]}]\" /f\r\n\
        ";
    fs::write(&bat_path, script)?;

    info!("Requesting elevation to install driver...");
    // Run the batch file elevated using powershell without waiting so we don't block the UI thread indefinitely
    let status = Command::new("powershell")
        .args([
            "-NoProfile",
            "-WindowStyle", "Hidden",
            "-Command",
            &format!("Start-Process -FilePath '{}' -Verb RunAs", bat_path.display())
        ])
        .status()?;

    if !status.success() {
        anyhow::bail!("Failed to execute installation script.");
    }
    
    Ok(())
}
