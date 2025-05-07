// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![allow(unused)]

mod devices;
mod experiment;

use std::sync::Mutex;
use tracing_subscriber::fmt;
use tracing_appender::rolling;
use tracing::{info, error, warn, Level};
use devices::{CLD1015, MPM210H};
use visa_rs::DefaultRM;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Set up logging
    setup_logging();
    info!("Starting application");

    // Initialize VISA Resource Manager
    let rm = match DefaultRM::new() {
        Ok(rm) => {
            info!("Successfully initialized VISA resource manager");
            rm
        },
        Err(e) => {
            error!("Failed to initialize VISA resource manager: {}", e);
            return Err(Box::new(e));
        }
    };

    // Initialize devices
    let mut cld = CLD1015::new("USB0::4883::32847::M01053290::0::INSTR");
    let mut mpm = MPM210H::new("192.168.1.161", 5000);

    // Run the experiment - specifically using module 0, port 2
    // Create a custom configuration
    let config = experiment::CurrentSweepConfig {
        module: 0,               // Module 0
        port: 2,                 // Port 2 (specifically requested)
        start_ma: 10.0,          // Start at 10 mA
        stop_ma: 100.0,          // End at 100 mA
        step_ma: 5.0,            // 5 mA steps
        stabilization_delay_ms: 50, // 50ms stabilization delay
        wavelength_nm: 980,      // 980nm wavelength
        averaging_time_ms: 100.0, // 100ms averaging time
        power_unit: experiment::PowerUnit::DBm, // Use dBm units
    };
    
    // Run the experiment with our custom config that specifies module 0, port 2
    match experiment::run_current_sweep(&mut cld, &mut mpm, config) {
        Ok(path) => {
            info!("Experiment completed successfully. Results saved to: {}", path.display());
            println!("Experiment completed successfully. Results saved to: {}", path.display());
        },
        Err(e) => {
            error!("Experiment failed: {}", e);
            eprintln!("Experiment failed: {}", e);
            return Err(Box::new(std::io::Error::new(std::io::ErrorKind::Other, e)));
        }
    }

    info!("Application shutting down");
    Ok(())
}

fn setup_logging() {
    // Set up file-based logging with rotation
    let file_appender = rolling::daily("logs", "app.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
    
    // Create a subscriber that logs to both the file and the console
    fmt()
        .with_writer(non_blocking)
        .with_ansi(false) // Disable ANSI colors in log files
        .with_level(true)
        .init();
}