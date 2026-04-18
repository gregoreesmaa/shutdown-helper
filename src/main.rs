mod config;
mod logger;
mod server;

#[cfg(test)]
mod tests;

use crate::config::Config;
use crate::logger::init_logging;
use crate::server::run_server;
use anyhow::Result;
use std::path::PathBuf;
#[cfg(windows)]
use tokio::sync::watch;
use tracing::{error, info};

#[cfg(windows)]
use windows_service::{
    define_windows_service,
    service::{
        ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceStatus, ServiceType,
    },
    service_control_handler::{self, ServiceControlHandlerResult},
    service_dispatcher,
};

#[cfg(windows)]
const SERVICE_NAME: &str = "ShutdownHelper";
#[cfg(windows)]
const SERVICE_TYPE: ServiceType = ServiceType::OWN_PROCESS;

#[cfg(windows)]
define_windows_service!(ffi_service_main, service_main);

fn main() -> Result<()> {
    #[cfg(windows)]
    {
        if let Err(_e) = service_dispatcher::start(SERVICE_NAME, ffi_service_main) {
            run_standalone()?;
        }
    }

    #[cfg(not(windows))]
    {
        run_standalone()?;
    }

    Ok(())
}

fn get_absolute_log_dir(log_dir: &str) -> Result<PathBuf> {
    let exe_path = std::env::current_exe()?;
    let exe_dir = exe_path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Could not find executable directory"))?;
    Ok(exe_dir.join(log_dir))
}

fn run_standalone() -> Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let config = Config::load().expect("Failed to load config");
        let abs_log_dir = get_absolute_log_dir(&config.log_dir).expect("Failed to resolve log directory");
        let _guard = init_logging(abs_log_dir.to_str().expect("Invalid log path"));
        info!("Running as standalone application");
        if let Err(e) = run_server(config, None).await {
            error!("Server error: {}", e);
        }
    });
    Ok(())
}

#[cfg(windows)]
fn service_main(_arguments: Vec<std::ffi::OsString>) {
    if let Err(e) = run_service() {
        error!("Service failure: {}", e);
    }
}

#[cfg(windows)]
fn run_service() -> Result<()> {
    let (shutdown_tx, shutdown_rx) = watch::channel(());

    let event_handler = move |control_event| -> ServiceControlHandlerResult {
        match control_event {
            ServiceControl::Stop => {
                let _ = shutdown_tx.send(());
                ServiceControlHandlerResult::NoError
            }
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            _ => ServiceControlHandlerResult::NotImplemented,
        }
    };

    let status_handle = service_control_handler::register(SERVICE_NAME, event_handler)?;

    status_handle.set_service_status(ServiceStatus {
        service_type: SERVICE_TYPE,
        current_state: windows_service::service::ServiceState::Running,
        controls_accepted: ServiceControlAccept::STOP,
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: std::time::Duration::default(),
        process_id: None,
    })?;

    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let config = Config::load().expect("Failed to load config");
        let abs_log_dir = get_absolute_log_dir(&config.log_dir).expect("Failed to resolve log directory");
        let _guard = init_logging(abs_log_dir.to_str().expect("Invalid log path"));
        info!("Running as Windows Service");
        if let Err(e) = run_server(config, Some(shutdown_rx)).await {
            error!("Server error: {}", e);
        }
    });

    status_handle.set_service_status(ServiceStatus {
        service_type: SERVICE_TYPE,
        current_state: windows_service::service::ServiceState::Stopped,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: std::time::Duration::default(),
        process_id: None,
    })?;

    Ok(())
}
