/*!
Simple frame capture implementation.

This module provides the core frame capture functionality, receiving UDP packets
and writing them as .dsql files with proper frame boundary detection.
*/

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use tokio::net::UdpSocket;
use tokio::runtime::Runtime;
use tracing::{info, warn, error};
use crossbeam_channel::{bounded, Receiver, Sender};
use shared::frame::Frame;
use shared::coordinates::FieldWhitelist;
use serde_json;
use chrono::Local;
use crate::frame_sync::FrameSyncEngine;

/// Simple framegrabber that matches the C implementation approach:
/// 1. UDP receiver thread: UDP packets -> continuous buffer
/// 2. File writer thread: continuous buffer -> 12-byte chunks -> files
/// 3. No complex parsing or frame assembly during capture
/// 4. Support for live mode (output to stdout instead of files)
/// 5. Support for debug mode (generate synthetic LiDAR data)
/// 6. Support for decode mode (output decoded coordinates instead of raw HWORD data)
/// 7. Each capture session creates a timestamped subdirectory
pub struct SimpleFrameGrabber {
    bind_addr: String,
    port: u16,
    output_dir: String,
    timestamped_output_dir: String,  // Full path including timestamp subdirectory
    save_files: bool,
    live_output: bool,
    debug_mode: bool,
    decode_mode: bool,
    running: Arc<AtomicBool>,
}

impl SimpleFrameGrabber {
    /// Create a new frame grabber with timestamped subdirectory for this session
    pub fn new(
        bind_addr: String,
        port: u16,
        output_dir: String,
        save_files: bool,
        live_output: bool,
        debug_mode: bool,
        decode_mode: bool,
    ) -> Self {
        // Generate timestamp for this capture session
        let timestamp = Local::now().format("%Y%m%d_%H%M%S").to_string();

        // Create timestamped subdirectory path
        let timestamped_output_dir = if save_files {
            format!("{}/{}", output_dir, timestamp)
        } else {
            output_dir.clone()  // Don't create subdirectory if not saving files
        };

        if save_files {
            info!("📁 Capture session timestamp: {}", timestamp);
            info!("📁 Files will be saved to: {}", timestamped_output_dir);
        }

        Self {
            bind_addr,
            port,
            output_dir,
            timestamped_output_dir,
            save_files,
            live_output,
            debug_mode,
            decode_mode,
            running: Arc::new(AtomicBool::new(true)),
        }
    }
    
