/*!
Frame data structures and processing.

This module provides the high-level frame data structures used throughout
the system for representing complete frames with headers and pixel data.
*/

use crate::hword::{HWord, ControlBits};
use crate::coordinates::{CoordinateData, CoordinatePoint, FieldWhitelist, extract_coordinates_from_hword};
use crate::error::{SharedError, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Frame header containing register data and metadata
#[derive(Debug, Clone)]
pub struct FrameHeader {
    pub hwords: Vec<HWord>,
    pub registers: Vec<u16>, // Extracted register values
}

impl FrameHeader {
    /// Create a new empty frame header
    pub fn new() -> Self {
        Self {
            hwords: Vec::new(),
            registers: Vec::new(),
        }
    }
    
    /// Add an HWORD to the header
    pub fn add_hword(&mut self, hword: HWord) -> Result<()> {
        if !hword.control_bits.is_header() {
            return Err(SharedError::invalid_frame("Attempted to add non-header HWORD to frame header"));
        }
        
        self.hwords.push(hword);
        Ok(())
    }
    
    /// Check if the header is complete (110 HWORDs expected)
    pub fn is_complete(&self) -> bool {
        self.hwords.len() >= crate::protocol::HEADER_HWORDS_PER_FRAME
    }
    
    /// Extract register data from header HWORDs
    pub fn extract_registers(&mut self) -> Result<()> {
        self.registers.clear();
        
        for hword in &self.hwords {
            // Each header HWORD contains 5 x 16-bit registers in bits 79:0
            let data = hword.data_as_u128();
            
            // Extract 5 registers (16 bits each) from bits 79:0
            for i in 0..5 {
                let register = ((data >> (i * 16)) & 0xFFFF) as u16;
                self.registers.push(register);
            }
        }
        
        Ok(())
    }
}

impl Default for FrameHeader {
    fn default() -> Self {
        Self::new()
    }
}

/// Pixel data containing coordinate information
#[derive(Debug, Clone)]
pub struct PixelData {
    pub hwords: Vec<HWord>,
}

impl PixelData {
    /// Create new empty pixel data
    pub fn new() -> Self {
        Self {
            hwords: Vec::new(),
        }
    }
    
    /// Add an HWORD to the pixel data
    pub fn add_hword(&mut self, hword: HWord) -> Result<()> {
        if !hword.control_bits.is_pixel() {
            return Err(SharedError::invalid_frame("Attempted to add non-pixel HWORD to pixel data"));
        }
        
        self.hwords.push(hword);
        Ok(())
    }
    
    /// Get the number of pixel HWORDs
    pub fn len(&self) -> usize {
        self.hwords.len()
    }
    
    /// Check if pixel data is empty
    pub fn is_empty(&self) -> bool {
        self.hwords.is_empty()
    }
    
    /// Extract coordinate data from pixel HWORDs
    pub fn extract_coordinates(&self, whitelist: &FieldWhitelist, decimation: usize) -> CoordinateData {
        let mut coordinates = CoordinateData::with_capacity(self.hwords.len() / decimation.max(1));
        
        for (i, hword) in self.hwords.iter().enumerate() {
            // Apply decimation
            if decimation > 1 && i % decimation != 0 {
                continue;
            }
            
            if let Some(point) = extract_coordinates_from_hword(hword, whitelist) {
                coordinates.add_point(point);
            }
        }
        
        coordinates
    }
}

impl Default for PixelData {
    fn default() -> Self {
        Self::new()
    }
}

/// Complete frame with header and pixel data
#[derive(Debug, Clone)]
pub struct Frame {
    pub frame_id: u32,
    pub header: FrameHeader,
    pub pixels: PixelData,
    pub frame_type: String,
}

impl Frame {
    /// Create a new frame with the given ID
    pub fn new(frame_id: u32) -> Self {
        Self {
            frame_id,
            header: FrameHeader::new(),
            pixels: PixelData::new(),
            frame_type: "point_cloud".to_string(),
        }
    }
    
    /// Load a frame from a .dsql file
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        
        // Extract frame number from filename
        let frame_id = extract_frame_number_from_path(path)?;
        
        // Read the file as raw bytes
        let data = std::fs::read(path)?;
        
        // Parse HWORDs from the raw data
        Self::from_bytes(frame_id, &data)
    }
    
    /// Create a frame from raw bytes
    pub fn from_bytes(frame_id: u32, data: &[u8]) -> Result<Self> {
        if data.len() % crate::protocol::HWORD_SIZE_BYTES != 0 {
            return Err(SharedError::invalid_file_format(
                format!("File size {} is not a multiple of HWORD size (12 bytes)", data.len())
            ));
        }
        
        let mut frame = Frame::new(frame_id);
        let mut in_header = false;
        
        // Process each 12-byte HWORD
        for chunk in data.chunks_exact(crate::protocol::HWORD_SIZE_BYTES) {
            let hword_bytes: [u8; 12] = chunk.try_into()
                .map_err(|_| SharedError::invalid_file_format("Invalid HWORD chunk size"))?;
            
            let hword = HWord::from_bytes(&hword_bytes)?;
            
            match hword.control_bits {
                ControlBits::FirstHeader => {
                    // Start of new frame - should be at the beginning
                    in_header = true;
                    frame.header.add_hword(hword)?;
                }
                ControlBits::SubsequentHeader => {
                    if in_header {
                        frame.header.add_hword(hword)?;
                    }
                }
                ControlBits::FirstPixel => {
                    // Transition from header to pixel data
                    in_header = false;
                    frame.pixels.add_hword(hword)?;
                }
                ControlBits::SubsequentPixel => {
                    if !in_header {
                        frame.pixels.add_hword(hword)?;
                    }
                }
                ControlBits::Idle => {
                    // Skip idle HWORDs
                    continue;
                }
                _ => {
                    // Skip reserved control bits
                    continue;
                }
            }
        }
        
        // Extract register data from header
        frame.header.extract_registers()?;
        
        Ok(frame)
    }
    
    /// Get the frame number
    pub fn number(&self) -> u32 {
        self.frame_id
    }
    
    /// Get the frame type
    pub fn frame_type(&self) -> &str {
        &self.frame_type
    }
    
    /// Get the expected number of pixels (from header data if available)
    pub fn num_pixels(&self) -> usize {
        // For now, return the actual number of pixel HWORDs
        // In the future, this could be extracted from header registers
        self.pixels.len()
    }
    
    /// Extract coordinate data with optional decimation and field filtering
    pub fn data(&self, decimation: Option<usize>, field_whitelist: Option<&[&str]>) -> CoordinateData {
        let decimation = decimation.unwrap_or(1);
        
        let whitelist = if let Some(fields) = field_whitelist {
            FieldWhitelist::new(fields)
        } else {
            FieldWhitelist::all()
        };
        
        self.pixels.extract_coordinates(&whitelist, decimation)
    }
}

