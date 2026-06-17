use serde::{Deserialize, Serialize};

/// Configuration loaded from `config.toml` next to the binary,
/// or from `~/.config/efact-hardware-agent/config.toml` as fallback.
/// Legacy `efact-printer-agent` paths are still read for upgrades.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AgentConfig {
    /// HTTP port to listen on. Default: 8765
    #[serde(default = "default_port")]
    pub port: u16,

    /// Tray icon style: auto, color, dark, or light.
    #[serde(default = "default_tray_icon")]
    pub tray_icon: String,

    /// Printer settings. Top-level keys remain supported for legacy configs.
    #[serde(default, flatten)]
    pub printer: PrinterConfig,

    #[serde(default)]
    pub scale: ScaleConfig,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PrinterConfig {
    /// Specific USB vendor_id to target (hex string, e.g. "04b8" for Epson).
    pub usb_vendor_id: Option<String>,

    /// Specific USB product_id to target (hex string).
    pub usb_product_id: Option<String>,

    /// USB output endpoint address. Default: 0x01 (most thermal printers).
    #[serde(default = "default_endpoint")]
    pub usb_endpoint: u8,

    /// Chunk size in bytes when writing to USB. Default: 4096
    #[serde(default = "default_chunk_size")]
    pub chunk_size: usize,

    /// Optional system printer name to target through the OS print spooler.
    pub system_printer_name: Option<String>,

    /// Prefer the system print backend before trying USB HID.
    #[serde(default)]
    pub prefer_system_backend: bool,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ScaleConfig {
    /// Enable RS-232 scale integration.
    #[serde(default)]
    pub enabled: bool,

    /// Serial port path, e.g. COM3, /dev/ttyUSB0, /dev/cu.usbserial-*
    pub serial_port: Option<String>,

    /// Scale protocol. See GET /scale/protocols for supported values.
    #[serde(default = "default_scale_protocol")]
    pub protocol: String,

    #[serde(default = "default_baud_rate")]
    pub baud_rate: u32,

    #[serde(default = "default_data_bits")]
    pub data_bits: u8,

    #[serde(default = "default_parity")]
    pub parity: String,

    #[serde(default = "default_stop_bits")]
    pub stop_bits: u8,

    /// Identical consecutive readings required to mark weight as stable.
    #[serde(default = "default_stable_reads")]
    pub stable_reads: u8,

    /// Minimum milliseconds between identical readings for stability.
    #[serde(default = "default_stable_window_ms")]
    pub stable_window_ms: u64,
}

fn default_port() -> u16 {
    8765
}

fn default_tray_icon() -> String {
    "auto".to_string()
}

fn default_endpoint() -> u8 {
    0x01
}

fn default_chunk_size() -> usize {
    4096
}

fn default_scale_protocol() -> String {
    "excell".to_string()
}

fn default_baud_rate() -> u32 {
    9600
}

fn default_data_bits() -> u8 {
    8
}

fn default_parity() -> String {
    "none".to_string()
}

fn default_stop_bits() -> u8 {
    1
}

fn default_stable_reads() -> u8 {
    3
}

fn default_stable_window_ms() -> u64 {
    200
}

impl Default for PrinterConfig {
    fn default() -> Self {
        Self {
            usb_vendor_id: None,
            usb_product_id: None,
            usb_endpoint: default_endpoint(),
            chunk_size: default_chunk_size(),
            system_printer_name: None,
            prefer_system_backend: false,
        }
    }
}

impl Default for ScaleConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            serial_port: None,
            protocol: default_scale_protocol(),
            baud_rate: default_baud_rate(),
            data_bits: default_data_bits(),
            parity: default_parity(),
            stop_bits: default_stop_bits(),
            stable_reads: default_stable_reads(),
            stable_window_ms: default_stable_window_ms(),
        }
    }
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            port: default_port(),
            tray_icon: default_tray_icon(),
            printer: PrinterConfig::default(),
            scale: ScaleConfig::default(),
        }
    }
}
