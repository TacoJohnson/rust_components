/*!
Coordinate extraction and conversion utilities.

This module handles the conversion from HWORD data fields to floating-point
coordinates and other frame data fields.
*/

use crate::hword::{HWord, ControlBits};
use crate::protocol::COORDINATE_SCALE_FACTOR;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Represents the fields that can be extracted from frame data
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FieldType {
    X,
    Y,
    Z,
    Intensity,
    Gain,
    OverRange,
}

impl FieldType {
    /// Parse field type from string (case-insensitive)
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "x" => Some(Self::X),
            "y" => Some(Self::Y),
            "z" => Some(Self::Z),
            "intensity" => Some(Self::Intensity),
            "gain" => Some(Self::Gain),
            "over_range" | "overrange" => Some(Self::OverRange),
            _ => None,
        }
    }
    
    /// Convert to string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::X => "x",
            Self::Y => "y",
            Self::Z => "z",
            Self::Intensity => "intensity",
            Self::Gain => "gain",
            Self::OverRange => "over_range",
        }
    }
}

/// Field whitelist for controlling which fields to extract
#[derive(Debug, Clone)]
pub struct FieldWhitelist {
    fields: HashSet<FieldType>,
}

impl FieldWhitelist {
    /// Create a new field whitelist from a list of field names
    pub fn new(field_names: &[&str]) -> Self {
        let fields = field_names
            .iter()
            .filter_map(|name| FieldType::from_str(name))
            .collect();
        
        Self { fields }
    }
    
    /// Create a whitelist with all available fields
    pub fn all() -> Self {
        let fields = [
            FieldType::X,
            FieldType::Y,
            FieldType::Z,
            FieldType::Intensity,
            FieldType::Gain,
            FieldType::OverRange,
        ].into_iter().collect();
        
        Self { fields }
    }
    
    /// Check if a field should be included
    pub fn includes(&self, field: &FieldType) -> bool {
        self.fields.contains(field)
    }
    
    /// Get all included fields
    pub fn fields(&self) -> &HashSet<FieldType> {
        &self.fields
    }
}

impl Default for FieldWhitelist {
    fn default() -> Self {
        Self::all()
    }
}

/// Represents a single point's coordinate data
#[derive(Debug, Clone, PartialEq)]
pub struct CoordinatePoint {
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub z: Option<f64>,
    pub intensity: Option<u16>,
    pub gain: Option<bool>,  // HG/LG flag
    pub over_range: Option<bool>,
}

impl CoordinatePoint {
    /// Create a new coordinate point with all fields set to None
    pub fn new() -> Self {
        Self {
            x: None,
            y: None,
            z: None,
            intensity: None,
            gain: None,
            over_range: None,
        }
    }
}

impl Default for CoordinatePoint {
    fn default() -> Self {
        Self::new()
    }
}

/// Collection of coordinate data for multiple points
#[derive(Debug, Clone)]
pub struct CoordinateData {
    pub points: Vec<CoordinatePoint>,
}

impl CoordinateData {
    /// Create new empty coordinate data
    pub fn new() -> Self {
        Self {
            points: Vec::new(),
        }
    }
    
    /// Create coordinate data with a specific capacity
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            points: Vec::with_capacity(capacity),
        }
    }
    
    /// Add a point to the coordinate data
    pub fn add_point(&mut self, point: CoordinatePoint) {
        self.points.push(point);
    }
    
    /// Get the number of points
    pub fn len(&self) -> usize {
        self.points.len()
    }
    
    /// Check if the coordinate data is empty
    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }
    
    /// Apply decimation to the coordinate data
    pub fn decimate(&mut self, factor: usize) {
        if factor <= 1 {
            return;
        }
        
        let decimated_points: Vec<_> = self.points
            .iter()
            .step_by(factor)
            .cloned()
            .collect();
        
        self.points = decimated_points;
    }
}

impl Default for CoordinateData {
    fn default() -> Self {
        Self::new()
    }
}

