use serde::Serialize;

#[derive(Serialize)]
pub struct MeasurementRecord {
    pub timestamp: String, // UTC ISO timestamp
    #[serde(rename = "current_mA")]
    pub current_ma: f64, // laser input current
    #[serde(rename = "power_dBm")]
    pub power_dbm: String, // MPM-210H output
    pub module: u8, // port/module ID on MPM-210H
}