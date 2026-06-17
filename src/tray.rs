use std::{process::Command, thread};

use tokio::sync::oneshot;
use tracing::{error, warn};
use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    TrayIconBuilder,
};
use winit::{
    application::ApplicationHandler,
    event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy},
};

use crate::{
    config::AgentConfig,
    icon::build_tray_icon,
    paths::{self, APP_DISPLAY_NAME},
    run_server, AppState,
};

#[derive(Debug, Clone)]
enum UserEvent {
    Menu(MenuEvent),
}

pub(crate) fn run(state: AppState) {
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    let tray_config = state.config_store.get();
    let port = tray_config.port;

    let server_thread = thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();

        runtime.block_on(async move {
            let shutdown = async move {
                let _ = shutdown_rx.await;
            };

            if let Err(err) = run_server(state, port, shutdown).await {
                error!("Failed to run hardware agent server: {err}");
            }
        });
    });

    if let Err(err) = run_tray_app(tray_config, shutdown_tx) {
        error!("Tray application failed: {err}");
    }

    let _ = server_thread.join();
}

fn run_tray_app(config: AgentConfig, shutdown_tx: oneshot::Sender<()>) -> Result<(), String> {
    let event_loop = build_event_loop()?;

    let menu_proxy = event_loop.create_proxy();
    MenuEvent::set_event_handler(Some(move |event| {
        let _ = menu_proxy.send_event(UserEvent::Menu(event));
    }));

    let mut app = TrayApp::new(config, shutdown_tx, event_loop.create_proxy());
    event_loop.run_app(&mut app).map_err(|err| err.to_string())
}

fn build_event_loop() -> Result<EventLoop<UserEvent>, String> {
    #[cfg(target_os = "macos")]
    {
        use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};

        EventLoop::<UserEvent>::with_user_event()
            .with_activation_policy(ActivationPolicy::Accessory)
            .build()
            .map_err(|err| err.to_string())
    }

    #[cfg(not(target_os = "macos"))]
    {
        EventLoop::<UserEvent>::with_user_event()
            .build()
            .map_err(|err| err.to_string())
    }
}

struct TrayApp {
    config: AgentConfig,
    shutdown_tx: Option<oneshot::Sender<()>>,
    event_proxy: EventLoopProxy<UserEvent>,
    tray_icon: Option<tray_icon::TrayIcon>,
    status_item: Option<MenuItem>,
    open_config_item: Option<MenuItem>,
    open_logs_item: Option<MenuItem>,
    quit_item: Option<MenuItem>,
}

impl TrayApp {
    fn new(
        config: AgentConfig,
        shutdown_tx: oneshot::Sender<()>,
        event_proxy: EventLoopProxy<UserEvent>,
    ) -> Self {
        Self {
            config,
            shutdown_tx: Some(shutdown_tx),
            event_proxy,
            tray_icon: None,
            status_item: None,
            open_config_item: None,
            open_logs_item: None,
            quit_item: None,
        }
    }

    fn build_tray(&mut self) -> Result<(), String> {
        let menu = Menu::new();
        let status_text = status_label(&self.config);
        let status_item = MenuItem::new(status_text, false, None);
        let open_config_item = MenuItem::new("Abrir configuración", true, None);
        let open_logs_item = MenuItem::new("Abrir registros", true, None);
        let quit_item = MenuItem::new("Salir", true, None);

        menu.append(&status_item).map_err(|err| err.to_string())?;
        menu.append(&PredefinedMenuItem::separator())
            .map_err(|err| err.to_string())?;
        menu.append(&open_config_item)
            .map_err(|err| err.to_string())?;
        menu.append(&open_logs_item)
            .map_err(|err| err.to_string())?;
        menu.append(&PredefinedMenuItem::separator())
            .map_err(|err| err.to_string())?;
        menu.append(&quit_item).map_err(|err| err.to_string())?;

        let tooltip = format!("{APP_DISPLAY_NAME}\n{}", status_label(&self.config));
        let tray_icon = TrayIconBuilder::new()
            .with_tooltip(tooltip)
            .with_menu(Box::new(menu))
            .with_icon(build_tray_icon(&self.config.tray_icon)?)
            .build()
            .map_err(|err| err.to_string())?;

        self.status_item = Some(status_item);
        self.open_config_item = Some(open_config_item);
        self.open_logs_item = Some(open_logs_item);
        self.quit_item = Some(quit_item);
        self.tray_icon = Some(tray_icon);

        Ok(())
    }

    fn handle_menu_event(&mut self, event: MenuEvent, event_loop: &ActiveEventLoop) {
        if self
            .open_config_item
            .as_ref()
            .is_some_and(|item| event.id == *item.id())
        {
            if let Err(err) = open_folder(paths::config_dir()) {
                warn!("Failed to open config folder: {err}");
            }
            return;
        }

        if self
            .open_logs_item
            .as_ref()
            .is_some_and(|item| event.id == *item.id())
        {
            if let Err(err) = open_folder(paths::log_dir()) {
                warn!("Failed to open logs folder: {err}");
            }
            return;
        }

        if self
            .quit_item
            .as_ref()
            .is_some_and(|item| event.id == *item.id())
        {
            if let Some(shutdown_tx) = self.shutdown_tx.take() {
                let _ = shutdown_tx.send(());
            }
            event_loop.exit();
        }
    }
}

impl ApplicationHandler<UserEvent> for TrayApp {
    fn window_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _window_id: winit::window::WindowId,
        _event: winit::event::WindowEvent,
    ) {
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let _ = &self.event_proxy;
        if self.tray_icon.is_none() {
            if let Err(err) = self.build_tray() {
                error!("Failed to create tray icon: {err}");
                if let Some(shutdown_tx) = self.shutdown_tx.take() {
                    let _ = shutdown_tx.send(());
                }
                event_loop.exit();
            }
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent) {
        match event {
            UserEvent::Menu(event) => self.handle_menu_event(event, event_loop),
        }
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(());
        }
    }
}

fn status_label(config: &AgentConfig) -> String {
    let printer = if let Some(name) = &config.printer.system_printer_name {
        format!("Impresora: {name}")
    } else if config.printer.prefer_system_backend {
        "Impresora: sistema (predeterminada)".to_string()
    } else {
        "Impresora: auto (HID o sistema)".to_string()
    };

    let scale = if config.scale.enabled {
        let port = config.scale.serial_port.as_deref().unwrap_or("sin puerto");
        format!("Balanza: {} ({})", config.scale.protocol, port)
    } else {
        "Balanza: desactivada".to_string()
    };

    format!("{printer} | {scale}")
}

fn open_folder(path: std::path::PathBuf) -> Result<(), std::io::Error> {
    std::fs::create_dir_all(&path)?;

    #[cfg(target_os = "windows")]
    Command::new("explorer").arg(path).spawn()?;

    #[cfg(target_os = "macos")]
    Command::new("open").arg(path).spawn()?;

    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    Command::new("xdg-open").arg(path).spawn()?;

    Ok(())
}
