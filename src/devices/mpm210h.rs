#![allow(unused)]

use std::io::{Read, Write};
use std::net::{TcpStream, SocketAddr};
use std::time::Duration;
use thiserror::Error;
use tracing::{info, warn, error};

#[derive(Error, Debug)]
pub enum MPM210HError {
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("Parse error: {0}")]
    ParseError(String),
    
    #[error("Device not connected")]
    NotConnected,
}

pub type Result<T> = std::result::Result<T, MPM210HError>;

pub struct MPM210H {
    connection: Option<TcpStream>,
    address: String,
    port: u16,
}

impl MPM210H {
    pub fn new(ip_address: &str, port: u16) -> Self {
        let address = format!("{}:{}", ip_address, port);
        info!("Initializing MPM210H with address: {}", address);
        MPM210H {
            connection: None,
            address: ip_address.to_string(),
            port,
        }
    }

    pub fn connect(&mut self) -> Result<String> {
        let socket_addr = format!("{}:{}", self.address, self.port);
        info!("Attempting to connect to MPM210H at {}", socket_addr);
        
        let socket_addr: SocketAddr = socket_addr.parse()
            .map_err(|e: std::net::AddrParseError| MPM210HError::ParseError(e.to_string()))?;
        
        let stream = TcpStream::connect_timeout(&socket_addr, Duration::from_secs(5))?;
        stream.set_read_timeout(Some(Duration::from_secs(5)))?;
        stream.set_write_timeout(Some(Duration::from_secs(5)))?;
        
        self.connection = Some(stream);
        
        // Return the device identification
        let id = self.query("*IDN?")?;
        info!("MPM210H connected successfully. IDN: {}", id);
        Ok(id)
    }
    
    pub fn is_connected(&self) -> bool {
        self.connection.is_some()
    }

    pub fn send_command(&mut self, command: &str) -> Result<()> {
        if let Some(stream) = &mut self.connection {
            let cmd = format!("{}\n", command);
            info!("Sending command to MPM210H: {}", command);
            stream.write_all(cmd.as_bytes())?;
            stream.flush()?;
            
            // MPM210H requires a small delay after each command
            std::thread::sleep(Duration::from_millis(10));
            Ok(())
        } else {
            error!("Attempted to send command but MPM210H is not connected");
            Err(MPM210HError::NotConnected)
        }
    }

    pub fn read_response(&mut self) -> Result<String> {
        if let Some(stream) = &mut self.connection {
            let mut buf = [0_u8; 1024];
            let mut result = String::new();
            
            // MPM210H responses can be large, need to read until terminator or timeout
            let n = stream.read(&mut buf)?;
            if n == 0 {
                return Err(MPM210HError::IoError(std::io::Error::new(
                    std::io::ErrorKind::ConnectionAborted,
                    "Connection closed by remote",
                )));
            }
            
            let response = String::from_utf8_lossy(&buf[..n]).trim().to_string();
            info!("Received response from MPM210H: {}", response);
            Ok(response)
        } else {
            error!("Attempted to read from MPM210H but device is not connected");
            Err(MPM210HError::NotConnected)
        }
    }

    pub fn query(&mut self, command: &str) -> Result<String> {
        self.send_command(command)?;
        self.read_response()
    }

    pub fn get_recognized_modules(&mut self) -> Result<String> {
        self.query("IDIS?")
    }

    pub fn perform_zeroing(&mut self) -> Result<()> {
        info!("Performing zeroing operation to remove electrical offsets");
        if !self.is_connected() {
            return Err(MPM210HError::NotConnected);
        }
        self.send_command("ZERO")?;
        info!("Zeroing command sent successfully");
        Ok(())
    }
    
    pub fn read_power(&mut self, module: u8) -> Result<String> {
        info!("Reading power from module {}", module);
        self.query(&format!("READ? {}", module))
    }
    
    /// Read the optical power from a specific module and port
    pub fn read_power_from_port(&mut self, module: u8, port: u8) -> Result<String> {
        if port < 1 || port > 4 {
            return Err(MPM210HError::ParseError(format!("Invalid port number: {}. Port must be between 1 and 4.", port)));
        }
        
        info!("Reading power from module {}, port {}", module, port);
        
        // The READ? command returns comma-separated values for all ports in the module
        let response = self.query(&format!("READ? {}", module))?;
        
        // Split response by commas and extract the port value
        let values: Vec<&str> = response.split(',').collect();
        
        // Port index is 0-based in the array, but 1-based in the command 
        let port_index = (port - 1) as usize;
        
        if port_index >= values.len() {
            return Err(MPM210HError::ParseError(format!(
                "Response doesn't contain enough values. Expected at least {} values, got {}",
                port_index + 1,
                values.len()
            )));
        }
        
        let power = values[port_index].trim().to_string();
        info!("Power at module {}, port {}: {}", module, port, power);
        
        Ok(power)
    }

    pub fn get_wavelength(&mut self) -> Result<String> {
        self.query("WAV?")
    }

    pub fn set_wavelength(&mut self, wavelength: u32) -> Result<()> {
        info!("Setting MPM210H wavelength to {} nm", wavelength);
        self.send_command(&format!("WAV {}", wavelength))
    }

    pub fn get_error(&mut self) -> Result<String> {
        let response = self.query("ERR?")?;
        info!("Queried MPM210H error queue: {}", response);
        Ok(response)
    }

    pub fn clear_error_queue(&mut self) -> Result<Vec<String>> {
        let mut errors = Vec::new();
        loop {
            let response = self.query("ERR?")?;
            info!("Clearing error queue entry from MPM210H: {}", response);
            if response.trim().starts_with("0") || response.to_lowercase().contains("no error") {
                break;
            }
            errors.push(response);
        }
        Ok(errors)
    }
    
    // Configure the MPM210H for a specific measurement mode
    pub fn set_measurement_mode(&mut self, mode: &str) -> Result<()> {
        self.send_command(&format!("WMOD {}", mode))
    }
    
    // Set the average time (integration time)
    pub fn set_average_time(&mut self, avg_ms: f64) -> Result<()> {
        self.send_command(&format!("AVG {}", avg_ms))
    }
    
    // Set measurement unit (dBm or mW)
    pub fn set_unit(&mut self, unit: u8) -> Result<()> {
        if unit > 1 {
            return Err(MPM210HError::ParseError("Unit must be 0 (dBm) or 1 (mW)".to_string()));
        }
        self.send_command(&format!("UNIT {}", unit))
    }
}