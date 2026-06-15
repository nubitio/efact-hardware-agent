use std::path::PathBuf;

pub const APP_NAME: &str = "efact-hardware-agent";
pub const LEGACY_APP_NAME: &str = "efact-printer-agent";
pub const APP_DISPLAY_NAME: &str = "eFact Hardware Agent";

pub fn config_dir() -> PathBuf {
    resolve_config_dir(APP_NAME)
        .or_else(|| resolve_config_dir(LEGACY_APP_NAME))
        .unwrap_or_else(fallback_config_dir)
}

pub fn log_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        std::env::var("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|_| std::env::temp_dir())
            .join(APP_NAME)
    }
    #[cfg(target_os = "macos")]
    {
        std::env::var("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| std::env::temp_dir())
            .join("Library")
            .join("Logs")
            .join(APP_NAME)
    }
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    {
        std::env::var("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| std::env::temp_dir())
            .join(".local")
            .join("share")
            .join(APP_NAME)
    }
}

pub fn config_file_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    if let Some(exe_dir) = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("config.toml")))
    {
        candidates.push(exe_dir);
    }

    for app_name in [APP_NAME, LEGACY_APP_NAME] {
        if let Some(path) = resolve_config_file(app_name) {
            candidates.push(path);
        }
    }

    candidates
}

fn resolve_config_dir(app_name: &str) -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        std::env::var("APPDATA")
            .ok()
            .map(|d| PathBuf::from(d).join(app_name))
    }
    #[cfg(not(target_os = "windows"))]
    {
        std::env::var("HOME")
            .ok()
            .map(|h| PathBuf::from(h).join(".config").join(app_name))
    }
}

fn resolve_config_file(app_name: &str) -> Option<PathBuf> {
    resolve_config_dir(app_name).map(|dir| dir.join("config.toml"))
}

fn fallback_config_dir() -> PathBuf {
    std::env::temp_dir().join(APP_NAME)
}

pub fn default_config_path() -> PathBuf {
    resolve_config_dir(APP_NAME)
        .or_else(|| resolve_config_dir(LEGACY_APP_NAME))
        .map(|dir| dir.join("config.toml"))
        .unwrap_or_else(|| fallback_config_dir().join("config.toml"))
}
