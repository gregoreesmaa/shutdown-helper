# Shutdown Helper Setup Script

$ServiceName = "ShutdownHelper"
$BinaryName = "shutdown-helper.exe"
$ReleasePath = "target\release\$BinaryName"
$InstallPath = "C:\Program Files\ShutdownHelper"

# 1. Build the project
Write-Host "Building project in release mode..." -ForegroundColor Cyan
cargo build --release
if ($LASTEXITCODE -ne 0) {
    Write-Error "Build failed!"
    exit 1
}

# 2. Create install directory
if (!(Test-Path $InstallPath)) {
    Write-Host "Creating install directory at $InstallPath..." -ForegroundColor Cyan
    New-Item -ItemType Directory -Force -Path $InstallPath | Out-Null
}

# 3. Stop service if it exists
if (Get-Service $ServiceName -ErrorAction SilentlyContinue) {
    Write-Host "Stopping existing service..." -ForegroundColor Cyan
    Stop-Service $ServiceName -ErrorAction SilentlyContinue
}

# 4. Copy files
Write-Host "Copying binary to $InstallPath..." -ForegroundColor Cyan
Copy-Item $ReleasePath -Destination $InstallPath -Force

if (Test-Path "config.toml") {
    Write-Host "Copying config.toml to $InstallPath..." -ForegroundColor Cyan
    Copy-Item "config.toml" -Destination $InstallPath -Force
}

# 5. Create/Update service
$BinaryPath = Join-Path $InstallPath $BinaryName
if (Get-Service $ServiceName -ErrorAction SilentlyContinue) {
    Write-Host "Service already exists. Updating binary path..." -ForegroundColor Cyan
    sc.exe config $ServiceName binPath= $BinaryPath start= auto
} else {
    Write-Host "Creating new Windows Service..." -ForegroundColor Cyan
    sc.exe create $ServiceName binPath= $BinaryPath start= auto
}

# 6. Start service
Write-Host "Starting service..." -ForegroundColor Cyan
Start-Service $ServiceName

Write-Host "Setup complete!" -ForegroundColor Green
