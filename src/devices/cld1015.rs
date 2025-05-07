#![allow(unused)]

use std::ffi::CString;
use std::io::{BufRead, BufReader, Write};
use std::time::Duration;
use visa_rs::prelude::*;
use tracing::{info, warn, error};

pub struct CLD1015 {
    device: Option<Instrument>,
    resource_string: String,
}

// Helper function to convert IO errors to VISA errors
fn io_to_vs_err(err: std::io::Error) -> visa_rs::Error {
    visa_rs::io_to_vs_err(err)
}

impl CLD1015 {
    pub fn new(resource_string: &str) -> Self {
        info!("Initializing CLD1015 with resource string: {}", resource_string);
        CLD1015 {
            device: None,
            resource_string: resource_string.to_string(),
        }
    }

    pub fn connect(&mut self) -> visa_rs::Result<String> {
        info!("Attempting to connect to CLD1015 at {}", self.resource_string);
        let rm = DefaultRM::new()?;
        let resource = CString::new(self.resource_string.clone()).unwrap();
        let device = rm.open(
            &resource.into(),
            AccessMode::NO_LOCK,
            Duration::from_secs(2),
        )?;
        self.device = Some(device);
        
        // Identify the device
        let id = self.query("*IDN?")?;
        info!("CLD1015 connected successfully. IDN: {}", id);
        Ok(id)
    }

    pub fn is_connected(&self) -> bool {
        self.device.is_some()
    }

    pub fn write(&mut self, command: &str) -> visa_rs::Result<()> {
        if let Some(device) = &mut self.device {
            let command_with_newline = format!("{}\n", command);
            info!("Sending command to CLD1015: {}", command);
            device.write_all(command_with_newline.as_bytes()).map_err(io_to_vs_err)?;
            Ok(())
        } else {
            error!("Attempted to write to CLD1015 but device is not connected");
            Err(visa_rs::io_to_vs_err(std::io::Error::new(
                std::io::ErrorKind::NotConnected,
                "Device not connected",
            )))
        }
    }

    pub fn read(&mut self) -> visa_rs::Result<String> {
        if let Some(device) = &mut self.device {
            let mut response = String::new();
            let bytes_read = BufReader::new(device).read_line(&mut response).map_err(io_to_vs_err)?;
            let trimmed = response.trim().to_string();
            info!("Received response from CLD1015: {}", trimmed);
            Ok(trimmed)
        } else {
            error!("Attempted to read from CLD1015 but device is not connected");
            Err(visa_rs::io_to_vs_err(std::io::Error::new(
                std::io::ErrorKind::NotConnected,
                "Device not connected",
            )))
        }
    }

    pub fn query(&mut self, command: &str) -> visa_rs::Result<String> {
        self.write(command)?;
        // Add a small delay to ensure command is processed
        std::thread::sleep(Duration::from_millis(50));
        self.read()
    }

    pub fn enable_tec(&mut self) -> visa_rs::Result<()> {
        info!("Enabling TEC");
        self.write("OUTPut2:STATe ON")
    }
    
    pub fn get_tec_state(&mut self) -> visa_rs::Result<bool> {
        let response = self.query("OUTPut2:STATe?")?;
        Ok(response.eq_ignore_ascii_case("ON") || response == "1")
    }

    pub fn set_current_mode(&mut self) -> visa_rs::Result<()> {
        self.write("SOURce:FUNCtion:MODE CURRent")
    }
    
    pub fn set_current(&mut self, current_amps: f64) -> visa_rs::Result<()> {
        const MAX_SAFE_CURRENT_AMPS: f64 = 1.5;
        if current_amps > MAX_SAFE_CURRENT_AMPS {
            warn!("Attempted to set current above safe limit: {} A", current_amps);
            return Err(visa_rs::io_to_vs_err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("Requested current {} A exceeds the 1.5 A safety limit", current_amps),
            )));
        }
        info!("Setting current to {:.3} A", current_amps);
        self.write(&format!("SOURce:CURRent:LEVel:IMMediate:AMPLitude {}", current_amps))
    }

    pub fn get_current(&mut self) -> visa_rs::Result<f64> {
        let response = self.query("SOURce:CURRent:LEVel:IMMediate:AMPLitude?")?;
        info!("Queried current: {} A", response);
        response.parse::<f64>().map_err(|_| visa_rs::io_to_vs_err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Failed to parse current value",
        )))
    }

    pub fn set_laser_output(&mut self, enabled: bool) -> visa_rs::Result<()> {
        if enabled {
            // Safety check: ensure TEC is ON before enabling laser
            let tec_on = self.get_tec_state()?;
            if !tec_on {
                error!("Attempt to enable laser while TEC is OFF");
                return Err(visa_rs::io_to_vs_err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Cannot enable laser: TEC is OFF",
                )));
            }
            info!("Enabling laser output");
        } else {
            info!("Disabling laser output");
        }
    
        let state = if enabled { "ON" } else { "OFF" };
        self.write(&format!("OUTPut:STATe {}", state))
    }

    pub fn get_laser_output(&mut self) -> visa_rs::Result<bool> {
        let response = self.query("OUTPut:STATe?")?;
        Ok(response.eq_ignore_ascii_case("ON") || response == "1")
    }

    pub fn get_error(&mut self) -> visa_rs::Result<String> {
        let response = self.query("SYST:ERR?")?;
        info!("Queried CLD1015 error queue: {}", response);
        Ok(response)
    }

    pub fn clear_error_queue(&mut self) -> visa_rs::Result<Vec<String>> {
        let mut errors = Vec::new();
        loop {
            let response = self.query("SYST:ERR?")?;
            info!("Clearing error queue entry: {}", &response);
            if response.starts_with("0") {
                break;
            }
            errors.push(response);
        }
        Ok(errors)
    }    
    
    pub fn reset(&mut self) -> visa_rs::Result<()> {
        info!("Resetting CLD1015 to default state");
        
        // First, ensure device is connected
        if !self.is_connected() {
            error!("Cannot reset CLD1015: device not connected");
            return Err(visa_rs::io_to_vs_err(std::io::Error::new(
                std::io::ErrorKind::NotConnected,
                "Device not connected",
            )));
        }
        
        // Send the IEEE 488.2 *RST command to reset the device to defaults
        self.write("*RST")?;
        
        // Allow time for reset to complete
        std::thread::sleep(Duration::from_millis(500));
        
        // Verify reset was successful by checking device status
        let status = self.query("*OPC?")?;
        if status != "1" {
            return Err(visa_rs::io_to_vs_err(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Reset operation failed, unexpected response: {}", status),
            )));
        }
        
        // Clear error queue to ensure we're starting with a clean slate
        let _ = self.clear_error_queue(); // Ignore any errors here
        
        info!("CLD1015 reset completed successfully");
        Ok(())
    }
}