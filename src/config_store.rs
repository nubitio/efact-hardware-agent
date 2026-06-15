use std::{
    path::PathBuf,
    sync::{Arc, RwLock},
};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::config::AgentConfig;
use crate::paths::{config_file_candidates, default_config_path};

#[derive(Debug, Error)]
pub enum ConfigStoreError {
    #[error("Failed to serialize config: {0}")]
    Serialize(#[from] toml::ser::Error),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Clone)]
pub struct ConfigStore {
    inner: Arc<RwLock<StoredConfig>>,
}

#[derive(Clone)]
struct StoredConfig {
    path: PathBuf,
    agent: AgentConfig,
}

impl ConfigStore {
    pub fn load() -> Self {
        for path in config_file_candidates() {
            if !path.exists() {
                continue;
            }

            match std::fs::read_to_string(&path) {
                Ok(contents) => match toml::from_str::<AgentConfig>(&contents) {
                    Ok(agent) => {
                        tracing::info!("Loaded config from {}", path.display());
                        return Self::from_parts(path, agent);
                    }
                    Err(err) => tracing::warn!("Failed to parse {}: {err}", path.display()),
                },
                Err(err) => tracing::warn!("Failed to read {}: {err}", path.display()),
            }
        }

        tracing::info!("No config.toml found, using defaults");
        Self::from_parts(default_config_path(), AgentConfig::default())
    }

    fn from_parts(path: PathBuf, agent: AgentConfig) -> Self {
        Self {
            inner: Arc::new(RwLock::new(StoredConfig { path, agent })),
        }
    }

    pub fn get(&self) -> AgentConfig {
        self.inner.read().expect("config lock").agent.clone()
    }

    pub fn path(&self) -> PathBuf {
        self.inner.read().expect("config lock").path.clone()
    }

    pub fn save(&self, agent: AgentConfig) -> Result<PathBuf, ConfigStoreError> {
        let toml = toml::to_string_pretty(&agent)?;
        let path = self.path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, toml)?;
        self.inner.write().expect("config lock").agent = agent;
        Ok(path)
    }
}

/// Payload accepted by PUT /config — port changes require agent restart.
#[derive(Debug, Deserialize, Serialize)]
pub struct AgentConfigUpdate {
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default)]
    pub printer: Option<PrinterConfigUpdate>,
    #[serde(default)]
    pub scale: Option<crate::config::ScaleConfig>,
}

#[derive(Debug, Deserialize, Serialize, Default)]
pub struct PrinterConfigUpdate {
    pub usb_vendor_id: Option<String>,
    pub usb_product_id: Option<String>,
    pub usb_endpoint: Option<u8>,
    pub chunk_size: Option<usize>,
    pub system_printer_name: Option<String>,
    pub prefer_system_backend: Option<bool>,
}

impl AgentConfigUpdate {
    pub fn merge_into(self, current: AgentConfig) -> AgentConfig {
        let mut next = current;

        if let Some(port) = self.port {
            next.port = port;
        }

        if let Some(printer) = self.printer {
            if let Some(v) = printer.usb_vendor_id {
                next.printer.usb_vendor_id = Some(v);
            }
            if let Some(v) = printer.usb_product_id {
                next.printer.usb_product_id = Some(v);
            }
            if let Some(v) = printer.usb_endpoint {
                next.printer.usb_endpoint = v;
            }
            if let Some(v) = printer.chunk_size {
                next.printer.chunk_size = v;
            }
            if let Some(v) = printer.system_printer_name {
                next.printer.system_printer_name = Some(v);
            }
            if let Some(v) = printer.prefer_system_backend {
                next.printer.prefer_system_backend = v;
            }
        }

        if let Some(scale) = self.scale {
            next.scale = scale;
        }

        next
    }
}
