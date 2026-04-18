use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream, SocketAddr};
#[cfg(windows)]
use std::sync::mpsc;
#[cfg(windows)]
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::{env, fs, path::PathBuf};

use constant_time_eq::constant_time_eq;
use dotenvy::from_path;
use log::{info, warn, LevelFilter};
use simplelog::{Config, WriteLogger};

#[cfg(windows)]
use windows_service::{
    define_windows_service,
    service::{
        ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus, ServiceType,
    },
    service_control_handler::{self, ServiceControlHandlerResult},
    service_dispatcher,
};

#[cfg(windows)]
const SERVICE_NAME: &str = "ShutdownHelper";
#[cfg(windows)]
const SERVICE_TYPE: ServiceType = ServiceType::OWN_PROCESS;

const DEFAULT_BIND_ADDRESS: &str = "127.0.0.1:8080";
const DEFAULT_AUTH_TOKEN: &str = "change-me-secret-token";
const DEFAULT_LOG_DIR: &str = "logs";

const READ_BUFFER_SIZE: usize = 512;
const MAX_HEADERS: usize = 16;
const NETWORK_TIMEOUT: Duration = Duration::from_secs(5);

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let exe_path = env::current_exe()?;
    let base_dir = exe_path.parent().ok_or("Could not find executable directory")?;

    // Load .env from the executable directory
    let env_path = base_dir.join(".env");
    let _ = from_path(env_path);

    #[cfg(windows)]
    {
        if let Err(_e) = service_dispatcher::start(SERVICE_NAME, ffi_service_main) {
            run_server_loop(base_dir.to_path_buf())?;
        }
    }

    #[cfg(not(windows))]
    {
        run_server_loop(base_dir.to_path_buf())?;
    }

    Ok(())
}

#[cfg(windows)]
define_windows_service!(ffi_service_main, service_main);

#[cfg(windows)]
fn service_main(_arguments: Vec<std::ffi::OsString>) {
    if let Ok(exe_path) = env::current_exe() {
        if let Some(base_dir) = exe_path.parent() {
            if let Err(e) = run_service(base_dir.to_path_buf()) {
                warn!("Service failure: {}", e);
            }
            return;
        }
    }
    warn!("Service failed to initialize base directory");
}

#[cfg(windows)]
fn run_service(base_dir: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let (tx, rx) = mpsc::channel();

    let event_handler = move |control_event| -> ServiceControlHandlerResult {
        match control_event {
            ServiceControl::Stop => {
                let _ = tx.send(());
                ServiceControlHandlerResult::NoError
            }
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            _ => ServiceControlHandlerResult::NotImplemented,
        }
    };

    let status_handle = service_control_handler::register(SERVICE_NAME, event_handler)?;

    status_handle.set_service_status(ServiceStatus {
        service_type: SERVICE_TYPE,
        current_state: ServiceState::Running,
        controls_accepted: ServiceControlAccept::STOP,
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    })?;

    // Start server in background thread
    let server_dir = base_dir.clone();
    thread::spawn(move || {
        if let Err(e) = run_server_loop(server_dir) {
            warn!("Server loop exited with error: {}", e);
        }
    });

    // Wait for stop signal
    let _ = rx.recv();

    status_handle.set_service_status(ServiceStatus {
        service_type: SERVICE_TYPE,
        current_state: ServiceState::Stopped,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    })?;

    Ok(())
}

fn init_logging(base_dir: &PathBuf, log_dir_name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let log_dir = base_dir.join(log_dir_name);
    fs::create_dir_all(&log_dir)?;

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_secs();

    let log_filename = format!("shutdown-helper-{}.log", timestamp);
    let log_path = log_dir.join(log_filename);

    let log_file = fs::File::create(log_path)?;
    WriteLogger::init(LevelFilter::Info, Config::default(), log_file)?;

    Ok(())
}

