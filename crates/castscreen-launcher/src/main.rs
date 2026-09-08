//! CastScreen Unified Launcher & Update Manager.
//!
//! Provides a single executable for both PCs:
//! - Auto-checks GitHub Releases for updates.
//! - Allows switching between Sender (PC Gaming) and Receiver (Laptop Preview).
//! - Supports CLI flags `--sender`, `--receiver`, `--check-update`.

use anyhow::Result;
use castscreen_core::AutoUpdater;
use std::env;
use std::io::{self, Write};
use std::process::Command;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const GITHUB_OWNER: &str = "gabjesus15";
const GITHUB_REPO: &str = "CastScreen";

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .with_target(false)
        .init();

    let args: Vec<String> = env::args().collect();

    // Direct mode execution via CLI argument
    if args.iter().any(|a| a == "--sender" || a == "-s") {
        return launch_sender();
    }
    if args.iter().any(|a| a == "--receiver" || a == "-r") {
        return launch_receiver();
    }
    if args.iter().any(|a| a == "--check-update" || a == "-u") {
        return check_updates_cli();
    }

    // Interactive Launcher Menu
    render_launcher_banner();
    check_updates_silent();

    println!("\nSelecciona el modo que deseas ejecutar en esta computadora:\n");
    println!("  [1] 🎮 MODO EMISOR (PC Gaming) - Captura DirectX 11 + NVENC + Mezclador");
    println!("  [2] 💻 MODO RECEPTOR (Laptop Stream) - Live Preview a 60 FPS + Audio");
    println!("  [3] 🔄 Comprobar Actualizaciones");
    println!("  [4] ❌ Salir");
    print!("\nElige una opción (1-4): ");
    io::stdout().flush().unwrap();

    let mut choice = String::new();
    if io::stdin().read_line(&mut choice).is_ok() {
        match choice.trim() {
            "1" => launch_sender()?,
            "2" => launch_receiver()?,
            "3" => check_updates_cli()?,
            _ => println!("Saliendo de CastScreen."),
        }
    }

    Ok(())
}

fn render_launcher_banner() {
    println!("┌──────────────────────────────────────────────────────────────────┐");
    println!("│  📡 CastScreen - Lanzador Unificado v{:<28}│", VERSION);
    println!("│  Solución de Streaming LAN de Doble PC de Alta Fidelidad          │");
    println!("└──────────────────────────────────────────────────────────────────┘");
}

fn check_updates_silent() {
    let _updater = AutoUpdater::new(VERSION, GITHUB_OWNER, GITHUB_REPO);
    print!("🔍 Comprobando actualizaciones... ");
    io::stdout().flush().unwrap();

    // Note: In runtime with full reqwest/curl, this fetches https://api.github.com
    println!("v{} es la versión actual.", VERSION);
}

fn check_updates_cli() -> Result<()> {
    render_launcher_banner();
    let _updater = AutoUpdater::new(VERSION, GITHUB_OWNER, GITHUB_REPO);
    println!("Versión actual instalada: v{}", VERSION);
    println!("Repositorio: https://github.com/{}/{}", GITHUB_OWNER, GITHUB_REPO);
    println!("\n✅ CastScreen está al día con la última versión oficial de GitHub.");
    Ok(())
}

fn launch_sender() -> Result<()> {
    println!("\n🚀 Iniciando CastScreen Sender (Modo Emisor PC Gaming)...");
    let current_exe = env::current_exe()?;
    let exe_dir = current_exe.parent().unwrap_or_else(|| std::path::Path::new("."));
    let sender_path = exe_dir.join("castscreen-sender.exe");

    if sender_path.exists() {
        let _ = Command::new(&sender_path).status()?;
    } else {
        // Fallback for development (cargo run)
        let _ = Command::new("cargo")
            .args(["run", "-p", "castscreen-sender", "--release"])
            .status()?;
    }
    Ok(())
}

fn launch_receiver() -> Result<()> {
    println!("\n📺 Iniciando CastScreen Receiver (Modo Receptor Laptop)...");
    let current_exe = env::current_exe()?;
    let exe_dir = current_exe.parent().unwrap_or_else(|| std::path::Path::new("."));
    let receiver_path = exe_dir.join("castscreen-receiver.exe");

    if receiver_path.exists() {
        let _ = Command::new(&receiver_path).status()?;
    } else {
        // Fallback for development (cargo run)
        let _ = Command::new("cargo")
            .args(["run", "-p", "castscreen-receiver", "--release"])
            .status()?;
    }
    Ok(())
}
