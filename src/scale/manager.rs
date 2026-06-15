use std::{
    sync::{
        mpsc::{self, Receiver, Sender},
        Arc, RwLock,
    },
    thread,
    time::{Duration, Instant},
};

use serde::Serialize;
use serialport::{DataBits, FlowControl, Parity, SerialPort, StopBits};
use thiserror::Error;

use crate::config::ScaleConfig;
use crate::config_store::ConfigStore;

use super::protocols::{ScaleProtocol, WeightUnit};

#[derive(Debug, Error)]
pub enum ScaleError {
    #[error("Scale integration is disabled in config.toml")]
    Disabled,

    #[error("Serial port error: {0}")]
    Serial(#[from] serialport::Error),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("No weight reading available yet")]
    NoReading,
}

#[derive(Debug, Clone, Serialize)]
pub struct WeightReading {
    pub kg: f64,
    pub value: f64,
    pub unit: WeightUnit,
    pub stable: bool,
    pub connected: bool,
    pub protocol: String,
    pub port: Option<String>,
    pub raw: Option<String>,
    pub updated_at_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScaleStatus {
    pub enabled: bool,
    pub connected: bool,
    pub protocol: String,
    pub port: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SerialPortInfo {
    pub name: String,
    pub description: Option<String>,
}

enum ScaleCommand {
    Reload,
}

pub struct ScaleManager {
    config_store: Arc<ConfigStore>,
    reading: Arc<RwLock<WeightReading>>,
    status: Arc<RwLock<ScaleStatus>>,
    cmd_tx: Sender<ScaleCommand>,
}

impl ScaleManager {
    pub fn new(config_store: Arc<ConfigStore>) -> Self {
        let scale_config = config_store.get().scale;
        let protocol = scale_config.protocol.clone();
        let port = scale_config.serial_port.clone();
        let enabled = scale_config.enabled;

        let reading = Arc::new(RwLock::new(WeightReading {
            kg: 0.0,
            value: 0.0,
            unit: WeightUnit::Kg,
            stable: false,
            connected: false,
            protocol: protocol.clone(),
            port: port.clone(),
            raw: None,
            updated_at_ms: 0,
        }));

        let status = Arc::new(RwLock::new(ScaleStatus {
            enabled,
            connected: false,
            protocol,
            port,
            last_error: None,
        }));

        let (cmd_tx, cmd_rx) = mpsc::channel();
        let supervisor_store = Arc::clone(&config_store);
        let supervisor_reading = Arc::clone(&reading);
        let supervisor_status = Arc::clone(&status);

        thread::spawn(move || {
            run_supervisor(
                supervisor_store,
                supervisor_reading,
                supervisor_status,
                cmd_rx,
            );
        });

        Self {
            config_store,
            reading,
            status,
            cmd_tx,
        }
    }

    pub fn reload(&self) {
        let _ = self.cmd_tx.send(ScaleCommand::Reload);
    }

    pub fn list_ports(&self) -> Vec<SerialPortInfo> {
        serialport::available_ports()
            .unwrap_or_default()
            .into_iter()
            .map(|port| SerialPortInfo {
                name: port.port_name,
                description: Some(format!("{:?}", port.port_type)),
            })
            .collect()
    }

    pub fn status(&self) -> ScaleStatus {
        let mut status = self.status.read().expect("scale status lock").clone();
        let config = self.config_store.get().scale;
        status.enabled = config.enabled;
        status.protocol = config.protocol;
        status.port = config.serial_port;
        status
    }

    pub fn weight(&self) -> Result<WeightReading, ScaleError> {
        if !self.config_store.get().scale.enabled {
            return Err(ScaleError::Disabled);
        }

        let reading = self.reading.read().expect("scale reading lock").clone();
        if reading.updated_at_ms == 0 {
            return Err(ScaleError::NoReading);
        }
        Ok(reading)
    }
}

fn run_supervisor(
    config_store: Arc<ConfigStore>,
    reading: Arc<RwLock<WeightReading>>,
    status: Arc<RwLock<ScaleStatus>>,
    cmd_rx: Receiver<ScaleCommand>,
) {
    loop {
        let config = config_store.get().scale;
        sync_status_from_config(&status, &config);

        if !config.enabled {
            reset_reading(&reading, &config);
            wait_for_reload(&cmd_rx);
            continue;
        }

        let Some(port_name) = config.serial_port.clone() else {
            set_error(&status, "scale.serial_port is not configured");
            reset_reading(&reading, &config);
            wait_for_reload(&cmd_rx);
            continue;
        };

        let Some(protocol) = ScaleProtocol::parse_id(&config.protocol) else {
            set_error(&status, &format!("Unknown protocol: {}", config.protocol));
            reset_reading(&reading, &config);
            wait_for_reload(&cmd_rx);
            continue;
        };

        let (stop_tx, stop_rx) = mpsc::channel::<()>();
        let reader_config = config.clone();
        let reader_port = port_name.clone();
        let reader_reading = Arc::clone(&reading);
        let reader_status = Arc::clone(&status);

        let reader_handle = thread::spawn(move || {
            let _ = run_reader(
                &reader_config,
                protocol,
                &reader_port,
                &reader_reading,
                &reader_status,
                &stop_rx,
            );
        });

        loop {
            match cmd_rx.recv_timeout(Duration::from_millis(250)) {
                Ok(ScaleCommand::Reload) => {
                    let _ = stop_tx.send(());
                    let _ = reader_handle.join();
                    break;
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    let _ = stop_tx.send(());
                    let _ = reader_handle.join();
                    return;
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if reader_handle.is_finished() {
                        break;
                    }
                }
            }
        }

        thread::sleep(Duration::from_millis(300));
    }
}

fn wait_for_reload(cmd_rx: &Receiver<ScaleCommand>) {
    loop {
        match cmd_rx.recv_timeout(Duration::from_secs(2)) {
            Ok(ScaleCommand::Reload) => break,
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
    }
}

fn sync_status_from_config(status: &Arc<RwLock<ScaleStatus>>, config: &ScaleConfig) {
    if let Ok(mut st) = status.write() {
        st.enabled = config.enabled;
        st.protocol = config.protocol.clone();
        st.port = config.serial_port.clone();
    }
}

fn reset_reading(reading: &Arc<RwLock<WeightReading>>, config: &ScaleConfig) {
    if let Ok(mut rd) = reading.write() {
        rd.connected = false;
        rd.stable = false;
        rd.protocol = config.protocol.clone();
        rd.port = config.serial_port.clone();
        rd.updated_at_ms = 0;
    }
}

fn set_error(status: &Arc<RwLock<ScaleStatus>>, message: &str) {
    if let Ok(mut st) = status.write() {
        st.connected = false;
        st.last_error = Some(message.to_string());
    }
}

fn run_reader(
    config: &ScaleConfig,
    protocol: ScaleProtocol,
    port_name: &str,
    reading: &Arc<RwLock<WeightReading>>,
    status: &Arc<RwLock<ScaleStatus>>,
    stop_rx: &Receiver<()>,
) -> Result<(), ScaleError> {
    let mut port = open_port(config, port_name)?;
    tracing::info!(
        "Scale connected on {} using protocol {}",
        port_name,
        protocol.info().id
    );

    if let Ok(mut st) = status.write() {
        st.connected = true;
        st.last_error = None;
    }

    let mut buffer = Vec::new();
    let mut scratch = [0u8; 256];
    let mut tracker = StabilityTracker::new(config.stable_reads, config.stable_window_ms);

    loop {
        if stop_rx.try_recv().is_ok() {
            tracing::info!("Scale reader stopping for config reload");
            return Ok(());
        }

        let n = match port.read(&mut scratch) {
            Ok(n) => n,
            Err(err) if err.kind() == std::io::ErrorKind::TimedOut => {
                continue;
            }
            Err(err) => return Err(err.into()),
        };

        if n == 0 {
            thread::sleep(Duration::from_millis(20));
            continue;
        }

        buffer.extend_from_slice(&scratch[..n]);

        while let Some(pos) = buffer.iter().position(|b| *b == b'\n' || *b == b'\r') {
            let line_bytes: Vec<u8> = buffer.drain(..=pos).collect();
            let line = String::from_utf8_lossy(&line_bytes).to_string();
            if line.trim().is_empty() {
                continue;
            }

            if let Some(parsed) = protocol.parse_line(&line) {
                publish_reading(
                    reading,
                    protocol,
                    port_name,
                    &line,
                    &parsed,
                    tracker.update(parsed.unit.as_kg(parsed.value)),
                );
            }
        }

        if buffer.len() > 80 {
            let raw = String::from_utf8_lossy(&buffer).to_string();
            if let Some(parsed) = protocol.parse_line(&raw) {
                publish_reading(
                    reading,
                    protocol,
                    port_name,
                    &raw,
                    &parsed,
                    tracker.update(parsed.unit.as_kg(parsed.value)),
                );
            }
            buffer.clear();
        }
    }
}

fn publish_reading(
    reading: &Arc<RwLock<WeightReading>>,
    protocol: ScaleProtocol,
    port_name: &str,
    line: &str,
    parsed: &super::protocols::ParsedWeight,
    stable: bool,
) {
    let kg = parsed.unit.as_kg(parsed.value);
    let now_ms = unix_ms();
    if let Ok(mut rd) = reading.write() {
        rd.kg = kg;
        rd.value = parsed.value;
        rd.unit = parsed.unit;
        rd.stable = stable;
        rd.connected = true;
        rd.protocol = protocol.info().id.to_string();
        rd.port = Some(port_name.to_string());
        rd.raw = Some(line.trim().to_string());
        rd.updated_at_ms = now_ms;
    }
}

fn open_port(config: &ScaleConfig, port_name: &str) -> Result<Box<dyn SerialPort>, ScaleError> {
    let parity = match config.parity.to_ascii_lowercase().as_str() {
        "even" => Parity::Even,
        "odd" => Parity::Odd,
        _ => Parity::None,
    };

    let data_bits = match config.data_bits {
        5 => DataBits::Five,
        6 => DataBits::Six,
        7 => DataBits::Seven,
        _ => DataBits::Eight,
    };

    let stop_bits = match config.stop_bits {
        2 => StopBits::Two,
        _ => StopBits::One,
    };

    let port = serialport::new(port_name, config.baud_rate)
        .data_bits(data_bits)
        .parity(parity)
        .stop_bits(stop_bits)
        .flow_control(FlowControl::None)
        .timeout(Duration::from_millis(200))
        .open()?;

    Ok(port)
}

struct StabilityTracker {
    last_kg: Option<f64>,
    stable_count: u8,
    stable_reads_required: u8,
    stable_window: Duration,
    last_change: Instant,
}

impl StabilityTracker {
    fn new(stable_reads_required: u8, stable_window_ms: u64) -> Self {
        Self {
            last_kg: None,
            stable_count: 0,
            stable_reads_required: stable_reads_required.max(1),
            stable_window: Duration::from_millis(stable_window_ms.max(50)),
            last_change: Instant::now(),
        }
    }

    fn update(&mut self, kg: f64) -> bool {
        let same = self.last_kg.is_some_and(|prev| (prev - kg).abs() < 0.001);

        if same && self.last_change.elapsed() >= self.stable_window {
            self.stable_count = self.stable_count.saturating_add(1);
        } else if !same {
            self.last_kg = Some(kg);
            self.stable_count = 1;
            self.last_change = Instant::now();
        }

        self.stable_count >= self.stable_reads_required
    }
}

fn unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