/// Extract coordinate data from a pixel HWORD
pub fn extract_coordinates_from_hword(hword: &HWord, whitelist: &FieldWhitelist) -> Option<CoordinatePoint> {
    // Only process pixel HWORDs
    if !hword.control_bits.is_pixel() {
        return None;
    }
    
    let data = hword.data_as_u128();
    let mut point = CoordinatePoint::new();
    
    // Extract X coordinate (bits 23:0, 9.10 fixed point, 19 bits, SIGNED)
    if whitelist.includes(&FieldType::X) {
        let x_raw = (data & 0x7FFFF) as u32; // 19 bits
        // Sign extend from 19 bits to 32 bits
        let x_signed = if x_raw & 0x40000 != 0 {
            // Negative: set upper bits to 1
            (x_raw | 0xFFF80000) as i32
        } else {
            // Positive: keep as is
            x_raw as i32
        };
        point.x = Some(x_signed as f64 / COORDINATE_SCALE_FACTOR);
    }

    // Extract Y coordinate (bits 47:24, 9.10 fixed point, 19 bits, SIGNED)
    if whitelist.includes(&FieldType::Y) {
        let y_raw = ((data >> 24) & 0x7FFFF) as u32; // 19 bits
        // Sign extend from 19 bits to 32 bits
        let y_signed = if y_raw & 0x40000 != 0 {
            // Negative: set upper bits to 1
            (y_raw | 0xFFF80000) as i32
        } else {
            // Positive: keep as is
            y_raw as i32
        };
        point.y = Some(y_signed as f64 / COORDINATE_SCALE_FACTOR);
    }

    // Extract Z coordinate (bits 71:48, 12.10 fixed point, 22 bits, SIGNED)
    if whitelist.includes(&FieldType::Z) {
        let z_raw = ((data >> 48) & 0x3FFFFF) as u32; // 22 bits
        // Sign extend from 22 bits to 32 bits
        let z_signed = if z_raw & 0x200000 != 0 {
            // Negative: set upper bits to 1
            (z_raw | 0xFFC00000) as i32
        } else {
            // Positive: keep as is
            z_raw as i32
        };
        point.z = Some(z_signed as f64 / COORDINATE_SCALE_FACTOR);
    }
    
    // Extract Intensity (bits 87:72, 12 bits)
    if whitelist.includes(&FieldType::Intensity) {
        let intensity = ((data >> 72) & 0xFFF) as u16; // 12 bits
        point.intensity = Some(intensity);
    }
    
    // Extract Over-range flag (bit 90)
    if whitelist.includes(&FieldType::OverRange) {
        let over_range = ((data >> 90) & 0x1) != 0;
        point.over_range = Some(over_range);
    }
    
    // Extract HG/LG flag (bit 91) - this is the "gain" field
    if whitelist.includes(&FieldType::Gain) {
        let hg_lg = ((data >> 91) & 0x1) != 0;
        point.gain = Some(hg_lg); // true = LG (Low Gain), false = HG (High Gain)
    }
    
    Some(point)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hword::ControlBits;

    #[test]
    fn test_field_type_parsing() {
        assert_eq!(FieldType::from_str("x"), Some(FieldType::X));
        assert_eq!(FieldType::from_str("X"), Some(FieldType::X));
        assert_eq!(FieldType::from_str("intensity"), Some(FieldType::Intensity));
        assert_eq!(FieldType::from_str("over_range"), Some(FieldType::OverRange));
        assert_eq!(FieldType::from_str("invalid"), None);
    }

    #[test]
    fn test_field_whitelist() {
        let whitelist = FieldWhitelist::new(&["x", "y", "z"]);
        assert!(whitelist.includes(&FieldType::X));
        assert!(whitelist.includes(&FieldType::Y));
        assert!(whitelist.includes(&FieldType::Z));
        assert!(!whitelist.includes(&FieldType::Intensity));
    }

    #[test]
    fn test_coordinate_extraction() {
        // Create a test pixel HWORD with known data
        let mut hword = HWord {
            control_bits: ControlBits::FirstPixel,
            parity: false,
            data: [0; 11],
            remaining_bits: 0,
        };
        
        // Set some test coordinate data
        // X = 1024 (1.0 in fixed point), Y = 2048 (2.0), Z = 3072 (3.0)
        let test_data: u128 = 1024 | (2048 << 24) | (3072 << 48) | (100 << 72); // intensity = 100
        
        // Pack the data into the HWORD
        for i in 0..11 {
            hword.data[i] = ((test_data >> (i * 8)) & 0xFF) as u8;
        }
        hword.remaining_bits = ((test_data >> 88) & 0xF) as u8;
        
        let whitelist = FieldWhitelist::all();
        let point = extract_coordinates_from_hword(&hword, &whitelist).unwrap();
        
        assert_eq!(point.x, Some(1.0));
        assert_eq!(point.y, Some(2.0));
        assert_eq!(point.z, Some(3.0));
        assert_eq!(point.intensity, Some(100));
    }
}
