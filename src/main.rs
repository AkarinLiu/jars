use clap::{Parser, Subcommand};
use dirs::home_dir;
use indicatif::ProgressBar;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;
use winreg::enums::HKEY_LOCAL_MACHINE;
use winreg::RegKey;
use zip::result::ZipError;

#[derive(Debug, Error)]
enum JarsError {
    #[error("Java version not found: {0}")]
    VersionNotFound(String),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Failed to download Java: {0}")]
    DownloadError(String),
    #[error("Zip archive error: {0}")]
    ZipError(#[from] ZipError),
}

#[derive(Parser)]
#[command(name = "jars", version = "0.1", author = "AkarinLiu")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Set the current Java version
    Use {
        version: String,
    },
    /// List installed Java versions
    List,
    /// Show current Java version
    Current,
    /// Install a new Java version
    Install {
        version: String,
    },
}

fn get_versions_dir() -> PathBuf {
    home_dir().unwrap().join(".jars").join("versions")
}

fn detect_system_java() -> Vec<String> {
    let mut versions = Vec::new();
    
    // Windows registry detection
    if cfg!(windows) {
        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
        if let Ok(java_soft) = hklm.open_subkey("SOFTWARE\\JavaSoft\\Java Runtime Environment") {
            if let Ok(current_version) = java_soft.get_value::<String, _>("CurrentVersion") {
                versions.push(current_version);
            }
        }
    }
    
    // Check common installation paths
    let common_paths = if cfg!(windows) {
        vec![
            PathBuf::from("C:\\Program Files\\Java"),
            PathBuf::from("C:\\Program Files (x86)\\Java"),
        ]
    } else {
        vec![
            PathBuf::from("/usr/lib/jvm"),
            PathBuf::from("/Library/Java/JavaVirtualMachines"),
        ]
    };
    
    for path in common_paths {
        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.filter_map(Result::ok) {
                if let Some(name) = entry.file_name().to_str() {
                    versions.push(name.to_string());
                }
            }
        }
    }
    
    versions
}

fn get_download_url(version: &str) -> Result<String, JarsError> {
    let os = if cfg!(windows) { "windows" } else if cfg!(target_os = "macos") { "mac" } else { "linux" };
    let arch = if cfg!(target_arch = "x86_64") { "x64" } else { "aarch64" };
    
    Ok(format!(
        "https://api.adoptium.net/v3/binary/latest/{}/ga/{}/{}/jdk/hotspot/normal/eclipse?project=jdk",
        version, os, arch
    ))
}

fn install_java(version: &str) -> Result<(), JarsError> {
    let versions_dir = get_versions_dir();
    let target_dir = versions_dir.join(version);
    
    if !target_dir.exists() {
        fs::create_dir_all(&target_dir)?;
    }
    
    let pb = ProgressBar::new(100);
    pb.set_message(format!("Downloading Java {}", version));
    
    // Get download URL
    let url = get_download_url(version)?;
    let response = reqwest::blocking::get(&url)
        .map_err(|e| JarsError::DownloadError(e.to_string()))?;
    
    if !response.status().is_success() {
        return Err(JarsError::DownloadError(format!(
            "Failed to download Java {}: {}",
            version,
            response.status()
        )));
    }
    
    // Read response into memory and create seekable cursor
    let bytes = response.bytes()
        .map_err(|e| JarsError::DownloadError(e.to_string()))?;
    let cursor = std::io::Cursor::new(bytes);
    
    // Download and extract
    let mut archive = zip::ZipArchive::new(cursor)
        .map_err(|e| JarsError::DownloadError(e.to_string()))?;
    
    let archive_len = archive.len();
    for i in 0..archive_len {
        let mut file = archive.by_index(i)?;
        let outpath = target_dir.join(file.mangled_name());
        
        if file.name().ends_with('/') {
            fs::create_dir_all(&outpath)?;
        } else {
            if let Some(p) = outpath.parent() {
                if !p.exists() {
                    fs::create_dir_all(p)?;
                }
            }
            
            let mut outfile = fs::File::create(&outpath)?;
            std::io::copy(&mut file, &mut outfile)?;
        }
        
        pb.set_position((i as u64 * 100 / archive_len as u64) as u64);
    }
    
    pb.finish_with_message(format!("Java {} installed successfully", version));
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Use { version } => {
            let java_path = get_versions_dir().join(&version).join("bin").join("java");
            
            if !java_path.exists() {
                return Err(Box::new(JarsError::VersionNotFound(version)));
            }
            
            env::set_var("JAVA_HOME", get_versions_dir().join(&version));
            let path_sep = if cfg!(windows) { ";" } else { ":" };
            env::set_var(
                "PATH", 
                format!("{}{}{}", java_path.parent().unwrap().display(), path_sep, env::var("PATH")?)
            );
            
            println!("Now using Java version {}", version);
        }
        Commands::List => {
            let versions_dir = get_versions_dir();
            let mut versions = Vec::new();
            
            if versions_dir.exists() {
                for entry in fs::read_dir(versions_dir)? {
                    let entry = entry?;
                    versions.push(entry.file_name().to_string_lossy().into_owned());
                }
            }
            
            versions.extend(detect_system_java());
            
            if versions.is_empty() {
                println!("No Java versions found");
            } else {
                println!("Available Java versions:");
                for version in versions {
                    println!("- {}", version);
                }
            }
        }
        Commands::Current => {
            if let Ok(java_home) = env::var("JAVA_HOME") {
                let version = Path::new(&java_home)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown");
                println!("Current Java version: {}", version);
            } else {
                println!("No Java version set");
            }
        }
        Commands::Install { version } => {
            install_java(&version)?;
            println!("Successfully installed Java {}", version);
        }
    }
    
    Ok(())
}
