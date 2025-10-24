/*!
Configuration management for the frame grabber application.
*/

use serde::{Deserialize, Serialize};
use std::path::Path;
use anyhow::{Context, Result};

/// Main application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub framegrabber: FrameGrabberConfig,
    pub gui: GuiConfig,
}

impl AppConfig {
    /// Create a new configuration with default values
    pub fn new() -> Self {
        Self {
            framegrabber: FrameGrabberConfig::default(),
            gui: GuiConfig::default(),
        }
    }
    
    /// Load configuration from a TOML file
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = std::fs::read_to_string(path.as_ref())
            .with_context(|| format!("Failed to read config file: {}", path.as_ref().display()))?;
        
        let config: AppConfig = toml::from_str(&content)
            .with_context(|| "Failed to parse config file as TOML")?;
        
        Ok(config)
    }
    
    /// Save configuration to a TOML file
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let content = toml::to_string_pretty(self)
            .with_context(|| "Failed to serialize config to TOML")?;
        
        std::fs::write(path.as_ref(), content)
            .with_context(|| format!("Failed to write config file: {}", path.as_ref().display()))?;
        
        Ok(())
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Frame grabber specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrameGrabberConfig {
    /// UDP bind address
    pub udp_bind_addr: String,
    
    /// UDP port to listen on
    pub udp_port: u16,
    
    /// Output directory for .dsql files
    pub output_directory: String,
    
    /// Channel buffer size for UDP data
    pub channel_buffer_size: usize,
    
    /// Enable file storage
    pub enable_storage: bool,
    
    /// Enable real-time processing
    pub enable_realtime_processing: bool,
    
    /// Drop packets with parity errors
    pub drop_parity_errors: bool,
    
    /// Maximum frame size in HWORDs
    pub max_frame_size_hwords: usize,
    
    /// Statistics reporting interval in seconds
    pub stats_interval_seconds: u64,
}

impl Default for FrameGrabberConfig {
    fn default() -> Self {
        Self {
            udp_bind_addr: "0.0.0.0".to_string(),
            udp_port: 12345,
            output_directory: "./frames".to_string(),
            channel_buffer_size: 1000,
            enable_storage: true,
            enable_realtime_processing: false,
            drop_parity_errors: false,
            max_frame_size_hwords: 1000000, // 1M HWORDs max per frame
            stats_interval_seconds: 10,
        }
    }
}

/// GUI specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuiConfig {
    /// Window width
    pub window_width: f32,
    
    /// Window height
    pub window_height: f32,
    
    /// Enable dark mode
    pub dark_mode: bool,
    
    /// Auto-save configuration on exit
    pub auto_save_config: bool,
    
    /// Show advanced options
    pub show_advanced_options: bool,
}

impl Default for GuiConfig {
    fn default() -> Self {
        Self {
            window_width: 1200.0,
            window_height: 800.0,
            dark_mode: true,
            auto_save_config: true,
            show_advanced_options: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_config_roundtrip() {
        let original_config = AppConfig::new();
        
        let temp_file = NamedTempFile::new().unwrap();
        let temp_path = temp_file.path();
        
        // Save and load
        original_config.save_to_file(temp_path).unwrap();
        let loaded_config = AppConfig::load_from_file(temp_path).unwrap();
        
        // Compare (using debug format since we don't have PartialEq)
        assert_eq!(format!("{:?}", original_config), format!("{:?}", loaded_config));
    }
    
    #[test]
    fn test_default_values() {
        let config = AppConfig::new();
        
        assert_eq!(config.framegrabber.udp_bind_addr, "0.0.0.0");
        assert_eq!(config.framegrabber.udp_port, 12345);
        assert_eq!(config.framegrabber.output_directory, "./frames");
        assert!(config.framegrabber.enable_storage);
        assert!(!config.framegrabber.enable_realtime_processing);
        
        assert_eq!(config.gui.window_width, 1200.0);
        assert_eq!(config.gui.window_height, 800.0);
        assert!(config.gui.dark_mode);
        assert!(config.gui.auto_save_config);
    }
}
