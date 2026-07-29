use crate::keycodes::{
    is_digit, is_letter, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, MOD_WIN, VK_CONTROL, VK_ESCAPE,
    VK_LWIN, VK_MENU, VK_Q, VK_RETURN, VK_SHIFT, VK_Y,
};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuickTagEntry {
    pub number: u8,
    pub exe_path: String,
}

fn default_hotkey_modifiers() -> u32 {
    MOD_CONTROL | MOD_ALT | MOD_NOREPEAT
}
fn default_hotkey_vk() -> u32 {
    VK_Q
}
fn default_label_hotkey_modifiers() -> u32 {
    MOD_WIN | MOD_NOREPEAT
}
fn default_label_hotkey_vk() -> u32 {
    VK_Y
}
fn default_overlay_opacity() -> u8 {
    220
}
fn default_fade_duration_ms() -> u32 {
    150
}
fn default_grid_padding() -> f32 {
    16.0
}
fn default_label_font_size() -> f32 {
    18.0
}
fn default_title_font_size() -> f32 {
    13.0
}
fn default_background_opacity() -> f32 {
    0.86
}
fn default_label_overlap_strategy() -> LabelOverlapStrategy {
    LabelOverlapStrategy::AutoNudge
}
fn default_confirm_modifiers() -> u32 {
    0
}
fn default_confirm_vk() -> u32 {
    VK_RETURN
}
fn default_dismiss_modifiers() -> u32 {
    0
}
fn default_dismiss_vk() -> u32 {
    VK_ESCAPE
}
fn default_tag_modifier() -> ActionModifier {
    ActionModifier::Ctrl
}
fn default_move_modifier() -> ActionModifier {
    ActionModifier::Shift
}

/// A modifier key usable for the overlay's tag-assign and move-to-monitor actions.
/// Distinct from the `MOD_*` flags used by `RegisterHotKey`: this is a single
/// modifier choice compared against physical key state via `GetAsyncKeyState`,
/// not a global-hotkey registration.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActionModifier {
    Ctrl,
    Alt,
    Shift,
    Win,
}

impl ActionModifier {
    /// All variants, in the fixed order shown by the Keybindings page's option
    /// buttons. Single source of truth for both rendering order and the
    /// `tag_modifier_buttons`/`move_modifier_buttons` rect indices.
    pub const ALL: [ActionModifier; 4] = [
        ActionModifier::Ctrl,
        ActionModifier::Alt,
        ActionModifier::Shift,
        ActionModifier::Win,
    ];

    /// The generic (either-side) `GetAsyncKeyState` virtual-key code for this modifier.
    pub fn to_vk(self) -> u32 {
        match self {
            ActionModifier::Ctrl => VK_CONTROL,
            ActionModifier::Alt => VK_MENU,
            ActionModifier::Shift => VK_SHIFT,
            ActionModifier::Win => VK_LWIN,
        }
    }

    /// Human-readable name for display in the Keybindings and Guide pages.
    pub fn display_name(self) -> &'static str {
        match self {
            ActionModifier::Ctrl => "Ctrl",
            ActionModifier::Alt => "Alt",
            ActionModifier::Shift => "Shift",
            ActionModifier::Win => "Win",
        }
    }
}

/// How to handle overlapping labels in label mode.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum LabelOverlapStrategy {
    /// Automatically nudge overlapping labels apart (default).
    AutoNudge,
    /// Place labels only inside the window's visible (non-occluded) region.
    VisibleRegion,
}