    /// Get a reference to the running flag for external control
    pub fn get_running_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.running)
    }

    /// Start the frame capture process
    pub fn start(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Create channel for raw UDP data (like C version's buffer)
        // Increased from 1000 to 10000 to handle high-speed UDP streams without dropping packets
        let (data_tx, data_rx) = bounded::<Vec<u8>>(10000);

        // Clone for threads
        let running_udp = Arc::clone(&self.running);
        let running_writer = Arc::clone(&self.running);
        let bind_addr = self.bind_addr.clone();
        let port = self.port;
        let timestamped_output_dir = self.timestamped_output_dir.clone();
        let save_files = self.save_files;
        let live_output = self.live_output;
        let debug_mode = self.debug_mode;
        let decode_mode = self.decode_mode;

        // Start data source thread (UDP receiver or debug generator)
        let udp_handle = if debug_mode {
            // Debug mode: generate synthetic data
            thread::spawn(move || {
                match Self::debug_data_generator_thread(data_tx, running_udp) {
                    Ok(_) => {
                        info!("Debug data generator thread finished successfully");
                        Ok(())
                    }
                    Err(e) => {
                        error!("Debug data generator thread failed: {}", e);
                        Err(e)
                    }
                }
            })
        } else {
            // Normal mode: UDP receiver
            thread::spawn(move || {
                let rt = Runtime::new().unwrap();
                rt.block_on(async {
                    match Self::udp_receiver_thread(bind_addr, port, data_tx, running_udp).await {
                        Ok(_) => {
                            info!("UDP receiver thread finished successfully");
                            Ok(())
                        }
                        Err(e) => {
                            error!("UDP receiver thread failed: {}", e);
                            Err(e)
                        }
                    }
                })
            })
        };

        // Start file writer thread (matches C's thFrameCap)
        let writer_handle = thread::spawn(move || {
            match Self::file_writer_thread(timestamped_output_dir, data_rx, running_writer, save_files, live_output, decode_mode) {
                Ok(_) => {
                    info!("File writer thread finished successfully");
                    Ok(())
                }
                Err(e) => {
                    error!("File writer thread failed: {}", e);
                    Err(e)
                }
            }
        });

        // Wait for both threads to complete
        let udp_result = udp_handle.join().map_err(|_| "UDP thread panicked")?;
        let writer_result = writer_handle.join().map_err(|_| "Writer thread panicked")?;

        udp_result?;
        writer_result?;

        Ok(())
    }

    /// UDP receiver thread - receives packets and forwards to writer
    async fn udp_receiver_thread(
        bind_addr: String,
        port: u16,
        data_tx: Sender<Vec<u8>>,
        running: Arc<AtomicBool>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let socket_addr = format!("{}:{}", bind_addr, port);
        info!("🔌 Binding UDP socket to {}", socket_addr);

        let socket = UdpSocket::bind(&socket_addr).await?;
        info!("✅ UDP socket bound successfully");

        // Set socket buffer size (match C implementation)
        let sock_ref = socket2::SockRef::from(&socket);
        sock_ref.set_recv_buffer_size(1024 * 1024)?; // 1MB like C version
        info!("📊 Socket receive buffer set to 1MB");

        let mut buffer = vec![0u8; 4096]; // Match C's FG_UDP_BUFLEN
        let mut total_bytes = 0u64;
        let mut packet_count = 0u64;
        let mut error_count = 0u64;
        let start_time = Instant::now();

        while running.load(Ordering::SeqCst) {
            // Set a timeout to check the running flag periodically
            let timeout = Duration::from_millis(100);
            
            match tokio::time::timeout(timeout, socket.recv(&mut buffer)).await {
                Ok(Ok(bytes_received)) => {
                    if bytes_received > 0 {
                        total_bytes += bytes_received as u64;
                        packet_count += 1;

                        // Send raw data to writer thread (like C's pipe/buffer)
                        let packet_data = buffer[..bytes_received].to_vec();
                        if let Err(_) = data_tx.try_send(packet_data) {
                            error!("Data channel full, dropping packet! This indicates the file writer can't keep up.");
                            error_count += 1;
                        }

                        // Log progress every 1000 packets
                        if packet_count % 1000 == 0 {
                            let elapsed = start_time.elapsed();
                            let rate_mbps = (total_bytes as f64 * 8.0) / (elapsed.as_secs_f64() * 1_000_000.0);
                            info!("📊 Received {} packets, {:.1} MB, {:.2} Mbps, {} errors", 
                                  packet_count, total_bytes as f64 / 1_000_000.0, rate_mbps, error_count);
                        }
                    }
                }
                Ok(Err(e)) => {
                    error!("UDP receive error: {}", e);
                    error_count += 1;
                }
                Err(_) => {
                    // Timeout - continue to check running flag
                    continue;
                }
            }
        }

        // Final statistics
        let elapsed = start_time.elapsed();
        let rate_mbps = (total_bytes as f64 * 8.0) / (elapsed.as_secs_f64() * 1_000_000.0);
        info!("📈 UDP receiver final stats:");
        info!("   Packets: {}", packet_count);
        info!("   Bytes: {:.1} MB", total_bytes as f64 / 1_000_000.0);
        info!("   Rate: {:.2} Mbps", rate_mbps);
        info!("   Errors: {}", error_count);
        info!("   Duration: {:.1}s", elapsed.as_secs_f64());

        Ok(())
    }

    /// Find the next available frame number by checking existing files
    fn find_next_frame_number(output_dir: &str) -> Result<u32, Box<dyn std::error::Error + Send + Sync>> {
        let mut max_frame = 0u32;

        if let Ok(entries) = std::fs::read_dir(output_dir) {
            for entry in entries.flatten() {
                if let Ok(metadata) = entry.metadata() {
                    if metadata.is_file() {
                        if let Some(filename) = entry.file_name().to_str() {
                            if filename.ends_with(".dsql") {
                                // Try to parse the frame number from the filename (e.g., "00000001.dsql")
                                if let Ok(frame_num) = u32::from_str_radix(&filename[..8], 16) {
                                    if frame_num >= max_frame {
                                        max_frame = frame_num + 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(max_frame)
    }

    /// File writer thread - supports both file saving and live output modes
    /// Uses count-based frame synchronization instead of signature-based detection
    fn file_writer_thread(
        output_dir: String,
        data_rx: Receiver<Vec<u8>>,
        running: Arc<AtomicBool>,
        save_files: bool,
        live_output: bool,
        decode_mode: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Create timestamped output directory if saving files
        if save_files {
            std::fs::create_dir_all(&output_dir)?;
            info!("📁 Created capture directory: {}", output_dir);
        }

        // Find the highest existing frame number to continue from where we left off
        let mut frame_counter = if save_files {
            let counter = Self::find_next_frame_number(&output_dir)?;
            if counter > 0 {
                info!("Found existing frame files, continuing from frame {}", counter);
            }
            counter
        } else {
            0  // Start from 0 if not saving files
        };

        let mut buffer = Vec::new();
        let mut total_hwords_processed = 0u64;
        let mut file_write_errors = 0u64;

        // Create frame synchronization engine
        let mut sync_engine = FrameSyncEngine::new();

        info!("📝 File writer thread started (save_files: {}, live_output: {}, decode_mode: {})", save_files, live_output, decode_mode);
        info!("🔧 Using count-based frame synchronization");

        while running.load(Ordering::SeqCst) || !data_rx.is_empty() {
            // Receive data with timeout
            match data_rx.recv_timeout(Duration::from_millis(100)) {
                Ok(mut packet_data) => {
                    // Append to continuous buffer
                    buffer.append(&mut packet_data);

                    // Process 12-byte chunks using count-based synchronization
                    while buffer.len() >= 12 {
                        // Extract 12-byte chunk
                        let chunk: [u8; 12] = buffer.drain(0..12)
                            .collect::<Vec<u8>>()
                            .try_into()
                            .expect("Chunk should be exactly 12 bytes");

                        total_hwords_processed += 1;

                        // Process HWORD through synchronization engine
                        if let Some(frame_data) = sync_engine.process_hword(&chunk) {
                            // Frame complete! Write it immediately
                            let hwords_in_frame = frame_data.len() / 12;

                            // Handle live output if enabled
                            if live_output {
                                Self::output_live_frame(&frame_data, frame_counter, hwords_in_frame, decode_mode)?;
                            }

                            // Handle file saving if enabled
                            if save_files {
                                let filename = format!("{}/{:08X}.dsql", output_dir, frame_counter);
                                match std::fs::write(&filename, &frame_data) {
                                    Ok(_) => {
                                        info!("✅ Completed frame file: {} ({} HWORDs, {:.1} KB)",
                                              filename, hwords_in_frame, frame_data.len() as f64 / 1024.0);
                                    }
                                    Err(e) => {
                                        error!("❌ Failed to write frame file {}: {}", filename, e);
                                        file_write_errors += 1;
                                    }
                                }
                            }

                            frame_counter += 1;
                        }
                    }
                }
                Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
                    // Timeout - continue to check running flag
                    continue;
                }
                Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                    info!("Data channel disconnected - UDP receiver has stopped");
                    break;
                }
            }
        }

        // Check if there's an incomplete frame in the sync engine
        let current_buffer = sync_engine.current_buffer();
        if !current_buffer.is_empty() {
            warn!("⚠️ Incomplete frame at shutdown: {} bytes", current_buffer.len());

            // Optionally write incomplete frame for debugging
            if save_files {
                let filename = format!("{}/{:08X}_incomplete.dsql", output_dir, frame_counter);
                match std::fs::write(&filename, current_buffer) {
                    Ok(_) => {
                        info!("💾 Saved incomplete frame: {} ({} bytes)",
                              filename, current_buffer.len());
                    }
                    Err(e) => {
                        error!("❌ Failed to write incomplete frame {}: {}", filename, e);
                    }
                }
            }
        }

        // Get sync engine statistics
        let (frames_completed, sync_errors, header_index_errors) = sync_engine.stats();

        info!("📊 File writer final stats:");
        info!("   Total HWORDs processed: {}", total_hwords_processed);
        info!("   Frames completed: {}", frames_completed);
        info!("   Frames written: {}", frame_counter);
        info!("   Sync errors: {}", sync_errors);
        info!("   Header index errors: {}", header_index_errors);
        info!("   File write errors: {}", file_write_errors);

        Ok(())
    }

    /// Output frame data to stdout for live processing
    fn output_live_frame(
        hword_buffer: &[u8],
        frame_counter: u32,
        hwords_in_frame: usize,
        decode_mode: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        use std::io::{self, Write};

        if decode_mode {
            // Decode the HWORD data and output JSON coordinates
            match Frame::from_bytes(frame_counter, hword_buffer) {
                Ok(frame) => {
                    // Extract coordinates with all fields
                    let whitelist = FieldWhitelist::all();
                    let coordinates = frame.pixels.extract_coordinates(&whitelist, 1); // No decimation

                    // Convert coordinates to arrays
                    let mut x_coords = Vec::new();
                    let mut y_coords = Vec::new();
                    let mut z_coords = Vec::new();
                    let mut intensities = Vec::new();

                    for point in &coordinates.points {
                        x_coords.push(point.x.unwrap_or(0.0));
                        y_coords.push(point.y.unwrap_or(0.0));
                        z_coords.push(point.z.unwrap_or(0.0));
                        intensities.push(point.intensity.unwrap_or(0));
                    }

                    // Create JSON output
                    let json_output = serde_json::json!({
                        "frame_number": frame_counter,
                        "num_points": coordinates.len(),
                        "x": x_coords,
                        "y": y_coords,
                        "z": z_coords,
                        "intensity": intensities
                    });

                    // Output JSON to stdout
                    println!("{}", json_output);
                    io::stdout().flush()?;
                }
                Err(e) => {
                    eprintln!("❌ Failed to decode frame {}: {}", frame_counter, e);
                }
            }
        } else {
            // Output raw binary frame data to stdout with frame boundaries
            // Format: [frame_size: u32 little-endian][frame_data: bytes]
            let frame_size = hword_buffer.len() as u32;

            // Write frame size (4 bytes, little-endian)
            let size_bytes = frame_size.to_le_bytes();
            io::stdout().write_all(&size_bytes)?;

            // Write frame data
            io::stdout().write_all(hword_buffer)?;

            // Flush to ensure data is sent immediately
            io::stdout().flush()?;
        }

        Ok(())
    }

    /// Debug data generator thread - generates synthetic LiDAR data
    fn debug_data_generator_thread(
        data_tx: Sender<Vec<u8>>,
        running: Arc<AtomicBool>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("🧪 Starting debug data generator");

        let mut frame_counter = 0u32;
        let frame_interval = Duration::from_millis(1000); // 1 FPS
        let mut last_frame_time = Instant::now();

        while running.load(Ordering::SeqCst) {
            let now = Instant::now();

            // Generate frame at specified interval
            if now.duration_since(last_frame_time) >= frame_interval {
                let synthetic_frame = Self::generate_synthetic_frame(frame_counter);

                // Send synthetic frame data
                if let Err(e) = data_tx.try_send(synthetic_frame) {
                    warn!("Failed to send synthetic frame data: {}", e);
                    break;
                }

                frame_counter += 1;
                last_frame_time = now;

                if frame_counter % 10 == 0 {
                    info!("🧪 Generated {} synthetic frames", frame_counter);
                }
            }

            // Small sleep to prevent busy waiting
            thread::sleep(Duration::from_millis(10));
        }

        info!("🧪 Debug data generator stopped after {} frames", frame_counter);
        Ok(())
    }

    /// Generate a synthetic LiDAR frame with realistic point cloud data
    fn generate_synthetic_frame(frame_number: u32) -> Vec<u8> {
        use std::f32::consts::PI;

        // Generate a synthetic point cloud that looks like LiDAR data
        let points_per_frame = 122000; // Fixed 122,000 points per frame
        let mut frame_data = Vec::with_capacity((points_per_frame + 2) * 12); // +2 for header HWORDs

        // Add frame header HWORDs (required for file writer to detect frame boundaries)
        // FirstHeader HWORD
        let first_header_data: u128 = (frame_number as u128) | ((points_per_frame as u128) << 32); // Frame number and pixel count
        let first_header_control = 2u8; // FirstHeader (010)
        let first_header_word = ((first_header_control as u128) << 93) | first_header_data;
        let mut first_header_hword = [0u8; 12];
        for j in 0..12 {
            first_header_hword[j] = ((first_header_word >> (88 - j * 8)) & 0xFF) as u8;
        }
        frame_data.extend_from_slice(&first_header_hword);

        // SubsequentHeader HWORD (optional but good for completeness)
        let subsequent_header_data: u128 = 0; // Additional header data if needed
        let subsequent_header_control = 3u8; // SubsequentHeader (011)
        let subsequent_header_word = ((subsequent_header_control as u128) << 93) | subsequent_header_data;
        let mut subsequent_header_hword = [0u8; 12];
        for j in 0..12 {
            subsequent_header_hword[j] = ((subsequent_header_word >> (88 - j * 8)) & 0xFF) as u8;
        }
        frame_data.extend_from_slice(&subsequent_header_hword);

        // Generate points in a realistic pattern
        for i in 0..points_per_frame {
            // Create a rotating pattern with some randomness
            let angle = (i as f32 * 0.01) + (frame_number as f32 * 0.1);
            let radius = 50.0 + 100.0 * (angle * 0.1).sin();
            let height = 20.0 * (angle * 0.05).cos();

            // Add some noise for realism
            let noise_x = (i as f32 * 0.123).sin() * 2.0;
            let noise_y = (i as f32 * 0.456).cos() * 2.0;
            let noise_z = (i as f32 * 0.789).sin() * 1.0;

            // Calculate coordinates
            let x = radius * angle.cos() + noise_x;
            let y = radius * angle.sin() + noise_y;
            let z = height + noise_z;

            // Convert to fixed-point format (matching the real LiDAR format)
            // X, Y: 19-bit signed (9.10 fixed point)
            // Z: 22-bit signed (12.10 fixed point)
            let x_fixed = ((x * 1024.0) as i32).clamp(-262144, 262143) as u32 & 0x7FFFF;
            let y_fixed = ((y * 1024.0) as i32).clamp(-262144, 262143) as u32 & 0x7FFFF;
            let z_fixed = ((z * 1024.0) as i32).clamp(-2097152, 2097151) as u32 & 0x3FFFFF;

            // Synthetic intensity (16 bits)
            let intensity = ((angle * 10.0).sin().abs() * 65535.0) as u16 & 0xFFFF;

            // Determine control bits: FirstPixel (100 = 4) for first point, SubsequentPixel (101 = 5) for others
            let control_bits = if i == 0 { 4u8 } else { 5u8 }; // FirstPixel for first HWORD, SubsequentPixel for others

            // Pack data according to the exact HWORD specification:
            // Bits 23:0   = X coordinate (19 bits)
            // Bits 47:24  = Y coordinate (19 bits)
            // Bits 71:48  = Z coordinate (22 bits)
            // Bits 87:72  = Intensity (16 bits)
            // Bit 90      = Over-range flag (0)
            // Bit 91      = HG/LG flag (0)
            // Bit 92      = Parity (calculated)
            // Bits 95:93  = Control bits

            let mut data_92bit: u128 = 0;
            data_92bit |= x_fixed as u128;                    // Bits 23:0
            data_92bit |= (y_fixed as u128) << 24;           // Bits 47:24
            data_92bit |= (z_fixed as u128) << 48;           // Bits 71:48
            data_92bit |= (intensity as u128) << 72;         // Bits 87:72
            // Bits 90-91 are 0 (over_range and gain flags)

            // Calculate parity for the entire 96-bit word
            let control_and_data = ((control_bits as u128) << 93) | data_92bit;
            let ones_count = control_and_data.count_ones();
            let parity_bit = if ones_count % 2 == 0 { 1u128 } else { 0u128 }; // Odd parity

            // Construct the full 96-bit word
            let word_96bit = ((control_bits as u128) << 93) | (parity_bit << 92) | data_92bit;

            // Convert to 12 bytes (big-endian as expected by the parser)
            let mut hword = [0u8; 12];
            for j in 0..12 {
                hword[j] = ((word_96bit >> (88 - j * 8)) & 0xFF) as u8;
            }

            frame_data.extend_from_slice(&hword);
        }

        info!("🧪 Generated synthetic frame {} with {} points ({} bytes)",
              frame_number, points_per_frame, frame_data.len());

        frame_data
    }
}
