#Requires -RunAsAdministrator
# Shutdown Helper Setup Script

$ServiceName = "ShutdownHelper"
$BinaryName = "shutdown-helper.exe"
$InstallPath = "C:\Program Files\ShutdownHelper"

# 1. Determine Binary Source
if (Test-Path $BinaryName) {
    Write-Host "Found prebuilt binary: $BinaryName" -ForegroundColor Cyan
    $BinarySource = $BinaryName
} elseif (Test-Path "target\release\$BinaryName") {
    Write-Host "Found built binary in target directory." -ForegroundColor Cyan
    $BinarySource = "target\release\$BinaryName"
} else {
    Write-Host "Binary not found. Attempting to build from source..." -ForegroundColor Yellow
    if (!(Get-Command "cargo" -ErrorAction SilentlyContinue)) {
        Write-Error "Rust/Cargo not found. Please install Rust or use a prebuilt release."
        exit 1
    }
    $env:RUSTFLAGS = "-C target-cpu=native"
    cargo build --release
    if ($LASTEXITCODE -ne 0) {
        Write-Error "Build failed!"
        exit 1
    }
    $BinarySource = "target\release\$BinaryName"
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

# 4. Handle configuration (.env)
$DestEnv = Join-Path $InstallPath ".env"
$GeneratedToken = $null

if (!(Test-Path $DestEnv)) {
    if (Test-Path ".env") {
        Write-Host "Copying existing .env from current directory..." -ForegroundColor Cyan
        Copy-Item ".env" -Destination $DestEnv -Force
    } else {
        Write-Host "Generating new configuration with random token..." -ForegroundColor Cyan
        # Generate a 32-character random hex string
        $RandomBytes = New-Object Byte[] 16
        [System.Security.Cryptography.RandomNumberGenerator]::Create().GetBytes($RandomBytes)
        $GeneratedToken = [System.BitConverter]::ToString($RandomBytes).Replace("-", "").ToLower()

        $DefaultConfig = @"
BIND_ADDRESS=0.0.0.0:7986
AUTH_TOKEN=$GeneratedToken
LOG_DIR=logs
"@
        $DefaultConfig | Out-File -FilePath $DestEnv -Encoding utf8
    }
} else {
    Write-Host "Existing .env found in $InstallPath. Preserving configuration." -ForegroundColor Yellow
}

# 5. Copy binary
Write-Host "Installing binary to $InstallPath..." -ForegroundColor Cyan
Copy-Item $BinarySource -Destination $InstallPath -Force

# 6. Create/Update service
$BinaryPath = Join-Path $InstallPath $BinaryName
if (Get-Service $ServiceName -ErrorAction SilentlyContinue) {
    Write-Host "Updating service configuration..." -ForegroundColor Cyan
    sc.exe config $ServiceName binPath= $BinaryPath start= auto
} else {
    Write-Host "Creating new Windows Service..." -ForegroundColor Cyan
    sc.exe create $ServiceName binPath= $BinaryPath start= auto
}

# 7. Start service
Write-Host "Starting service..." -ForegroundColor Cyan
Start-Service $ServiceName

Write-Host "`nSetup complete!" -ForegroundColor Green
Write-Host "Installed to: $InstallPath"

if ($GeneratedToken) {
    Write-Host "`nA new AUTH_TOKEN has been generated for you:" -ForegroundColor White
    Write-Host "$GeneratedToken" -ForegroundColor Yellow
    Write-Host "`nSave this token for your Home Assistant / curl configuration." -ForegroundColor White
} else {
    $CurrentToken = Select-String -Path $DestEnv -Pattern "AUTH_TOKEN=(.*)" | ForEach-Object { $_.Matches.Groups[1].Value }
    Write-Host "Using existing AUTH_TOKEN: $CurrentToken" -ForegroundColor Yellow
}

Write-Host "`nPress any key to finish..." -ForegroundColor Gray
$null = [System.Console]::ReadKey($true)
