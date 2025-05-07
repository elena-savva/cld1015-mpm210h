pub mod data;

use crate::devices::{CLD1015, MPM210H};
use data::MeasurementRecord;
use chrono::Utc;
use std::fs::File;
use std::io;
use std::path::PathBuf;
use csv::Writer;
use tracing::{info, error, warn};

/// Configuration for a current sweep experiment
#[derive(Debug)]
pub struct CurrentSweepConfig {
    pub module: u8,                  // MPM210H module number to use
    pub port: u8,                    // MPM210H port number to use (1-4)
    pub start_ma: f64,               // Start current in mA
    pub stop_ma: f64,                // End current in mA
    pub step_ma: f64,                // Step size in mA
    pub stabilization_delay_ms: u64, // Delay after setting current before measuring
    pub wavelength_nm: u32,          // Wavelength in nm
    pub averaging_time_ms: f64,      // Power meter averaging time in ms
    pub power_unit: PowerUnit,       // Power measurement unit
}

/// Power measurement unit
#[derive(Debug)]
pub enum PowerUnit {
    DBm,
    MilliWatt,
}

impl Default for CurrentSweepConfig {
    fn default() -> Self {
        Self {
            module: 0,
            port: 2,   // Default to port 2 as requested
            start_ma: 10.0,
            stop_ma: 100.0,
            step_ma: 5.0,
            stabilization_delay_ms: 50,
            wavelength_nm: 980,
            averaging_time_ms: 100.0,
            power_unit: PowerUnit::DBm,
        }
    }
}

/// Run a current sweep with custom configuration
pub fn run_current_sweep(
    cld: &mut CLD1015,
    mpm: &mut MPM210H,
    config: CurrentSweepConfig,
) -> Result<PathBuf, String> {
    info!("Starting current sweep with configuration: {:?}", config);
    
    // Connect to devices and run experiment
    _run_current_sweep_internal(cld, mpm, config)
}

/// Run a basic current sweep with default parameters
pub fn run_basic_current_sweep(
    cld: &mut CLD1015,
    mpm: &mut MPM210H,
) -> Result<PathBuf, String> {
    // Use default configuration
    let config = CurrentSweepConfig::default();
    
    // Run with default parameters
    info!("Starting basic current sweep with default parameters");
    _run_current_sweep_internal(cld, mpm, config)
}

