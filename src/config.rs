use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;

// Nerd Font priority list
const NERD_FONT_CANDIDATES: &[&str] = &[
    "SauceCodePro Nerd Font",
    "SauceCodePro Nerd Font Mono",
    "Monokoi Nerd Font",
    "Monokoi Nerd Font Mono",
    "JetBrains Mono Nerd Font",
    "JetBrains Mono NF",
    "JetBrainsMono Nerd Font",
    "FiraCode Nerd Font",
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ScrollbarVisibility {
    Auto,
    Always,
}

impl Default for ScrollbarVisibility {
    fn default() -> Self {
        Self::Always
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_font_size")]
    pub font_size: f32,

    #[serde(default = "default_font_family")]
    pub font_family: String,

    #[serde(default = "default_padding")]
    pub padding: f32,

    #[serde(default)]
    pub scrollbar_visibility: ScrollbarVisibility,

    #[serde(default = "default_scrollback_lines")]
    pub scrollback_lines: usize,

    #[serde(default = "default_initial_width")]
    pub initial_width: f32,

    #[serde(default = "default_initial_height")]
    pub initial_height: f32,

    #[serde(default = "default_cols")]
    pub cols: usize,

    #[serde(default = "default_rows")]
    pub rows: usize,

    #[serde(default = "default_theme")]
    pub theme: String,

    #[serde(default)]
    pub restore_session: bool,

    #[serde(default)]
    pub session_history_file: Option<PathBuf>,
}

fn default_font_size() -> f32 {
    14.0
}

fn detect_available_fonts() -> Vec<String> {
    // Try to get installed fonts using fc-list
    if let Ok(output) = Command::new("fc-list")
        .args(&[":", "family,style"])
        .output()
    {
        if let Ok(stdout) = String::from_utf8(output.stdout) {
            return stdout
                .lines()
                .filter_map(|line| {
                    line.split(',').next().map(|s| s.trim().to_string())
                })
                .collect();
        }
    }
    Vec::new()
}

fn default_font_family() -> String {
    let available_fonts = detect_available_fonts();

    // Try each candidate font in priority order
    for candidate in NERD_FONT_CANDIDATES {
        if available_fonts.iter().any(|f| f.eq_ignore_ascii_case(candidate)) {
            eprintln!("[Config] Using font: {}", candidate);
            return candidate.to_string();
        }
    }

    // Fallback to first candidate if none found (system may still have it)
    eprintln!("[Config] No Nerd Font detected, using default: {}", NERD_FONT_CANDIDATES[0]);
    NERD_FONT_CANDIDATES[0].to_string()
}

fn default_padding() -> f32 {
    2.0
}

fn default_scrollback_lines() -> usize {
    10000
}

fn default_initial_width() -> f32 {
    1200.0
}

fn default_initial_height() -> f32 {
    600.0
}

fn default_cols() -> usize {
    100
}

fn default_rows() -> usize {
    30
}

fn default_theme() -> String {
    "dark".to_string()
}

impl Default for Config {
    fn default() -> Self {
        Config {
            font_size: default_font_size(),
            font_family: default_font_family(),
            padding: default_padding(),
            scrollbar_visibility: ScrollbarVisibility::default(),
            scrollback_lines: default_scrollback_lines(),
            initial_width: default_initial_width(),
            initial_height: default_initial_height(),
            cols: default_cols(),
            rows: default_rows(),
            theme: default_theme(),
            restore_session: false,
            session_history_file: None,
        }
    }
}

impl Config {
    pub fn load() -> Self {
        if let Ok(config_path) = Self::config_path() {
            if config_path.exists() {
                if let Ok(content) = std::fs::read_to_string(&config_path) {
                    if let Ok(config) = toml::from_str::<Config>(&content) {
                        eprintln!("[Config] Loaded from {}", config_path.display());
                        eprintln!("[Config] Font: {}", config.font_family);
                        return config;
                    } else {
                        eprintln!("[Config] Failed to parse config file: {}", config_path.display());
                    }
                }
            }
        }
        eprintln!("[Config] Using default configuration");
        let config = Self::default();
        eprintln!("[Config] Font: {}", config.font_family);
        config
    }

    #[allow(dead_code)]
    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let config_path = Self::config_path()?;
        let config_dir = config_path.parent().unwrap();

        // Create config directory if it doesn't exist
        std::fs::create_dir_all(config_dir)?;

        // Write config file
        let content = toml::to_string_pretty(self)?;
        std::fs::write(&config_path, content)?;
        eprintln!("[Config] Saved to {}", config_path.display());
        Ok(())
    }

    pub fn session_history_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
        let config_dir = dirs::config_dir()
            .ok_or("Failed to determine config directory")?;
        Ok(config_dir
            .join("terminal_emulator")
            .join("session_history.json"))
    }

    fn config_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
        let config_dir = dirs::config_dir()
            .ok_or("Failed to determine config directory")?;
        Ok(config_dir.join("terminal_emulator").join("config.toml"))
    }

    pub fn get_font_family(&self) -> &str {
        &self.font_family
    }
}

pub fn create_default_config() {
    let config = Config::default();
    if let Err(e) = config.save() {
        eprintln!("[Config] Warning: Could not save default config: {}", e);
    }
}