/// Extract frame number from .dsql file path
fn extract_frame_number_from_path(path: &Path) -> Result<u32> {
    let filename = path.file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| SharedError::invalid_file_format("Invalid filename"))?;

    // Try to parse as hex (8-digit format like "00000001")
    if filename.len() == 8 {
        if let Ok(frame_id) = u32::from_str_radix(filename, 16) {
            return Ok(frame_id);
        }
    }

    // Try to parse as decimal
    if let Ok(frame_id) = filename.parse::<u32>() {
        return Ok(frame_id);
    }

    // Try to extract numbers from filename (e.g., "frame_123", "test_456", "data_001")
    use regex::Regex;
    let re = Regex::new(r"(\d+)").unwrap();
    if let Some(captures) = re.find(filename) {
        if let Ok(frame_id) = captures.as_str().parse::<u32>() {
            return Ok(frame_id);
        }
    }

    // If no number found, use a hash of the filename as frame ID
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    filename.hash(&mut hasher);
    let hash = hasher.finish();

    // Use the lower 32 bits of the hash as frame ID
    Ok((hash & 0xFFFFFFFF) as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frame_number_extraction() {
        use std::path::PathBuf;
        
        // Test hex format
        let path = PathBuf::from("00000001.dsql");
        assert_eq!(extract_frame_number_from_path(&path).unwrap(), 1);
        
        let path = PathBuf::from("000000FF.dsql");
        assert_eq!(extract_frame_number_from_path(&path).unwrap(), 255);
        
        // Test decimal format
        let path = PathBuf::from("123.dsql");
        assert_eq!(extract_frame_number_from_path(&path).unwrap(), 123);
    }
    
    #[test]
    fn test_frame_creation() {
        let frame = Frame::new(42);
        assert_eq!(frame.number(), 42);
        assert_eq!(frame.frame_type(), "point_cloud");
        assert!(frame.header.hwords.is_empty());
        assert!(frame.pixels.is_empty());
    }
}
