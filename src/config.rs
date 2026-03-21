use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Persisted application configuration.
/// Stored at %APPDATA%\window-selector\config.toml
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Activation hotkey modifier flags (MOD_CONTROL | MOD_ALT etc.)
    pub hotkey_modifiers: u32,
    /// Activation hotkey virtual key code (e.g., VK_SPACE = 0x20)
    pub hotkey_vk: u32,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            // MOD_CONTROL (0x0002) | MOD_ALT (0x0001) | MOD_NOREPEAT (0x4000)
            hotkey_modifiers: 0x0002 | 0x0001 | 0x4000,
            // VK_SPACE = 0x20
            hotkey_vk: 0x20,
        }
    }
}

impl AppConfig {
    /// Load config from the given directory. Creates default config if file does not exist.
    /// Falls back to defaults if the file is corrupt.
    pub fn load(config_dir: &Path) -> std::io::Result<Self> {
        let config_path = config_dir.join("config.toml");

        if !config_path.exists() {
            let default = AppConfig::default();
            Self::save_to_path(&default, &config_path)?;
            return Ok(default);
        }

        let contents = fs::read_to_string(&config_path)?;
        match toml::from_str::<AppConfig>(&contents) {
            Ok(config) => Ok(config),
            Err(e) => {
                tracing::warn!(
                    "Config file at {:?} is corrupt: {}. Using defaults.",
                    config_path,
                    e
                );
                Ok(AppConfig::default())
            }
        }
    }

    /// Save config to the given directory atomically (write to temp then rename).
    #[allow(dead_code)]
    pub fn save(config_dir: &Path, config: &AppConfig) -> std::io::Result<()> {
        let config_path = config_dir.join("config.toml");
        Self::save_to_path(config, &config_path)
    }

    fn save_to_path(config: &AppConfig, config_path: &Path) -> std::io::Result<()> {
        // Ensure directory exists
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let toml_str = toml::to_string(config)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

        // Atomic write: write to temp file then rename
        let tmp_path = config_path.with_extension("toml.tmp");
        fs::write(&tmp_path, &toml_str)?;
        fs::rename(&tmp_path, config_path)?;

        Ok(())
    }

    /// Returns the default config directory path: %APPDATA%\window-selector
    pub fn default_config_dir() -> Option<PathBuf> {
        let appdata = std::env::var("APPDATA").ok()?;
        Some(PathBuf::from(appdata).join("window-selector"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "window-selector-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .subsec_nanos()
        ));
        dir
    }

    #[test]
    fn test_default_config_load_on_missing_file() {
        let dir = temp_dir();
        let config = AppConfig::load(&dir).expect("load should succeed");
        assert_eq!(config.hotkey_modifiers, 0x0002 | 0x0001 | 0x4000);
        assert_eq!(config.hotkey_vk, 0x20); // VK_SPACE
        // Cleanup
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_config_round_trip_save_and_load() {
        let dir = temp_dir();
        let original = AppConfig {
            hotkey_modifiers: 0x0004, // MOD_SHIFT
            hotkey_vk: 0x70,          // VK_F1
        };
        AppConfig::save(&dir, &original).expect("save should succeed");
        let loaded = AppConfig::load(&dir).expect("load should succeed");
        assert_eq!(loaded.hotkey_modifiers, original.hotkey_modifiers);
        assert_eq!(loaded.hotkey_vk, original.hotkey_vk);
        // Cleanup
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_corrupt_config_falls_back_to_defaults() {
        let dir = temp_dir();
        fs::create_dir_all(&dir).expect("create dir");
        let config_path = dir.join("config.toml");
        fs::write(&config_path, b"not valid toml {{{{").expect("write corrupt");
        let config = AppConfig::load(&dir).expect("should return defaults, not error");
        assert_eq!(config.hotkey_vk, 0x20); // Default VK_SPACE
        // Cleanup
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_config_directory_created_if_absent() {
        let dir = temp_dir().join("nested").join("dir");
        let config = AppConfig::default();
        AppConfig::save(&dir, &config).expect("save should create dirs");
        assert!(dir.join("config.toml").exists());
        // Cleanup
        let _ = fs::remove_dir_all(dir.parent().unwrap().parent().unwrap());
    }

    #[test]
    fn test_atomic_write_uses_temp_file() {
        // This verifies the save implementation writes to a .tmp then renames.
        // We check that after save, only config.toml exists (not .tmp).
        let dir = temp_dir();
        let config = AppConfig::default();
        AppConfig::save(&dir, &config).expect("save should succeed");
        assert!(dir.join("config.toml").exists());
        assert!(!dir.join("config.toml.tmp").exists());
        // Cleanup
        let _ = fs::remove_dir_all(&dir);
    }
}