/// Persisted application configuration.
/// Stored at %APPDATA%\window-selector\config.toml
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Activation hotkey modifier flags (MOD_CONTROL | MOD_ALT etc.)
    #[serde(default = "default_hotkey_modifiers")]
    pub hotkey_modifiers: u32,
    /// Activation hotkey virtual key code (e.g., VK_Q = 0x51)
    #[serde(default = "default_hotkey_vk")]
    pub hotkey_vk: u32,
    /// When true, pressing a letter key immediately switches to that window.
    /// When false (default), letter selects and Enter/Space confirms.
    #[serde(default)]
    pub direct_switch: bool,
    /// Label mode hotkey modifier flags (MOD_WIN etc.)
    #[serde(default = "default_label_hotkey_modifiers")]
    pub label_hotkey_modifiers: u32,
    /// Label mode hotkey virtual key code (e.g., VK_Y = 0x59)
    #[serde(default = "default_label_hotkey_vk")]
    pub label_hotkey_vk: u32,

    // --- New fields (all with serde defaults for backward compatibility) ---
    /// Launch Window Selector automatically at Windows startup.
    #[serde(default)]
    pub launch_at_startup: bool,

    /// Maximum alpha for overlay window (window-level opacity). Range: 50–255. Default: 220.
    #[serde(default = "default_overlay_opacity")]
    pub overlay_opacity: u8,

    /// Duration in milliseconds of the fade animation. Range: 0–500. Default: 150.
    /// Set to 0 for instant show/hide with no animation.
    #[serde(default = "default_fade_duration_ms")]
    pub fade_duration_ms: u32,

    /// Padding between grid cells in logical pixels. Range: 4.0–48.0. Default: 16.0.
    #[serde(default = "default_grid_padding")]
    pub grid_padding: f32,

    /// Font size for letter labels in the overlay. Range: 10.0–32.0. Default: 18.0.
    #[serde(default = "default_label_font_size")]
    pub label_font_size: f32,

    /// Font size for window title text in the overlay. Range: 8.0–24.0. Default: 13.0.
    #[serde(default = "default_title_font_size")]
    pub title_font_size: f32,

    /// Alpha channel of the backdrop brush (dark rectangle behind grid). Range: 0.0–1.0. Default: 0.86.
    #[serde(default = "default_background_opacity")]
    pub background_opacity: f32,
    /// Persistent quick-list tags. Each entry maps number key (1-9) to executable path.
    #[serde(default)]
    pub quick_tags: Vec<QuickTagEntry>,

    /// How to handle overlapping labels in label mode.
    /// Default: AutoNudge.
    #[serde(default = "default_label_overlap_strategy")]
    pub label_overlap_strategy: LabelOverlapStrategy,

    // --- Configurable in-overlay keybindings ---
    /// Modifier flags (MOD_CONTROL etc., 0 = none) for the custom Confirm key.
    /// Space always confirms regardless of this value.
    #[serde(default = "default_confirm_modifiers")]
    pub confirm_modifiers: u32,
    /// Virtual key code for the custom Confirm key. Default: VK_RETURN.
    #[serde(default = "default_confirm_vk")]
    pub confirm_vk: u32,
    /// Modifier flags (MOD_CONTROL etc., 0 = none) for the custom Dismiss key.
    /// Escape always dismisses regardless of this value.
    #[serde(default = "default_dismiss_modifiers")]
    pub dismiss_modifiers: u32,
    /// Virtual key code for the custom Dismiss key. Default: VK_ESCAPE.
    #[serde(default = "default_dismiss_vk")]
    pub dismiss_vk: u32,
    /// Modifier held with 1-9 to assign a session tag to the selected window.
    #[serde(default = "default_tag_modifier")]
    pub tag_modifier: ActionModifier,
    /// Modifier held with 1-9 to move the selected window to a monitor's center.
    #[serde(default = "default_move_modifier")]
    pub move_modifier: ActionModifier,
    /// Whether the Guide page has already been shown once, automatically, on
    /// first launch. Set to true right after that first auto-open.
    #[serde(default)]
    pub guide_shown: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            hotkey_modifiers: default_hotkey_modifiers(),
            hotkey_vk: default_hotkey_vk(),
            direct_switch: false,
            label_hotkey_modifiers: default_label_hotkey_modifiers(),
            label_hotkey_vk: default_label_hotkey_vk(),
            launch_at_startup: false,
            overlay_opacity: default_overlay_opacity(),
            fade_duration_ms: default_fade_duration_ms(),
            grid_padding: default_grid_padding(),
            label_font_size: default_label_font_size(),
            title_font_size: default_title_font_size(),
            background_opacity: default_background_opacity(),
            quick_tags: Vec::new(),
            label_overlap_strategy: default_label_overlap_strategy(),
            confirm_modifiers: default_confirm_modifiers(),
            confirm_vk: default_confirm_vk(),
            dismiss_modifiers: default_dismiss_modifiers(),
            dismiss_vk: default_dismiss_vk(),
            tag_modifier: default_tag_modifier(),
            move_modifier: default_move_modifier(),
            guide_shown: false,
        }
    }
}

