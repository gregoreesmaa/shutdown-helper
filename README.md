# Shutdown Helper

An ultra-low footprint Rust utility to remotely shut down Windows machines via a secure REST endpoint. Optimized for native system integration and minimal resource usage, it's designed to be the "turn off" companion for smart home platforms like Home Assistant.

## Use Cases

- **Smart Home Integration**: Add a "Power Off" switch to your dashboard that works alongside Wake-on-LAN.
- **Remote Management**: Safely shut down headless machines or workstations without RDP/SSH.
- **Automation**: Trigger shutdowns based on occupancy, power usage (UPS), or bedtime routines.

## Features

- **Extreme Efficiency**: Single-binary, zero-async, and zero-allocation request handling. Minimal RAM/CPU impact.
- **Windows Service**: Runs natively in the background; starts automatically with Windows.
- **Secure**: Constant-time token verification via `X-Auth-Token` header.
- **Persistent Audit**: Generates a unique, timestamped log file on every boot for auditing.
- **Zero-Config Deployment**: Includes a PowerShell script for one-command installation.

## Prerequisites

- **Rust**: You must have the Rust toolchain installed to build the binary. [Install Rust](https://www.rust-lang.org/tools/install).

## Installation

1. **Build & Install**:
   Run as Administrator:
   ```powershell
   ./setup.ps1
   ```
   This installs the helper to `C:\Program Files\ShutdownHelper`.

2. **Configure**:
   Edit `C:\Program Files\ShutdownHelper\.env` (the script creates a default one if missing):
   ```env
   BIND_ADDRESS=0.0.0.0:7986
   AUTH_TOKEN=your-secret-token
   LOG_DIR=logs
   ```

3. **Apply Changes**:
   ```powershell
   Restart-Service ShutdownHelper
   ```

## Usage

Endpoint: `POST /shutdown`

```bash
curl -X POST http://<IP>:7986/shutdown -H "X-Auth-Token: your-secret-token"
```

## Home Assistant Integration

Integrate the Shutdown Helper as a standard switch using the `wake_on_lan` platform. This provides a single toggle that can both turn the computer **on** (via Magic Packet) and **off** (via this REST API).

```yaml
# configuration.yaml
wake_on_lan:

rest_command:
  shutdown_computer:
    url: "http://192.168.1.50:7986/shutdown" # Replace with your PC's IP or hostname
    method: POST
    headers:
      X-Auth-Token: "YOUR_SECRET_TOKEN"

switch:
  - platform: wake_on_lan
    mac: "AA:BB:CC:DD:EE:FF" # Replace with your PC's MAC address
    name: "Computer"
    host: "192.168.1.50" # Replace with your PC's IP or hostname
    turn_off:
      action: rest_command.shutdown_computer
```

## Audit Logs

Logs are uniquely timestamped on each service start:
`C:\Program Files\ShutdownHelper\logs\shutdown-helper-<TIMESTAMP>.log`

---
> **Claude**: This documentation reflects the refactored single-file architecture and architectural optimizations.
