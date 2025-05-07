# Optical Lab Automation

This application provides a streamlined way to run optical power measurements using a Thorlabs CLD1015 laser diode controller and a Santec MPM-210H optical power meter.

## Features

- Automatic connection to CLD1015 via VISA and MPM-210H via TCP/IP
- Automated current sweep with real-time optical power measurements
- Comprehensive logging for traceability and diagnostics
- CSV output with timestamped data

## Prerequisites

- Rust (latest stable)
- VISA drivers installed for the CLD1015
- Network connectivity to the MPM-210H power meter
- Connected and properly configured optical equipment

## Building and Running

1. Clone the repository
2. Navigate to the project directory
3. Build the application:
   ```bash
   cargo build --release
   ```
4. Run the application:
   ```bash
   cargo run --release
   ```

## Configuration

The application uses hardcoded values for the experiment parameters:

- CLD1015 connection: `USB0::4883::32847::M01053290::0::INSTR`
- MPM-210H connection: `192.168.1.161:5000`
- Sweep parameters: 10mA to 100mA in 5mA steps

To modify these parameters, edit the `src-tauri/src/experiment/mod.rs` file and rebuild the application.

## Output

The application saves measurement data in CSV format under the `logs` directory. Each file is named with a timestamp for easy identification. The CSV contains the following columns:

- `timestamp`: ISO format timestamp
- `current_mA`: Laser current in milliamperes
- `power_dBm`: Measured optical power in dBm
- `module`: MPM-210H module/port number used for the measurement

## Safety Features

The application includes several safety features:

- TEC verification before enabling the laser
- Current limiting (max 1.5A)
- Automatic zeroing of the power meter before measurements
- Proper laser shutdown after measurements or in case of errors
- Comprehensive logging for troubleshooting

## Architecture

The application is organized into the following modules:

- `devices/`: Hardware interface implementations
  - `cld1015.rs`: Thorlabs CLD1015 laser diode controller driver
  - `mpm210h.rs`: Santec MPM-210H optical power meter driver
- `experiment/`: Measurement logic
  - `data.rs`: Data structures for measurements
  - `mod.rs`: Experiment execution logic

The application uses the visa-rs library for VISA communication with the CLD1015 and standard TCP/IP sockets for communicating with the MPM-210H.

## Customizing Experiments

To customize the experiment parameters, modify the following in `src-tauri/src/experiment/mod.rs`:

```rust
// Hardcoded experiment parameters
let module = 0;  // First module
let start_ma = 10.0;  // 10 mA
let stop_ma = 100.0;  // 100 mA
let step_ma = 5.0;    // 5 mA steps
let stabilization_delay_ms = 50; // 50ms stabilization delay
```

## Troubleshooting

If you encounter issues with the application, check the following:

1. Ensure the CLD1015 is properly connected via USB and detected by your system
2. Verify the MPM-210H is on the same network and reachable via the configured IP address
3. Check the log files in the `logs` directory for detailed error information
4. Ensure the optical path is properly aligned between the laser and the power meter

## License

MIT