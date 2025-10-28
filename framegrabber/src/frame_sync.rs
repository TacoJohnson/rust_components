/*!
Count-based frame synchronization state machine.

This module implements the frame synchronization logic as described in the
"Frame Data Interface.docx" specification, using HWORD counting and header
index validation instead of signature-based detection.
*/

use tracing::{info, warn, debug};
use shared::hword::HWord;
use shared::protocol::HEADER_HWORDS_PER_FRAME;

/// Idle HWORD pattern for initial synchronization (control bits = 111)
/// Pattern: 0xFD3C4B5A69788796A5B4C3B2 (12 bytes)
const IDLE_HWORD_PATTERN: [u8; 12] = [
    0xFD, 0x3C, 0x4B, 0x5A, 0x69, 0x78, 0x87, 0x96, 0xA5, 0xB4, 0xC3, 0xB2
];

/// Frame synchronization state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameSyncState {
    /// Waiting for initial HWORD synchronization
    WaitingForSync,
    /// Waiting for first frame header
    WaitingForFrame,
    /// Collecting header HWORDs (expecting 110)
    CollectingHeader { count: usize, last_index: Option<u8> },
    /// Collecting pixel HWORDs
    CollectingPixels { header_count: usize, pixel_count: usize, expected_pixels: usize },
    /// Frame complete and ready to write
    FrameComplete,
}

/// Frame mode detection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameMode {
    /// 1-point scan: 110 header + 1 pixel = 111 HWORDs total
    OnePointScan,
    /// 5-point scan: 110 header + 5 pixels = 115 HWORDs total
    FivePointScan,
    /// Imaging mode: 110 header + variable pixels (determined by NUM_PIXELS_RW)
    Imaging { expected_pixels: usize },
    /// Unknown mode: use default behavior
    Unknown,
}

impl FrameMode {
    /// Get expected pixel count for this mode
    pub fn expected_pixel_count(&self) -> usize {
        match self {
            FrameMode::OnePointScan => 1,
            FrameMode::FivePointScan => 5,
            FrameMode::Imaging { expected_pixels } => *expected_pixels,
            FrameMode::Unknown => 1, // Default to 1-point scan
        }
    }

    /// Get total expected HWORD count (header + pixels)
    pub fn total_hword_count(&self) -> usize {
        HEADER_HWORDS_PER_FRAME + self.expected_pixel_count()
    }

    /// Detect frame mode from header size and pixel count
    pub fn detect(header_count: usize, pixel_count: usize) -> Self {
        let total = header_count + pixel_count;
        match total {
            111 => FrameMode::OnePointScan,
            115 => FrameMode::FivePointScan,
            _ if pixel_count > 5 => FrameMode::Imaging { expected_pixels: pixel_count },
            _ => FrameMode::Unknown,
        }
    }
}

/// Frame synchronization engine
pub struct FrameSyncEngine {
    state: FrameSyncState,
    frame_buffer: Vec<u8>,
    current_mode: FrameMode,
    frames_completed: u64,
    sync_errors: u64,
    header_index_errors: u64,
}

impl FrameSyncEngine {
    /// Create a new frame synchronization engine
    pub fn new() -> Self {
        Self {
            state: FrameSyncState::WaitingForSync,
            frame_buffer: Vec::new(),
            current_mode: FrameMode::Unknown,
            frames_completed: 0,
            sync_errors: 0,
            header_index_errors: 0,
        }
    }

    /// Get current state
    pub fn state(&self) -> FrameSyncState {
        self.state
    }

    /// Get statistics
    pub fn stats(&self) -> (u64, u64, u64) {
        (self.frames_completed, self.sync_errors, self.header_index_errors)
    }

    /// Check if an HWORD matches the Idle pattern
    fn is_idle_hword(chunk: &[u8]) -> bool {
        if chunk.len() != 12 {
            return false;
        }
        
        // Check control bits first (should be 111 = 0b111)
        let control_bits = (chunk[0] >> 5) & 0x07;
        if control_bits != 0b111 {
            return false;
        }
        
        // Check if it matches the Idle pattern
        chunk == IDLE_HWORD_PATTERN
    }

