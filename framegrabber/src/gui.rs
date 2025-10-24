/*!
GUI implementation for the frame grabber application.
*/

use crate::config::AppConfig;
use crate::capture::SimpleFrameGrabber;
use eframe::egui;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use tracing::{info, error};

/// Main GUI application state
pub struct FrameGrabberGui {
    config: AppConfig,
    config_path: PathBuf,
    
    // UI state
    capture_running: bool,
    capture_thread: Option<thread::JoinHandle<()>>,
    running_flag: Option<Arc<AtomicBool>>,
    
    // Status
    status_message: String,
    packet_count: u64,
    frame_count: u64,
    
    // Temporary UI values
    temp_bind_addr: String,
    temp_port: String,
    temp_output_dir: String,
}

impl FrameGrabberGui {
    /// Create a new GUI instance
    pub fn new(config: AppConfig, config_path: PathBuf, _cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            temp_bind_addr: config.framegrabber.udp_bind_addr.clone(),
            temp_port: config.framegrabber.udp_port.to_string(),
            temp_output_dir: config.framegrabber.output_directory.clone(),
            config,
            config_path,
            capture_running: false,
            capture_thread: None,
            running_flag: None,
            status_message: "Ready".to_string(),
            packet_count: 0,
            frame_count: 0,
        }
    }
    
    /// Start frame capture
    fn start_capture(&mut self) {
        if self.capture_running {
            return;
        }
        
        // Update config from UI
        self.config.framegrabber.udp_bind_addr = self.temp_bind_addr.clone();
        if let Ok(port) = self.temp_port.parse::<u16>() {
            self.config.framegrabber.udp_port = port;
        }
        self.config.framegrabber.output_directory = self.temp_output_dir.clone();
        
        let mut grabber = SimpleFrameGrabber::new(
            self.config.framegrabber.udp_bind_addr.clone(),
            self.config.framegrabber.udp_port,
            self.config.framegrabber.output_directory.clone(),
            self.config.framegrabber.enable_storage,
            self.config.framegrabber.enable_realtime_processing,
            false, // debug_mode - not supported in GUI yet
            false, // decode_mode - not supported in GUI yet
        );
        
        let running_flag = grabber.get_running_flag();
        self.running_flag = Some(Arc::clone(&running_flag));
        
        // Start capture in background thread
        let handle = thread::spawn(move || {
            match grabber.start() {
                Ok(_) => {
                    info!("Frame capture completed successfully");
                }
                Err(e) => {
                    error!("Frame capture failed: {}", e);
                }
            }
        });
        
        self.capture_thread = Some(handle);
        self.capture_running = true;
        self.status_message = "Capture started".to_string();
    }
    
    /// Stop frame capture
    fn stop_capture(&mut self) {
        if !self.capture_running {
            return;
        }
        
        // Signal the capture to stop
        if let Some(running_flag) = &self.running_flag {
            running_flag.store(false, Ordering::SeqCst);
        }
        
        // Wait for thread to finish (with timeout)
        if let Some(handle) = self.capture_thread.take() {
            // In a real implementation, you might want to handle this more gracefully
            let _ = handle.join();
        }
        
        self.capture_running = false;
        self.running_flag = None;
        self.status_message = "Capture stopped".to_string();
    }
}

impl eframe::App for FrameGrabberGui {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Check if capture thread has finished
        if self.capture_running {
            if let Some(handle) = &self.capture_thread {
                if handle.is_finished() {
                    self.capture_running = false;
                    self.capture_thread = None;
                    self.running_flag = None;
                    self.status_message = "Capture finished".to_string();
                }
            }
        }
        
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("🚀 Universal Instrument Control - Frame Grabber");
            ui.separator();
            
            // Configuration section
            ui.group(|ui| {
                ui.label("📡 Network Configuration");
                
                ui.horizontal(|ui| {
                    ui.label("Bind Address:");
                    ui.text_edit_singleline(&mut self.temp_bind_addr);
                });
                
                ui.horizontal(|ui| {
                    ui.label("Port:");
                    ui.text_edit_singleline(&mut self.temp_port);
                });
            });
            
