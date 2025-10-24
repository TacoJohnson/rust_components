/*!
# Frame Grabber Application

High-performance UDP frame capture application that receives frame data
and saves it as .dsql files for processing by the Python visualization system.

## Features

- UDP packet capture with configurable bind address and port
- Real-time HWORD parsing and frame boundary detection
- Efficient file writing with automatic frame numbering
- Live output mode for real-time processing
- GUI configuration interface
- Command-line interface for headless operation

## Usage

### GUI Mode (default)
```bash
framegrabber
```

### Command Line Mode
```bash
framegrabber --cli --port 12345 --output-dir ./frames --bind-addr 0.0.0.0
```

### Live Output Mode (no file saving)
```bash
framegrabber --cli --live --port 12345
```
*/

use std::path::PathBuf;
use tracing_subscriber;
use clap::{Parser, Subcommand};

mod config;
mod gui;
mod capture;

use config::AppConfig;
use gui::FrameGrabberGui;
use capture::SimpleFrameGrabber;

#[derive(Parser)]
#[command(name = "framegrabber")]
#[command(about = "High-performance UDP frame capture and DSQL file generation")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
    
    /// Configuration file path
    #[arg(short, long, default_value = "framegrabber.toml")]
    config: PathBuf,
    
    /// Run in command-line mode (no GUI)
    #[arg(long)]
    cli: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Start frame capture
    Capture {
        /// UDP bind address
        #[arg(short, long, default_value = "0.0.0.0")]
        bind_addr: String,
        
        /// UDP port to listen on
        #[arg(short, long, default_value = "12345")]
        port: u16,
        
        /// Output directory for .dsql files
        #[arg(short, long, default_value = "./frames")]
        output_dir: String,
        
        /// Enable live output mode (no file saving)
        #[arg(long)]
        live: bool,

        /// Enable debug mode (generate synthetic data)
        #[arg(long)]
        debug: bool,

        /// Enable live decoding (output decoded coordinates instead of raw HWORD data)
        #[arg(long)]
        decode: bool,
    },
    
    /// Generate configuration file
    Config {
        /// Output path for configuration file
        #[arg(short, long, default_value = "framegrabber.toml")]
        output: PathBuf,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // Check if we're in live mode - if so, disable logging completely
    let is_live_mode = matches!(cli.command, Some(Commands::Capture { live: true, .. }));

    if !is_live_mode {
        // Initialize logging to stderr to keep stdout clean for binary data
        tracing_subscriber::fmt()
            .with_writer(std::io::stderr)
            .init();
    }

    
    match cli.command {
        Some(Commands::Capture { bind_addr, port, output_dir, live, debug, decode }) => {
            // In live mode, suppress all stdout output to keep binary stream clean
            if live {
                // Disable all logging to stdout when in live mode
                println!("🚀 Starting frame capture (DEBUG mode)");
                if debug {
                    println!("🧪 Generating synthetic LiDAR data");
                }
                println!("📺 Live output mode (no file saving)");
            }

            // Command-line capture mode
            run_capture_cli(bind_addr, port, output_dir, !live, live, debug, decode)
        }
        
        Some(Commands::Config { output }) => {
            // Generate configuration file
            generate_config_file(output)
        }
        
        None => {
            if cli.cli {
                // CLI mode with config file
                run_capture_from_config(cli.config)
            } else {
                // GUI mode
                run_gui(cli.config)
            }
        }
    }
}

/// Run frame capture in CLI mode with explicit parameters
fn run_capture_cli(
    bind_addr: String,
    port: u16,
    output_dir: String,
    save_files: bool,
    live_output: bool,
    debug_mode: bool,
    decode_mode: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if debug_mode {
        println!("🚀 Starting frame capture (DEBUG mode)");
        println!("🧪 Generating synthetic LiDAR data");
    } else {
        println!("🚀 Starting frame capture (CLI mode)");
        println!("📡 Bind address: {}", bind_addr);
        println!("🔌 Port: {}", port);
    }

    if save_files {
        println!("💾 Output directory: {}", output_dir);
    } else {
        println!("📺 Live output mode (no file saving)");
    }

    let mut grabber = SimpleFrameGrabber::new(
        bind_addr,
        port,
        output_dir,
        save_files,
        live_output,
        debug_mode,
        decode_mode,
    );
    
    // Set up Ctrl+C handler
    let running = grabber.get_running_flag();
    ctrlc::set_handler(move || {
        println!("\n🛑 Received Ctrl+C, shutting down gracefully...");
        running.store(false, std::sync::atomic::Ordering::SeqCst);
    })?;
    
    if let Err(e) = grabber.start() {
        eprintln!("Failed to start frame capture: {}", e);
        return Err(format!("Frame capture failed: {}", e).into());
    }
    
    println!("✅ Frame capture completed");
    Ok(())
}

/// Run frame capture from configuration file
fn run_capture_from_config(config_path: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let config = AppConfig::load_from_file(&config_path)?;
    
    println!("🚀 Starting frame capture from config: {}", config_path.display());
    
    let mut grabber = SimpleFrameGrabber::new(
        config.framegrabber.udp_bind_addr,
        config.framegrabber.udp_port,
        config.framegrabber.output_directory,
        config.framegrabber.enable_storage,
        !config.framegrabber.enable_storage, // live_output is inverse of storage
        false, // debug_mode - not supported in config yet
        false, // decode_mode - not supported in config yet
    );
    
    // Set up Ctrl+C handler
    let running = grabber.get_running_flag();
    ctrlc::set_handler(move || {
        println!("\n🛑 Received Ctrl+C, shutting down gracefully...");
        running.store(false, std::sync::atomic::Ordering::SeqCst);
    })?;
    
    if let Err(e) = grabber.start() {
        eprintln!("Failed to start frame capture: {}", e);
        return Err(format!("Frame capture failed: {}", e).into());
    }
    
    println!("✅ Frame capture completed");
    Ok(())
}

/// Run the GUI application
fn run_gui(config_path: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    println!("🖥️ Starting Frame Grabber GUI");
    
    let config = AppConfig::load_from_file(&config_path).unwrap_or_else(|_| {
        eprintln!("⚠️ Failed to load config, using defaults");
        AppConfig::new()
    });
    
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_title("Universal Instrument Control - Frame Grabber"),
        ..Default::default()
    };
    
    eframe::run_native(
        "Frame Grabber",
        options,
        Box::new(|cc| {
            Ok(Box::new(FrameGrabberGui::new(config, config_path, cc)))
        })
    )?;
    
    Ok(())
}

/// Generate a default configuration file
fn generate_config_file(output_path: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let config = AppConfig::new();
    config.save_to_file(&output_path)?;
    
    println!("✅ Generated configuration file: {}", output_path.display());
    println!("📝 Edit the file to customize settings, then run:");
    println!("   framegrabber --config {}", output_path.display());
    
    Ok(())
}
