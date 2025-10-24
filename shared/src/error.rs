/*!
Common error types for the Universal Instrument Control Rust components.
*/

use thiserror::Error;

/// Common result type used throughout the shared library
pub type Result<T> = std::result::Result<T, SharedError>;

/// Comprehensive error type for all shared operations
#[derive(Error, Debug)]
pub enum SharedError {
    /// HWORD parsing errors
    #[error("HWORD error: {0}")]
    HWord(#[from] crate::hword::HWordError),
    
    /// I/O errors (file operations, etc.)
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    
    /// Serialization/deserialization errors
    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),
    
    /// Invalid frame data
    #[error("Invalid frame data: {0}")]
    InvalidFrame(String),
    
    /// Invalid coordinate data
    #[error("Invalid coordinate data: {0}")]
    InvalidCoordinates(String),
    
    /// File format errors
    #[error("Invalid file format: {0}")]
    InvalidFileFormat(String),
    
    /// Configuration errors
    #[error("Configuration error: {0}")]
    Config(String),
    
    /// Generic errors with context
    #[error("Error: {0}")]
    Generic(String),
}

impl SharedError {
    /// Create a new generic error with a message
    pub fn new(msg: impl Into<String>) -> Self {
        Self::Generic(msg.into())
    }
    
    /// Create a new invalid frame error
    pub fn invalid_frame(msg: impl Into<String>) -> Self {
        Self::InvalidFrame(msg.into())
    }
    
    /// Create a new invalid coordinates error
    pub fn invalid_coordinates(msg: impl Into<String>) -> Self {
        Self::InvalidCoordinates(msg.into())
    }
    
    /// Create a new invalid file format error
    pub fn invalid_file_format(msg: impl Into<String>) -> Self {
        Self::InvalidFileFormat(msg.into())
    }
    
    /// Create a new configuration error
    pub fn config(msg: impl Into<String>) -> Self {
        Self::Config(msg.into())
    }
}