impl AppConfig {
    /// Clamp all numeric fields to valid ranges.
    /// Call after deserialization and after slider commit.
    pub fn validate(&mut self) {
        self.overlay_opacity = self.overlay_opacity.max(50); // min 50
                                                             // max is 255 by type (u8)
        self.fade_duration_ms = self.fade_duration_ms.min(500);
        self.grid_padding = self.grid_padding.clamp(4.0, 48.0);
        self.label_font_size = self.label_font_size.clamp(10.0, 32.0);
        self.title_font_size = self.title_font_size.clamp(8.0, 24.0);
        self.background_opacity = self.background_opacity.clamp(0.0, 1.0);
        self.quick_tags
            .retain(|entry| (1..=9).contains(&entry.number) && !entry.exe_path.is_empty());
        self.quick_tags.sort_by_key(|entry| entry.number);
        self.quick_tags.dedup_by_key(|entry| entry.number);

        // The tag-assign and move-to-monitor modifiers must stay distinct —
        // otherwise 1-9 could never disambiguate the two actions. Repair by
        // resetting both to their defaults.
        if self.tag_modifier == self.move_modifier {
            self.tag_modifier = default_tag_modifier();
            self.move_modifier = default_move_modifier();
        }

        // A bare (unmodified) letter for Confirm/Dismiss would collide with
        // window-selection (a-z) keys, so repair it back to the default combo.
        // A digit is repaired UNCONDITIONALLY, modifiers or not: 1-9 is fully
        // claimed by tag-assign/tag-jump/move-to-monitor, so a digit
        // Confirm/Dismiss binding (e.g. a hand-edited Ctrl+3) can never fire —
        // `interaction.rs`'s digit branch returns before the Confirm/Dismiss
        // check ever runs. This mirrors the recorder's own rule in
        // `settings_panel.rs::ll_keyboard_proc`; keep the two in sync.
        if is_digit(self.confirm_vk)
            || (self.confirm_modifiers == 0 && is_letter(self.confirm_vk))
        {
            self.confirm_modifiers = default_confirm_modifiers();
            self.confirm_vk = default_confirm_vk();
        }
        if is_digit(self.dismiss_vk)
            || (self.dismiss_modifiers == 0 && is_letter(self.dismiss_vk))
        {
            self.dismiss_modifiers = default_dismiss_modifiers();
            self.dismiss_vk = default_dismiss_vk();
        }
    }

    pub fn set_quick_tag(&mut self, number: u8, exe_path: String) {
        self.quick_tags.retain(|entry| entry.number != number);
        self.quick_tags.push(QuickTagEntry { number, exe_path });
        self.quick_tags.sort_by_key(|entry| entry.number);
    }