/// Internal implementation of current sweep
fn _run_current_sweep_internal(
    cld: &mut CLD1015,
    mpm: &mut MPM210H,
    config: CurrentSweepConfig,
) -> Result<PathBuf, String> {
    // Extract configuration parameters
    let module = config.module;
    let port = config.port;
    let start_ma = config.start_ma;
    let stop_ma = config.stop_ma;
    let step_ma = config.step_ma;
    let stabilization_delay_ms = config.stabilization_delay_ms;

    // Connect to devices
    info!("Connecting to devices");
    match cld.connect() {
        Ok(id) => info!("CLD1015 connected: {}", id),
        Err(e) => return Err(format!("Failed to connect to CLD1015: {}", e)),
    }

    match mpm.connect() {
        Ok(id) => info!("MPM210H connected: {}", id),
        Err(e) => return Err(format!("Failed to connect to MPM210H: {}", e)),
    }

    // Reset CLD1015 to ensure clean state before starting experiment
    info!("Resetting CLD1015 before starting experiment");
    match cld.reset() {
        Ok(_) => info!("CLD1015 reset completed successfully"),
        Err(e) => {
            warn!("Failed to reset CLD1015: {}", e);
            warn!("Continuing with experiment, but some settings may not be at default values");
        }
    }

    // Safety check: ensure laser is off after reset
    match cld.get_laser_output() {
        Ok(true) => {
            warn!("Laser output is still ON after reset, turning it OFF for safety");
            if let Err(e) = cld.set_laser_output(false) {
                return Err(format!("Failed to turn laser off after reset: {}", e));
            }
        },
        Ok(false) => info!("Confirmed laser is OFF after reset"),
        Err(e) => {
            warn!("Could not verify laser state after reset: {}", e);
            // Try to turn it off anyway as a precaution
            let _ = cld.set_laser_output(false);
        }
    }

    // Validate parameters
    if step_ma <= 0.0 || start_ma > stop_ma {
        return Err("Invalid sweep parameters".into());
    }

    // Safety: Ensure TEC is active
    let tec_on = match cld.get_tec_state() {
        Ok(state) => state,
        Err(e) => return Err(format!("Failed to get TEC state: {}", e)),
    };

    if !tec_on {
        info!("TEC is off, enabling it");
        match cld.enable_tec() {
            Ok(_) => {
                info!("TEC enabled successfully, waiting for stabilization");
                // Wait for TEC to stabilize
                std::thread::sleep(std::time::Duration::from_secs(5));
            },
            Err(e) => return Err(format!("Failed to enable TEC: {}", e)),
        }
    }

    // Perform zeroing before starting the sweep to ensure accurate measurements
    info!("Performing zeroing operation before sweep to remove electrical offsets");
    match mpm.perform_zeroing() {
        Ok(_) => info!("Zeroing command sent successfully"),
        Err(e) => {
            error!("Failed to perform zeroing: {}", e);
            return Err(format!("Failed to perform zeroing: {}", e));
        }
    }

    // Give time for the zeroing operation to complete (3 seconds as per documentation)
    std::thread::sleep(std::time::Duration::from_secs(3));
    info!("Zeroing completed, proceeding with sweep");

    // Set current mode
    if let Err(e) = cld.set_current_mode() {
        return Err(format!("Failed to set current mode: {}", e));
    }

    // Turn laser off at the beginning
    if let Err(e) = cld.set_laser_output(false) {
        warn!("Failed to disable laser output: {}", e);
    }

    // Configure the MPM210H
    // Set measurement mode to CONST1 (fixed wavelength, manual range)
    if let Err(e) = mpm.set_measurement_mode("CONST1") {
        return Err(format!("Failed to set MPM210H measurement mode: {}", e));
    }
    
    // Set average time
    if let Err(e) = mpm.set_average_time(config.averaging_time_ms) {
        return Err(format!("Failed to set MPM210H averaging time: {}", e));
    }
    
    // Set power unit
    let unit_value = match config.power_unit {
        PowerUnit::DBm => 0,
        PowerUnit::MilliWatt => 1,
    };
    if let Err(e) = mpm.set_unit(unit_value) {
        return Err(format!("Failed to set MPM210H measurement unit: {}", e));
    }

    // Ensure mpm210h is at the correct wavelength for the laser
    if let Err(e) = mpm.set_wavelength(config.wavelength_nm) {
        return Err(format!("Failed to set MPM210H wavelength: {}", e));
    }

    // Turn laser on
    if let Err(e) = cld.set_laser_output(true) {
        return Err(format!("Failed to enable laser output: {}", e));
    }

    info!("Starting current sweep: {} mA to {} mA, step {} mA, module {}, port {}", 
          start_ma, stop_ma, step_ma, module, port);

    let mut records = Vec::new();
    let mut current_ma = start_ma;

    while current_ma <= stop_ma {
        // Set the current
        match cld.set_current(current_ma / 1000.0) {  // convert to A
            Ok(_) => {},
            Err(e) => {
                // Turn off the laser before returning error
                let _ = cld.set_laser_output(false);
                return Err(format!("Failed to set current to {} mA: {}", current_ma, e));
            }
        }

        // Wait for stabilization
        std::thread::sleep(std::time::Duration::from_millis(stabilization_delay_ms));

        // Read power from the specific module and port
        let power = match mpm.read_power_from_port(module, port) {
            Ok(p) => p,
            Err(e) => {
                // Turn off the laser before returning error
                let _ = cld.set_laser_output(false);
                return Err(format!("Failed to read power at {} mA from module {}, port {}: {}", 
                                 current_ma, module, port, e));
            }
        };

        let now = Utc::now().to_rfc3339();

        // Create measurement record
        let record = MeasurementRecord {
            timestamp: now.clone(),
            current_ma,
            power_dbm: power.clone(),
            module,
        };

        // Print the current measurement to console
        println!("Current: {:.2} mA, Power: {} dBm", current_ma, power);

        records.push(record);
        current_ma += step_ma;
    }

    // Turn laser off after sweep
    if let Err(e) = cld.set_laser_output(false) {
        warn!("Failed to disable laser output after sweep: {}", e);
    }

    // Save the results
    let path = match save_measurements_to_csv(&records) {
        Ok(p) => p,
        Err(e) => return Err(format!("Failed to save CSV: {}", e)),
    };

    info!("Sweep completed. Data saved to: {:?}", path);

    Ok(path)
}

/// Save the measurement records to a timestamped CSV file
fn save_measurements_to_csv(data: &[MeasurementRecord]) -> io::Result<PathBuf> {
    let timestamp = chrono::Local::now()
        .format("experiment_data_%Y-%m-%d_%H-%M-%S.csv")
        .to_string();

    let mut path = std::env::current_dir()?;
    path.push("logs");
    std::fs::create_dir_all(&path)?;
    path.push(timestamp);

    let file = File::create(&path)?;
    let mut writer = Writer::from_writer(file);
    for record in data {
        writer.serialize(record)?;
    }
    writer.flush()?;

    info!("Measurements saved to {}", path.display());
    Ok(path)
}