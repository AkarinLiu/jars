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
        /// JDK provider (temurin, corretto, zulu, etc.)
        #[arg(short, long, default_value = "temurin")]
        provider: String,
    },
    /// Remove an installed Java version
    Delete {
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

fn get_download_url(version: &str, provider: &str) -> Result<String, JarsError> {
    let os = if cfg!(windows) { "windows" } else if cfg!(target_os = "macos") { "mac" } else { "linux" };
    let arch = if cfg!(target_arch = "x86_64") { "x64" } else { "aarch64" };
    
    match provider.to_lowercase().as_str() {
        "temurin" => Ok(format!(
            "https://api.adoptium.net/v3/binary/latest/{}/ga/{}/{}/jdk/hotspot/normal/eclipse?project=jdk",
            version, os, arch
        )),
        "corretto" => Ok(format!(
            "https://corretto.aws/downloads/latest/amazon-corretto-{}-{}-{}.tar.gz",
            version, arch, os
        )),
        "zulu" => Ok(format!(
            "https://api.azul.com/zulu/download/community/v1.0/bundles/latest/?jdk_version={}&os={}&arch={}&ext=zip",
            version, os, arch
        )),
        _ => Err(JarsError::DownloadError(format!("Unsupported JDK provider: {}", provider)))
    }
}

fn install_java(version: &str, provider: &str) -> Result<(), JarsError> {
    let versions_dir = get_versions_dir();
    let full_version = format!("{}-{}", provider, version);
    let target_dir = versions_dir.join(&full_version);
    
    if !target_dir.exists() {
        fs::create_dir_all(&target_dir)?;
    }
    
    let pb = ProgressBar::new(100);
    pb.set_message(format!("Downloading Java {} from {}", version, provider));
    
    // Get download URL
    let url = get_download_url(version, provider)?;
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
    
    pb.finish_with_message(format!("Java {} from {} installed successfully", version, provider));
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Use { version } => {
            // Check if version already includes provider prefix
            // Try exact match first
            let mut java_path = get_versions_dir().join(&version).join("bin").join("java");
            
            // If not found, try with temurin prefix
            if !java_path.exists() && !version.contains('-') {
                let temurin_version = format!("temurin-{}", version);
                java_path = get_versions_dir().join(&temurin_version).join("bin").join("java");
                if java_path.exists() {
                    env::set_var("JAVA_HOME", get_versions_dir().join(&temurin_version));
                    let path_sep = if cfg!(windows) { ";" } else { ":" };
                    env::set_var(
                        "PATH", 
                        format!("{}{}{}", java_path.parent().unwrap().display(), path_sep, env::var("PATH")?)
                    );
                    println!("Now using Java version {}", version);
                    return Ok(());
                }
            }
            
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
                    if version.contains('-') {
                        let parts: Vec<&str> = version.split('-').collect();
                        println!("- {} (Provider: {})", parts[1], parts[0]);
                    } else {
                        println!("- {} (System installed)", version);
                    }
                }
            }
        }
        Commands::Current => {
            if let Ok(java_home) = env::var("JAVA_HOME") {
                let full_version = Path::new(&java_home)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown");
                
                if full_version.contains('-') {
                    let parts: Vec<&str> = full_version.split('-').collect();
                    println!("Current Java version: {} (Provider: {})", parts[1], parts[0]);
                } else {
                    println!("Current Java version: {} (System installed)", full_version);
                }
            } else {
                println!("No Java version set");
            }
        }
        Commands::Install { version, provider } => {
            install_java(&version, &provider)?;
            println!("Successfully installed Java {} from {}", version, provider);
        }
        Commands::Delete { version } => {
            // Try exact match first
            let mut target_dir = get_versions_dir().join(&version);
            
            // If not found, try with temurin prefix
            if !target_dir.exists() && !version.contains('-') {
                let temurin_version = format!("temurin-{}", version);
                target_dir = get_versions_dir().join(&temurin_version);
                if !target_dir.exists() {
                    return Err(Box::new(JarsError::VersionNotFound(version)));
                }
                fs::remove_dir_all(&target_dir)?;
                println!("Successfully removed Java version {}", temurin_version);
                return Ok(());
            }
            
            if !target_dir.exists() {
                return Err(Box::new(JarsError::VersionNotFound(version)));
            }
            
            fs::remove_dir_all(&target_dir)?;
            println!("Successfully removed Java version {}", version);
        }
    }
    
    Ok(())
}