    pub fn remove_quick_tag(&mut self, number: u8) {
        self.quick_tags.retain(|entry| entry.number != number);
    }

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
            Ok(mut config) => {
                config.validate();
                Ok(config)
            }
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
    use crate::keycodes::{
        MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, MOD_WIN, VK_F1, VK_Q, VK_Y,
    };
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
        assert_eq!(
            config.hotkey_modifiers,
            MOD_CONTROL | MOD_ALT | MOD_NOREPEAT
        );
        assert_eq!(config.hotkey_vk, VK_Q);
        assert_eq!(config.label_hotkey_modifiers, MOD_WIN | MOD_NOREPEAT);
        assert_eq!(config.label_hotkey_vk, VK_Y);
        // Cleanup
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_config_round_trip_save_and_load() {
        let dir = temp_dir();
        let original = AppConfig {
            hotkey_modifiers: MOD_SHIFT,
            hotkey_vk: VK_F1,
            direct_switch: true,
            label_hotkey_modifiers: MOD_WIN | MOD_NOREPEAT,
            label_hotkey_vk: VK_Y,
            launch_at_startup: false,
            overlay_opacity: 200,
            fade_duration_ms: 100,
            grid_padding: 20.0,
            label_font_size: 22.0,
            title_font_size: 15.0,
            background_opacity: 0.9,
            quick_tags: vec![QuickTagEntry {
                number: 1,
                exe_path: String::from(r"C:\\Windows\\System32\\notepad.exe"),
            }],
            label_overlap_strategy: LabelOverlapStrategy::VisibleRegion,
            confirm_modifiers: MOD_CONTROL,
            confirm_vk: VK_F1,
            dismiss_modifiers: MOD_ALT,
            dismiss_vk: VK_F1,
            tag_modifier: ActionModifier::Alt,
            move_modifier: ActionModifier::Win,
            guide_shown: true,
        };
        AppConfig::save(&dir, &original).expect("save should succeed");
        let loaded = AppConfig::load(&dir).expect("load should succeed");
        assert_eq!(loaded.hotkey_modifiers, original.hotkey_modifiers);
        assert_eq!(loaded.hotkey_vk, original.hotkey_vk);
        assert_eq!(loaded.direct_switch, original.direct_switch);
        assert_eq!(loaded.overlay_opacity, original.overlay_opacity);
        assert_eq!(loaded.fade_duration_ms, original.fade_duration_ms);
        assert!((loaded.grid_padding - original.grid_padding).abs() < 0.001);
        assert!((loaded.label_font_size - original.label_font_size).abs() < 0.001);
        assert!((loaded.title_font_size - original.title_font_size).abs() < 0.001);
        assert!((loaded.background_opacity - original.background_opacity).abs() < 0.001);
        assert_eq!(loaded.quick_tags, original.quick_tags);
        assert_eq!(loaded.label_overlap_strategy, original.label_overlap_strategy);
        assert_eq!(loaded.confirm_modifiers, original.confirm_modifiers);
        assert_eq!(loaded.confirm_vk, original.confirm_vk);
        assert_eq!(loaded.dismiss_modifiers, original.dismiss_modifiers);
        assert_eq!(loaded.dismiss_vk, original.dismiss_vk);
        assert_eq!(loaded.tag_modifier, original.tag_modifier);
        assert_eq!(loaded.move_modifier, original.move_modifier);
        assert_eq!(loaded.guide_shown, original.guide_shown);
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
        assert_eq!(config.hotkey_vk, VK_Q); // default
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

    #[test]
    fn test_default_values_match_spec() {
        let config = AppConfig::default();
        assert_eq!(config.overlay_opacity, 220);
        assert_eq!(config.fade_duration_ms, 150);
        assert!((config.grid_padding - 16.0).abs() < 0.001);
        assert!((config.label_font_size - 18.0).abs() < 0.001);
        assert!((config.title_font_size - 13.0).abs() < 0.001);
        assert!((config.background_opacity - 0.86).abs() < 0.001);
        assert!(!config.launch_at_startup);
        assert!(!config.direct_switch);
        assert!(config.quick_tags.is_empty());
        assert_eq!(config.label_overlap_strategy, LabelOverlapStrategy::AutoNudge);
        assert_eq!(config.confirm_modifiers, 0);
        assert_eq!(config.confirm_vk, VK_RETURN);
        assert_eq!(config.dismiss_modifiers, 0);
        assert_eq!(config.dismiss_vk, VK_ESCAPE);
        assert_eq!(config.tag_modifier, ActionModifier::Ctrl);
        assert_eq!(config.move_modifier, ActionModifier::Shift);
        assert!(!config.guide_shown);
    }

    #[test]
    fn test_validate_clamps_out_of_range_values() {
        let mut config = AppConfig {
            overlay_opacity: 10,     // below min 50
            fade_duration_ms: 9999,  // above max 500
            grid_padding: 0.5,       // below min 4.0
            label_font_size: 100.0,  // above max 32.0
            title_font_size: 1.0,    // below min 8.0
            background_opacity: 5.0, // above max 1.0
            ..Default::default()
        };
        config.validate();
        assert_eq!(config.overlay_opacity, 50);
        assert_eq!(config.fade_duration_ms, 500);
        assert!((config.grid_padding - 4.0).abs() < 0.001);
        assert!((config.label_font_size - 32.0).abs() < 0.001);
        assert!((config.title_font_size - 8.0).abs() < 0.001);
        assert!((config.background_opacity - 1.0).abs() < 0.001);
    }

    // TC-4.11: AppConfig::validate() on a default config does not change any values.
    // Ensures that all default values are already within their valid ranges.
    #[test]
    fn test_validate_does_not_change_default_values() {
        let original = AppConfig::default();
        let mut validated = AppConfig::default();
        validated.validate();

        assert_eq!(
            validated.overlay_opacity, original.overlay_opacity,
            "validate() must not change default overlay_opacity"
        );
        assert_eq!(
            validated.fade_duration_ms, original.fade_duration_ms,
            "validate() must not change default fade_duration_ms"
        );
        assert!(
            (validated.grid_padding - original.grid_padding).abs() < 0.001,
            "validate() must not change default grid_padding"
        );
        assert!(
            (validated.label_font_size - original.label_font_size).abs() < 0.001,
            "validate() must not change default label_font_size"
        );
        assert!(
            (validated.title_font_size - original.title_font_size).abs() < 0.001,
            "validate() must not change default title_font_size"
        );
        assert!(
            (validated.background_opacity - original.background_opacity).abs() < 0.001,
            "validate() must not change default background_opacity"
        );
        assert_eq!(
            validated.hotkey_modifiers, original.hotkey_modifiers,
            "validate() must not change default hotkey_modifiers"
        );
        assert_eq!(
            validated.hotkey_vk, original.hotkey_vk,
            "validate() must not change default hotkey_vk"
        );
        assert_eq!(
            validated.direct_switch, original.direct_switch,
            "validate() must not change default direct_switch"
        );
        assert_eq!(
            validated.launch_at_startup, original.launch_at_startup,
            "validate() must not change default launch_at_startup"
        );
        assert_eq!(
            validated.quick_tags, original.quick_tags,
            "validate() must not change default quick_tags"
        );
        assert_eq!(validated.overlay_opacity, original.overlay_opacity,
            "validate() must not change default overlay_opacity");
        assert_eq!(validated.fade_duration_ms, original.fade_duration_ms,
            "validate() must not change default fade_duration_ms");
        assert!((validated.grid_padding - original.grid_padding).abs() < 0.001,
            "validate() must not change default grid_padding");
        assert!((validated.label_font_size - original.label_font_size).abs() < 0.001,
            "validate() must not change default label_font_size");
        assert!((validated.title_font_size - original.title_font_size).abs() < 0.001,
            "validate() must not change default title_font_size");
        assert!((validated.background_opacity - original.background_opacity).abs() < 0.001,
            "validate() must not change default background_opacity");
        assert_eq!(validated.hotkey_modifiers, original.hotkey_modifiers,
            "validate() must not change default hotkey_modifiers");
        assert_eq!(validated.hotkey_vk, original.hotkey_vk,
            "validate() must not change default hotkey_vk");
        assert_eq!(validated.direct_switch, original.direct_switch,
            "validate() must not change default direct_switch");
        assert_eq!(validated.launch_at_startup, original.launch_at_startup,
            "validate() must not change default launch_at_startup");
        assert_eq!(validated.label_overlap_strategy, original.label_overlap_strategy,
            "validate() must not change default label_overlap_strategy");
        assert_eq!(validated.confirm_modifiers, original.confirm_modifiers,
            "validate() must not change default confirm_modifiers");
        assert_eq!(validated.confirm_vk, original.confirm_vk,
            "validate() must not change default confirm_vk");
        assert_eq!(validated.dismiss_modifiers, original.dismiss_modifiers,
            "validate() must not change default dismiss_modifiers");
        assert_eq!(validated.dismiss_vk, original.dismiss_vk,
            "validate() must not change default dismiss_vk");
        assert_eq!(validated.tag_modifier, original.tag_modifier,
            "validate() must not change default tag_modifier");
        assert_eq!(validated.move_modifier, original.move_modifier,
            "validate() must not change default move_modifier");
    }

    // Plan step 4: validate() repairs an equal tag/move modifier pair back to defaults.
    #[test]
    fn test_validate_repairs_equal_action_modifiers() {
        let mut config = AppConfig {
            tag_modifier: ActionModifier::Shift,
            move_modifier: ActionModifier::Shift,
            ..Default::default()
        };
        config.validate();
        assert_eq!(config.tag_modifier, ActionModifier::Ctrl);
        assert_eq!(config.move_modifier, ActionModifier::Shift);
    }

    // Plan step 4: validate() resets a bare-letter/digit Confirm or Dismiss key.
    #[test]
    fn test_validate_repairs_bare_alphanumeric_confirm_and_dismiss() {
        let mut config = AppConfig {
            confirm_modifiers: 0,
            confirm_vk: crate::keycodes::VK_J,
            dismiss_modifiers: 0,
            dismiss_vk: crate::keycodes::VK_5,
            ..Default::default()
        };
        config.validate();
        assert_eq!(config.confirm_modifiers, 0);
        assert_eq!(config.confirm_vk, VK_RETURN);
        assert_eq!(config.dismiss_modifiers, 0);
        assert_eq!(config.dismiss_vk, VK_ESCAPE);
    }

    // A modifier-qualified letter, and a non-alphanumeric key with or without
    // a modifier, are left untouched for Confirm/Dismiss.
    #[test]
    fn test_validate_keeps_modifier_qualified_confirm_and_dismiss() {
        let mut config = AppConfig {
            confirm_modifiers: MOD_CONTROL,
            confirm_vk: crate::keycodes::VK_J,
            dismiss_modifiers: MOD_ALT,
            dismiss_vk: crate::keycodes::VK_F4,
            ..Default::default()
        };
        config.validate();
        assert_eq!(config.confirm_modifiers, MOD_CONTROL);
        assert_eq!(config.confirm_vk, crate::keycodes::VK_J);
        assert_eq!(config.dismiss_modifiers, MOD_ALT);
        assert_eq!(config.dismiss_vk, crate::keycodes::VK_F4);
    }

    // Hermes BLOCKER: a digit Confirm/Dismiss key must be repaired even when
    // modifier-qualified (e.g. a hand-edited Ctrl+3 in config.toml) — digits
    // are fully claimed by tag-assign/tag-jump/move-to-monitor, so such a
    // binding can never actually fire at runtime (interaction.rs's digit
    // branch returns before the Confirm/Dismiss check runs).
    #[test]
    fn test_validate_repairs_modifier_qualified_digit_confirm_and_dismiss() {
        let mut config = AppConfig {
            confirm_modifiers: MOD_CONTROL,
            confirm_vk: crate::keycodes::VK_3,
            dismiss_modifiers: MOD_ALT,
            dismiss_vk: crate::keycodes::VK_7,
            ..Default::default()
        };
        config.validate();
        assert_eq!(config.confirm_modifiers, 0);
        assert_eq!(config.confirm_vk, VK_RETURN);
        assert_eq!(config.dismiss_modifiers, 0);
        assert_eq!(config.dismiss_vk, VK_ESCAPE);
    }

    #[test]
    fn test_old_config_missing_new_fields_gets_defaults() {
        // Simulate loading an old config file with only the original fields
        let dir = temp_dir();
        fs::create_dir_all(&dir).expect("create dir");
        let config_path = dir.join("config.toml");
        // Old format with only original fields
        fs::write(&config_path, b"hotkey_modifiers = 16387\nhotkey_vk = 81\n").expect("write");
        let config = AppConfig::load(&dir).expect("load should succeed");
        assert_eq!(config.hotkey_modifiers, 16387);
        assert_eq!(config.hotkey_vk, 81);
        assert_eq!(config.overlay_opacity, 220); // default
        assert_eq!(config.fade_duration_ms, 150); // default
        assert!((config.background_opacity - 0.86).abs() < 0.001); // default
        assert!(config.quick_tags.is_empty());
        // Plan step 4 / spec "Config file predating the feature": confirm is
        // Enter, dismiss is Escape, tag modifier is Ctrl, move modifier is Shift.
        assert_eq!(config.confirm_modifiers, 0);
        assert_eq!(config.confirm_vk, VK_RETURN);
        assert_eq!(config.dismiss_modifiers, 0);
        assert_eq!(config.dismiss_vk, VK_ESCAPE);
        assert_eq!(config.tag_modifier, ActionModifier::Ctrl);
        assert_eq!(config.move_modifier, ActionModifier::Shift);
        assert!(!config.guide_shown);
        // Cleanup
        let _ = fs::remove_dir_all(&dir);
    }
}
