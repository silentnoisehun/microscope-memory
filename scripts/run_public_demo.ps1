<#
.SYNOPSIS
    Run Microscope Memory as a public demo MCP server
.DESCRIPTION
    Memory is not a static vault.
    Memory is a living structure.

    ARCHITECTURAL MEMORY · PUBLIC DEMO

    T0      Fresh safe index
    T100    Clusters forming
    T1000   Context fields crystallize
    T5000   Public demo locked

    Starts the MCP server with public-demo configuration.
    Read-only mode, no writes, safe for public use.
#>

Write-Host "=== ARCHITECTURAL MEMORY · PUBLIC DEMO ===" -ForegroundColor Cyan
Write-Host "Memory is not a static vault." -ForegroundColor White
Write-Host "Memory is a living structure." -ForegroundColor White
Write-Host "" 

# Check for config
$configPath = "examples/config.public-demo.toml"
if (-not (Test-Path $configPath)) {
    Write-Host "ERROR: $configPath not found" -ForegroundColor Red
    exit 1
}

# Check for binary
$binaryPath = "target/release/microscope-mem.exe"
if (-not (Test-Path $binaryPath)) {
    Write-Host "Building release binary..." -ForegroundColor Yellow
    cargo build --release 2>&1 | Out-Null
    if ($LASTEXITCODE -ne 0) {
        Write-Host "ERROR: Build failed" -ForegroundColor Red
        exit 1
    }
}

Write-Host "Configuration: $configPath" -ForegroundColor Green
Write-Host "Binary: $binaryPath" -ForegroundColor Green
Write-Host "Mode: READ-ONLY (public demo)" -ForegroundColor Yellow
Write-Host "Writes: DISABLED" -ForegroundColor Yellow
Write-Host "" 

# Set environment and run
$env:MICROSCOPE_CONFIG = (Resolve-Path $configPath).Path

Write-Host "T0 — Starting MCP server on stdio..." -ForegroundColor Cyan
Write-Host "Connect your MCP client to this process." -ForegroundColor Cyan
Write-Host "" 

& $binaryPath mcp
