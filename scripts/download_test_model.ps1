<#
.SYNOPSIS
    Downloads baseline ONNX super-resolution models into the models cache directory.
#>

$CacheDir = Join-Path $PSScriptRoot "..\models\cache"
if (-not (Test-Path $CacheDir)) {
    New-Item -ItemType Directory -Path $CacheDir -Force | Out-Null
}

Write-Host "=== Downloading Baseline ONNX Models ===" -ForegroundColor Cyan

# Model registry URL (Compact / Real-ESRGAN in ONNX format)
$ModelUrl = "https://github.com/xinntao/Real-ESRGAN/releases/download/v0.2.5.0/realesr-general-x4v3.onnx"
$TargetPath = Join-Path $CacheDir "realesrgan-x4plus.onnx"

if (-not (Test-Path $TargetPath)) {
    Write-Host "Downloading Real-ESRGAN ONNX model to $TargetPath..." -ForegroundColor Yellow
    try {
        Invoke-WebRequest -Uri $ModelUrl -OutFile $TargetPath -UserAgent "NativeUpscaler"
        Write-Host "Download complete!" -ForegroundColor Green
    } catch {
        Write-Warning "Could not download directly from release mirror: $_"
        Write-Host "You can place custom ONNX models in: $CacheDir" -ForegroundColor Gray
    }
} else {
    Write-Host "Model already exists at $TargetPath." -ForegroundColor Green
}
