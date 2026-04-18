mod config;
mod logger;
mod server;

use std::sync::Arc;
use crate::config::Config;
use crate::logger::init_logging;
use crate::server::run_server;
use tracing::info;
use windows_service::{
    define_windows_service,
    service::{
        ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceStatus, ServiceType,
    },
    service_control_handler::{self, ServiceControlHandlerResult},
    service_dispatcher,
};
use std::ffi::OsString;
use std::time::Duration;

const SERVICE_NAME: &str = "ShutdownHelper";
const SERVICE_TYPE: ServiceType = ServiceType::OWN_PROCESS;

define_windows_service!(ffi_service_main, service_main);

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Check if we're running as a service
    if let Err(_e) = service_dispatcher::start(SERVICE_NAME, ffi_service_main) {
        // If not running as a service, just run as a regular console app
        // This is useful for debugging
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(async {
            let config = Config::load().expect("Failed to load config");
            let _guard = init_logging(&config.log_dir);
            info!("Running as standalone application");
            run_server(Arc::new(config)).await;
        });
    }
    Ok(())
}

fn service_main(_arguments: Vec<OsString>) {
    if let Err(_e) = run_service() {
        // Handle error
    }
}

fn run_service() -> Result<(), Box<dyn std::error::Error>> {
    let event_handler = move |control_event| -> ServiceControlHandlerResult {
        match control_event {
            ServiceControl::Stop => ServiceControlHandlerResult::NoError,
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
        wait_hint: Duration::default(),
        process_id: None,
    })?;

    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let config = Config::load().expect("Failed to load config");
        let _guard = init_logging(&config.log_dir);
        info!("Running as Windows Service");
        run_server(Arc::new(config)).await;
    });

    status_handle.set_service_status(ServiceStatus {
        service_type: SERVICE_TYPE,
        current_state: windows_service::service::ServiceState::Stopped,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    })?;

    Ok(())
}
