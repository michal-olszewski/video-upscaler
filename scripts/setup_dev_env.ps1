<#
.SYNOPSIS
    Automated developer environment setup script for Video Upscaler Studio on Windows.
.DESCRIPTION
    Checks and installs Rust (rustup), Node.js (via nvm or winget), and FFmpeg with NVENC/AMF support.
#>

Write-Host "=== Video Upscaler Studio: Toolchain Setup ===" -ForegroundColor Cyan

# 1. Check Winget
if (-not (Get-Command winget -ErrorAction SilentlyContinue)) {
    Write-Error "Windows Package Manager (winget) is required."
    exit 1
}

# 2. Check Rust
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-Host "[1/3] Rust toolchain not found. Installing Rust via winget..." -ForegroundColor Yellow
    winget install --id Rustlang.Rustup -e --accept-source-agreements --accept-package-agreements
    Write-Host "Rust installed. Please restart your terminal after completion to refresh PATH." -ForegroundColor Green
} else {
    Write-Host "[1/3] Rust toolchain is installed: $(cargo --version)" -ForegroundColor Green
}

# 3. Check Node.js
if (-not (Get-Command node -ErrorAction SilentlyContinue)) {
    Write-Host "[2/3] Node.js not found. Installing Node.js LTS via winget..." -ForegroundColor Yellow
    winget install --id OpenJS.NodeJS.LTS -e --accept-source-agreements --accept-package-agreements
    Write-Host "Node.js installed." -ForegroundColor Green
} else {
    Write-Host "[2/3] Node.js is installed: $(node --version)" -ForegroundColor Green
}

# 4. Check FFmpeg
if (-not (Get-Command ffmpeg -ErrorAction SilentlyContinue)) {
    Write-Host "[3/3] FFmpeg not found. Installing FFmpeg via winget..." -ForegroundColor Yellow
    winget install --id Gyan.FFmpeg -e --accept-source-agreements --accept-package-agreements
    Write-Host "FFmpeg installed." -ForegroundColor Green
} else {
    Write-Host "[3/3] FFmpeg is installed." -ForegroundColor Green
}

Write-Host "`nSetup complete! You can build the project with:" -ForegroundColor Cyan
Write-Host "  cargo build --workspace" -ForegroundColor White
Write-Host "  cd apps/desktop/ui && npm install && npm run dev" -ForegroundColor White