fn run_server_loop(base_dir: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let log_dir_name = env::var("LOG_DIR").unwrap_or_else(|_| DEFAULT_LOG_DIR.to_string());
    init_logging(&base_dir, &log_dir_name)?;

    let addr_str = env::var("BIND_ADDRESS").unwrap_or_else(|_| DEFAULT_BIND_ADDRESS.to_string());
    let addr: SocketAddr = addr_str.parse()?;

    let auth_token_str = env::var("AUTH_TOKEN").unwrap_or_else(|_| DEFAULT_AUTH_TOKEN.to_string());
    let auth_token = auth_token_str.as_bytes();

    let listener = TcpListener::bind(&addr)?;
    info!("Server listening on {}", addr);

    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                let _ = stream.set_read_timeout(Some(NETWORK_TIMEOUT));
                let _ = stream.set_write_timeout(Some(NETWORK_TIMEOUT));

                match handle_connection(&mut stream, auth_token) {
                    Ok(true) => {
                        info!("Shutdown signal received and authorized. Shutting down system.");
                        let _ = system_shutdown::shutdown();
                        break;
                    }
                    Ok(false) => {}
                    Err(e) => warn!("Error handling connection: {}", e),
                }
            }
            Err(e) => warn!("Connection failed: {}", e),
        }
    }

    Ok(())
}

fn handle_connection(stream: &mut TcpStream, expected_token: &[u8]) -> Result<bool, Box<dyn std::error::Error>> {
    let mut buffer = [0u8; READ_BUFFER_SIZE];
    let n = stream.read(&mut buffer)?;
    if n == 0 { return Ok(false); }

    let mut headers = [httparse::EMPTY_HEADER; MAX_HEADERS];
    let mut req = httparse::Request::new(&mut headers);

    let res = req.parse(&buffer[..n])?;
    if !res.is_complete() {
        return Ok(false);
    }

    if req.method != Some("POST") || req.path != Some("/shutdown") {
        let _ = stream.write_all(b"HTTP/1.1 404 Not Found\r\n\r\n");
        return Ok(false);
    }

    let mut authorized = false;
    for header in req.headers {
        if header.name.eq_ignore_ascii_case("x-auth-token") {
            if constant_time_eq(header.value, expected_token) {
                authorized = true;
                break;
            }
        }
    }

    if authorized {
        let _ = stream.write_all(b"HTTP/1.1 200 OK\r\n\r\n");
        Ok(true)
    } else {
        warn!("Unauthorized access attempt");
        let _ = stream.write_all(b"HTTP/1.1 401 Unauthorized\r\n\r\n");
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::thread;

    #[test]
    fn test_handle_connection_unauthorized() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            handle_connection(&mut stream, b"secret").unwrap();
        });

        let mut client = TcpStream::connect(addr).unwrap();
        client.write_all(b"POST /shutdown HTTP/1.1\r\nx-auth-token: wrong\r\n\r\n").unwrap();

        let mut response = String::new();
        client.read_to_string(&mut response).unwrap();
        assert!(response.contains("401 Unauthorized"));
    }

    #[test]
    fn test_handle_connection_authorized() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            handle_connection(&mut stream, b"secret").unwrap();
        });

        let mut client = TcpStream::connect(addr).unwrap();
        client.write_all(b"POST /shutdown HTTP/1.1\r\nx-auth-token: secret\r\n\r\n").unwrap();

        let mut response = String::new();
        client.read_to_string(&mut response).unwrap();
        assert!(response.contains("200 OK"));
    }

    #[test]
    fn test_handle_connection_not_found() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            handle_connection(&mut stream, b"secret").unwrap();
        });

        let mut client = TcpStream::connect(addr).unwrap();
        client.write_all(b"GET / HTTP/1.1\r\n\r\n").unwrap();

        let mut response = String::new();
        client.read_to_string(&mut response).unwrap();
        assert!(response.contains("404 Not Found"));
    }
}
