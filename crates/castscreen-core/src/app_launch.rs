//! Starting the other half of CastScreen.
//!
//! All three windows can hand off to another: the launcher starts a role, and
//! either role can go back to the launcher to swap. That is one behaviour, so
//! it is written once — three copies of "where is the binary" drift apart the
//! first time somebody changes the install layout.
//!
//! Going back is always available and always cheap. A choice you cannot undo
//! is a choice people avoid making, so the mode picker is never a one-way door.

use std::path::PathBuf;
use std::process::Command;

/// The launcher's executable name (see the `[[bin]]` entry in its manifest).
pub const LAUNCHER_BIN: &str = "CastScreen";
pub const SENDER_BIN: &str = "castscreen-sender";
pub const RECEIVER_BIN: &str = "castscreen-receiver";

/// Locate a sibling CastScreen binary, installed or built.
pub fn find_binary(binary_name: &str) -> Option<PathBuf> {
    let file = format!("{binary_name}.exe");

    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(dir) = current_exe.parent() {
            // Installed layout, and `cargo build` output, both put the
            // binaries side by side.
            let direct = dir.join(&file);
            if direct.exists() {
                return Some(direct);
            }
            for sibling in ["../release", "../debug"] {
                let candidate = dir.join(sibling).join(&file);
                if candidate.exists() {
                    return Some(candidate);
                }
            }
        }
    }

    for target in ["target/release", "target/debug"] {
        let candidate = std::path::Path::new(target).join(&file);
        if candidate.exists() {
            return Some(candidate);
        }
    }

    None
}

/// Start a CastScreen binary, falling back to `cargo run` when only sources
/// are present.
///
/// Returns `true` if a real binary was started, `false` if a build had to be
/// kicked off first — the caller uses that to decide whether it can close
/// itself immediately or should say "this will take a moment".
pub fn spawn(binary_name: &str, package: &str) -> std::io::Result<bool> {
    if let Some(path) = find_binary(binary_name) {
        Command::new(path).spawn()?;
        return Ok(true);
    }

    let mut cmd = Command::new("cargo");
    cmd.args(["run", "-p", package]);
    if !cfg!(debug_assertions) {
        cmd.arg("--release");
    }
    cmd.spawn()?;
    Ok(false)
}

pub fn launch_sender() -> std::io::Result<bool> {
    spawn(SENDER_BIN, "castscreen-sender")
}

pub fn launch_receiver() -> std::io::Result<bool> {
    spawn(RECEIVER_BIN, "castscreen-receiver")
}

/// Reopen the mode picker.
///
/// The launcher holds a single-instance mutex, so if it is already open this
/// simply brings the existing one back rather than stacking a second window.
pub fn launch_launcher() -> std::io::Result<bool> {
    spawn(LAUNCHER_BIN, "castscreen-launcher")
}