            ui.separator();
            
            // Storage configuration
            ui.group(|ui| {
                ui.label("💾 Storage Configuration");
                
                ui.horizontal(|ui| {
                    ui.label("Output Directory:");
                    ui.text_edit_singleline(&mut self.temp_output_dir);
                    if ui.button("📁 Browse").clicked() {
                        // In a real implementation, you'd open a file dialog
                        self.status_message = "File dialog not implemented yet".to_string();
                    }
                });
                
                ui.checkbox(&mut self.config.framegrabber.enable_storage, "Enable file storage");
                ui.checkbox(&mut self.config.framegrabber.enable_realtime_processing, "Enable live output");
            });
            
            ui.separator();
            
            // Control buttons
            ui.horizontal(|ui| {
                if self.capture_running {
                    if ui.button("🛑 Stop Capture").clicked() {
                        self.stop_capture();
                    }
                } else {
                    if ui.button("▶️ Start Capture").clicked() {
                        self.start_capture();
                    }
                }
                
                if ui.button("💾 Save Config").clicked() {
                    match self.config.save_to_file(&self.config_path) {
                        Ok(_) => {
                            self.status_message = "Configuration saved".to_string();
                        }
                        Err(e) => {
                            self.status_message = format!("Failed to save config: {}", e);
                        }
                    }
                }
                
                if ui.button("🔄 Load Config").clicked() {
                    match AppConfig::load_from_file(&self.config_path) {
                        Ok(config) => {
                            self.config = config;
                            self.temp_bind_addr = self.config.framegrabber.udp_bind_addr.clone();
                            self.temp_port = self.config.framegrabber.udp_port.to_string();
                            self.temp_output_dir = self.config.framegrabber.output_directory.clone();
                            self.status_message = "Configuration loaded".to_string();
                        }
                        Err(e) => {
                            self.status_message = format!("Failed to load config: {}", e);
                        }
                    }
                }
            });
            
            ui.separator();
            
            // Status section
            ui.group(|ui| {
                ui.label("📊 Status");
                
                ui.horizontal(|ui| {
                    ui.label("Status:");
                    if self.capture_running {
                        ui.colored_label(egui::Color32::GREEN, "🟢 Running");
                    } else {
                        ui.colored_label(egui::Color32::RED, "🔴 Stopped");
                    }
                });
                
                ui.horizontal(|ui| {
                    ui.label("Message:");
                    ui.label(&self.status_message);
                });
                
                ui.horizontal(|ui| {
                    ui.label("Packets:");
                    ui.label(format!("{}", self.packet_count));
                });
                
                ui.horizontal(|ui| {
                    ui.label("Frames:");
                    ui.label(format!("{}", self.frame_count));
                });
            });
            
            ui.separator();
            
            // Advanced options (collapsible)
            ui.collapsing("🔧 Advanced Options", |ui| {
                ui.horizontal(|ui| {
                    ui.label("Channel Buffer Size:");
                    ui.add(egui::DragValue::new(&mut self.config.framegrabber.channel_buffer_size).range(100..=10000));
                });
                
                ui.horizontal(|ui| {
                    ui.label("Max Frame Size (HWORDs):");
                    ui.add(egui::DragValue::new(&mut self.config.framegrabber.max_frame_size_hwords).range(1000..=10000000));
                });
                
                ui.checkbox(&mut self.config.framegrabber.drop_parity_errors, "Drop packets with parity errors");
                
                ui.horizontal(|ui| {
                    ui.label("Stats Interval (seconds):");
                    ui.add(egui::DragValue::new(&mut self.config.framegrabber.stats_interval_seconds).range(1..=300));
                });
            });
        });
        
        // Request repaint for real-time updates
        if self.capture_running {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }
    
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        // Stop capture if running
        if self.capture_running {
            self.stop_capture();
        }
        
        // Auto-save configuration if enabled
        if self.config.gui.auto_save_config {
            let _ = self.config.save_to_file(&self.config_path);
        }
    }
}