    /// Extract header index from header HWORD (bits 87:84)
    fn extract_header_index(hword: &HWord) -> Option<u8> {
        if !hword.control_bits.is_header() {
            return None;
        }
        
        let data = hword.data_as_u128();
        // Header index is in bits 87:84 of the 92-bit data field
        let index = ((data >> 84) & 0x0F) as u8;
        Some(index)
    }

    /// Extract NUM_PIXELS_RW from header (Register 2, which is in header HWORD 0)
    /// Each header HWORD contains 5 registers (16 bits each) in bits 79:0
    /// Register 2 is at bits 47:32 of the first header HWORD
    ///
    /// NOTE: NUM_PIXELS_RW is a 16-bit register (max 65,535), but documentation mentions
    /// imaging modes with up to 122,000 pixels. This may indicate:
    /// - A scaling factor is applied
    /// - Multiple registers are used
    /// - Documentation inconsistency
    /// For now, we extract the 16-bit value directly.
    fn extract_num_pixels(first_header_hword: &HWord) -> Option<usize> {
        if !first_header_hword.control_bits.is_frame_start() {
            return None;
        }

        let data = first_header_hword.data_as_u128();
        // Register 2 is at bits 47:32 (third 16-bit register)
        let num_pixels = ((data >> 32) & 0xFFFF) as usize;

        // Sanity check: reasonable pixel count
        // u16 max is 65,535, but doc mentions up to 122,000 for imaging
        if num_pixels > 0 {
            Some(num_pixels)
        } else {
            // Default to 1 pixel for 1-point scan mode if 0
            Some(1)
        }
    }

