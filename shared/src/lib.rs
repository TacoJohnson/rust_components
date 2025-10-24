/*!
# Shared Types and Utilities

This crate contains common types and utilities shared between all Rust components
in the Universal Instrument Control system.

## Core Types

- [`HWord`] - 96-bit HWORD data structure
- [`ControlBits`] - Control bit enumeration
- [`Frame`] - Complete frame data structure
- [`FrameData`] - Decoded coordinate data

## Modules

- [`hword`] - HWORD parsing and manipulation
- [`coordinates`] - Coordinate conversion utilities
- [`error`] - Common error types
*/

pub mod hword;
pub mod coordinates;
pub mod error;
pub mod frame;

// Re-export commonly used types
pub use hword::{HWord, ControlBits, HWordError};
pub use coordinates::{CoordinateData, FieldWhitelist};
pub use error::{SharedError, Result};
pub use frame::{Frame, FrameHeader, PixelData};

/// Version information for the shared library
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Protocol constants
pub mod protocol {
    /// Size of an HWORD in bytes
    pub const HWORD_SIZE_BYTES: usize = 12;
    
    /// Size of an HWORD in bits
    pub const HWORD_SIZE_BITS: usize = 96;
    
    /// Size of the data field in bits
    pub const DATA_FIELD_BITS: usize = 92;
    
    /// Expected number of header HWORDs per frame
    pub const HEADER_HWORDS_PER_FRAME: usize = 110;
    
    /// Fixed-point scaling factor for coordinates (2^10 = 1024)
    pub const COORDINATE_SCALE_FACTOR: f64 = 1024.0;
}
