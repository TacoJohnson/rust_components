/*!
HWORD (96-bit word) parsing and manipulation.

This module provides the core HWORD data structure and parsing logic
used throughout the frame processing pipeline.
*/

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Control bit values for HWORDs according to the protocol specification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum ControlBits {
    Reserved0 = 0b000,
    Reserved1 = 0b001,
    FirstHeader = 0b010,
    SubsequentHeader = 0b011,
    FirstPixel = 0b100,
    SubsequentPixel = 0b101,
    Reserved6 = 0b110,
    Idle = 0b111,
}

impl ControlBits {
    /// Parse control bits from a u8 value
    pub fn from_u8(value: u8) -> Option<Self> {
        match value & 0b111 {
            0b000 => Some(Self::Reserved0),
            0b001 => Some(Self::Reserved1),
            0b010 => Some(Self::FirstHeader),
            0b011 => Some(Self::SubsequentHeader),
            0b100 => Some(Self::FirstPixel),
            0b101 => Some(Self::SubsequentPixel),
            0b110 => Some(Self::Reserved6),
            0b111 => Some(Self::Idle),
            _ => None,
        }
    }
    
    /// Check if this is a header HWORD
    pub fn is_header(self) -> bool {
        matches!(self, Self::FirstHeader | Self::SubsequentHeader)
    }
    
    /// Check if this is a pixel HWORD
    pub fn is_pixel(self) -> bool {
        matches!(self, Self::FirstPixel | Self::SubsequentPixel)
    }
    
    /// Check if this is a frame start HWORD
    pub fn is_frame_start(self) -> bool {
        matches!(self, Self::FirstHeader)
    }
    
    /// Check if this is an idle HWORD
    pub fn is_idle(self) -> bool {
        matches!(self, Self::Idle)
    }
}

/// Errors that can occur during HWORD parsing
#[derive(Error, Debug)]
pub enum HWordError {
    #[error("Invalid HWORD length: expected 12 bytes, got {0}")]
    InvalidLength(usize),
    
    #[error("Invalid control bits: {0:03b}")]
    InvalidControlBits(u8),
    
    #[error("Parity check failed")]
    ParityCheckFailed,
    
    #[error("Invalid data field")]
    InvalidDataField,
}

/// A 96-bit HWORD (12 bytes) as defined in the protocol
#[derive(Debug, Clone, PartialEq)]
pub struct HWord {
    pub control_bits: ControlBits,
    pub parity: bool,
    pub data: [u8; 11], // 92 bits = 11.5 bytes, we'll use 11 bytes and handle the remaining 4 bits
    pub remaining_bits: u8, // The remaining 4 bits from the 92-bit data field
}

impl HWord {
    /// Parse an HWORD from 12 bytes of raw data
    pub fn from_bytes(bytes: &[u8; 12]) -> Result<Self, HWordError> {
        // Extract control bits directly from the first byte (top 3 bits)
        let raw_control_bits = (bytes[0] >> 5) & 0x7;
        
        let control_bits = ControlBits::from_u8(raw_control_bits)
            .ok_or(HWordError::InvalidControlBits(raw_control_bits))?;

        // Reconstruct the 96-bit word from big-endian bytes
        let mut word_96bit: u128 = 0;
        for (i, &byte) in bytes.iter().enumerate() {
            word_96bit |= (byte as u128) << (88 - i * 8);
        }

        // Extract parity bit (bit 92)
        let parity = ((word_96bit >> 92) & 0x1) != 0;

        // Extract the 92-bit data field (bits 91-0)
        let data_92bit = word_96bit & ((1u128 << 92) - 1);

        // Pack into 11 bytes + 4 remaining bits
        let mut data = [0u8; 11];
        for i in 0..11 {
            data[i] = ((data_92bit >> (i * 8)) & 0xFF) as u8;
        }
        let remaining_bits = ((data_92bit >> 88) & 0xF) as u8;

        Ok(HWord {
            control_bits,
            parity,
            data,
            remaining_bits,
        })
    }
    
    /// Convert HWORD back to 12 bytes
    pub fn to_bytes(&self) -> [u8; 12] {
        // Reconstruct the 92-bit data field from the 11 bytes + 4 remaining bits
        let mut data_92bit: u128 = 0;
        for i in 0..11 {
            data_92bit |= (self.data[i] as u128) << (i * 8);
        }
        data_92bit |= (self.remaining_bits as u128) << 88;

        // Reconstruct the full 96-bit word
        let control_bits = (self.control_bits as u128) << 93;
        let parity_bit = if self.parity { 1u128 << 92 } else { 0 };
        let word_96bit = control_bits | parity_bit | data_92bit;

        // Convert back to 12 bytes (big-endian)
        let mut bytes = [0u8; 12];
        for i in 0..12 {
            bytes[i] = ((word_96bit >> (88 - i * 8)) & 0xFF) as u8;
        }

        bytes
    }
    
    /// Verify the parity of this HWORD
    pub fn verify_parity(&self) -> bool {
        // Reconstruct the full word and count 1s
        let bytes = self.to_bytes();
        let mut word_96bit: u128 = 0;
        for (i, &byte) in bytes.iter().enumerate() {
            word_96bit |= (byte as u128) << (88 - i * 8);
        }
        
        // Count 1s in the entire 96-bit word
        let ones_count = word_96bit.count_ones();
        
        // Should be odd parity
        ones_count % 2 == 1
    }
    
    /// Get the 92-bit data field as a u128
    pub fn data_as_u128(&self) -> u128 {
        let mut data_92bit: u128 = 0;
        for i in 0..11 {
            data_92bit |= (self.data[i] as u128) << (i * 8);
        }
        data_92bit |= (self.remaining_bits as u128) << 88;
        data_92bit
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_control_bits_parsing() {
        assert_eq!(ControlBits::from_u8(0b010), Some(ControlBits::FirstHeader));
        assert_eq!(ControlBits::from_u8(0b100), Some(ControlBits::FirstPixel));
        assert_eq!(ControlBits::from_u8(0b111), Some(ControlBits::Idle));
        assert_eq!(ControlBits::from_u8(0b1000), None); // Invalid
    }

    #[test]
    fn test_hword_roundtrip() {
        // Create test data
        let original_bytes = [
            0x4F, 0x76, 0xB3, 0xBC, 0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0
        ];

        // Parse and convert back
        let hword = HWord::from_bytes(&original_bytes).unwrap();
        let converted_bytes = hword.to_bytes();

        assert_eq!(original_bytes, converted_bytes);
    }

    #[test]
    fn test_control_bits_classification() {
        assert!(ControlBits::FirstHeader.is_header());
        assert!(ControlBits::SubsequentHeader.is_header());
        assert!(ControlBits::FirstPixel.is_pixel());
        assert!(ControlBits::SubsequentPixel.is_pixel());
        assert!(ControlBits::FirstHeader.is_frame_start());
        assert!(!ControlBits::SubsequentHeader.is_frame_start());
        assert!(ControlBits::Idle.is_idle());
    }
}