    /// Process a 12-byte HWORD chunk
    /// Returns Some(frame_data) when a complete frame is ready
    pub fn process_hword(&mut self, chunk: &[u8; 12]) -> Option<Vec<u8>> {
        // Parse the HWORD
        let hword = match HWord::from_bytes(chunk) {
            Ok(h) => h,
            Err(e) => {
                debug!("Failed to parse HWORD: {}", e);
                self.sync_errors += 1;
                return None;
            }
        };

        match self.state {
            FrameSyncState::WaitingForSync => {
                // Look for Idle HWORD to establish synchronization
                if Self::is_idle_hword(chunk) {
                    info!("🔒 SYNCHRONIZED: Found Idle HWORD pattern");
                    self.state = FrameSyncState::WaitingForFrame;
                } else if hword.control_bits.is_frame_start() {
                    // Also accept FirstHeader as sync point
                    info!("🔒 SYNCHRONIZED: Found FirstHeader");
                    self.state = FrameSyncState::WaitingForFrame;
                    // Process this HWORD again in the new state
                    return self.process_hword(chunk);
                }
                None
            }

            FrameSyncState::WaitingForFrame => {
                // Wait for FirstHeader to start a new frame
                if hword.control_bits.is_frame_start() {
                    debug!("📦 Frame start detected");
                    self.frame_buffer.clear();
                    self.frame_buffer.extend_from_slice(chunk);
                    
                    // Try to extract expected pixel count from header
                    let expected_pixels = Self::extract_num_pixels(&hword)
                        .unwrap_or(1); // Default to 1-point scan
                    
                    self.current_mode = if expected_pixels == 1 {
                        FrameMode::OnePointScan
                    } else if expected_pixels == 5 {
                        FrameMode::FivePointScan
                    } else {
                        FrameMode::Imaging { expected_pixels }
                    };
                    
                    debug!("Frame mode: {:?}", self.current_mode);
                    
                    self.state = FrameSyncState::CollectingHeader {
                        count: 1,
                        last_index: Some(0),
                    };
                }
                None
            }

            FrameSyncState::CollectingHeader { count, last_index } => {
                // Validate header index progression
                if hword.control_bits.is_header() {
                    if let Some(index) = Self::extract_header_index(&hword) {
                        if let Some(last) = last_index {
                            let expected = (last + 1) % 16; // Header index wraps at 16
                            if index != expected && count < HEADER_HWORDS_PER_FRAME {
                                warn!("⚠️ Header index mismatch: expected {}, got {} (HWORD {})", 
                                      expected, index, count);
                                self.header_index_errors += 1;
                            }
                        }
                        
                        self.frame_buffer.extend_from_slice(chunk);
                        
                        if count + 1 >= HEADER_HWORDS_PER_FRAME {
                            // Header complete, transition to pixel collection
                            debug!("✅ Header complete ({} HWORDs)", HEADER_HWORDS_PER_FRAME);
                            self.state = FrameSyncState::CollectingPixels {
                                header_count: count + 1,
                                pixel_count: 0,
                                expected_pixels: self.current_mode.expected_pixel_count(),
                            };
                        } else {
                            self.state = FrameSyncState::CollectingHeader {
                                count: count + 1,
                                last_index: Some(index),
                            };
                        }
                    } else {
                        // No header index found, just count
                        self.frame_buffer.extend_from_slice(chunk);
                        
                        if count + 1 >= HEADER_HWORDS_PER_FRAME {
                            debug!("✅ Header complete ({} HWORDs)", HEADER_HWORDS_PER_FRAME);
                            self.state = FrameSyncState::CollectingPixels {
                                header_count: count + 1,
                                pixel_count: 0,
                                expected_pixels: self.current_mode.expected_pixel_count(),
                            };
                        } else {
                            self.state = FrameSyncState::CollectingHeader {
                                count: count + 1,
                                last_index,
                            };
                        }
                    }
                } else if hword.control_bits.is_pixel() {
                    // Premature transition to pixels - header might be shorter than expected
                    warn!("⚠️ Premature pixel data at header HWORD {}", count);
                    self.frame_buffer.extend_from_slice(chunk);
                    self.state = FrameSyncState::CollectingPixels {
                        header_count: count,
                        pixel_count: 1,
                        expected_pixels: self.current_mode.expected_pixel_count(),
                    };
                }
                None
            }

            FrameSyncState::CollectingPixels { header_count, pixel_count, expected_pixels } => {
                if hword.control_bits.is_pixel() {
                    self.frame_buffer.extend_from_slice(chunk);
                    
                    if pixel_count + 1 >= expected_pixels {
                        // Frame complete!
                        let total_hwords = header_count + pixel_count + 1;
                        info!("✅ Frame complete: {} header + {} pixel = {} total HWORDs ({} bytes)",
                              header_count, pixel_count + 1, total_hwords, self.frame_buffer.len());
                        
                        self.frames_completed += 1;
                        self.state = FrameSyncState::WaitingForFrame;
                        
                        // Return the completed frame
                        return Some(self.frame_buffer.clone());
                    } else {
                        self.state = FrameSyncState::CollectingPixels {
                            header_count,
                            pixel_count: pixel_count + 1,
                            expected_pixels,
                        };
                    }
                } else if hword.control_bits.is_frame_start() {
                    // New frame started before current frame completed
                    warn!("⚠️ Incomplete frame: expected {} pixels, got {} (starting new frame)",
                          expected_pixels, pixel_count);
                    self.sync_errors += 1;
                    
                    // Start new frame
                    self.frame_buffer.clear();
                    self.frame_buffer.extend_from_slice(chunk);
                    self.state = FrameSyncState::CollectingHeader {
                        count: 1,
                        last_index: Some(0),
                    };
                }
                None
            }

            FrameSyncState::FrameComplete => {
                // This state is not used in the current implementation
                // Frames are returned immediately when complete
                None
            }
        }
    }

    /// Get current frame buffer (for debugging)
    pub fn current_buffer(&self) -> &[u8] {
        &self.frame_buffer
    }
}

impl Default for FrameSyncEngine {
    fn default() -> Self {
        Self::new()
    }
}

