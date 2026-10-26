#!/usr/bin/env pwsh
# ============================================================================
# chain-services.ps1 - Chain's Microscope Memory Service Manager
# ============================================================================
# Manages all background services for Chain's cognitive infrastructure
# ============================================================================

$ErrorActionPreference = "Stop"

$Action = $args[0]
if (-not $Action) {
    Write-Host "Chain Microscope Memory Service Manager"
    Write-Host ""
    Write-Host "Usage: chain-services.ps1 <start|stop|status|restart>"
    Write-Host ""
    Write-Host "Services:"
    Write-Host "  autonomous  - Continuous cognitive loop (30s cycles)"
    Write-Host "  mcp         - Model Context Protocol server (stdio)"
    Write-Host "  spine       - Binary Spine IPC (2048B mmap)"
    Write-Host "  bridge      - REST Bridge API (port 6060)"
    Write-Host "  serve       - 3D Viewer HTTP server"
    exit 0
}

$BaseDir = "C:\Users\mater\microscope-memory-clone"
$Bin = "$BaseDir\target\release\microscope-mem.exe"
$PidDir = "$BaseDir\pids"

if (-not (Test-Path $PidDir)) { New-Item -ItemType Directory -Path $PidDir -Force | Out-Null }

function Start-Service {
    param([string]$Name, [string]$Args, [string]$LogFile = $null)
    $pidFile = "$PidDir\$Name.pid"
    $logFile = if ($LogFile) { $LogFile } else { "$BaseDir\$Name.log" }
    
    if (Test-Path $pidFile) {
        $oldPid = Get-Content $pidFile -ErrorAction SilentlyContinue
        if ($oldPid -and (Get-Process -Id $oldPid -ErrorAction SilentlyContinue)) {
            Write-Host "$Name already running (PID: $oldPid)" -ForegroundColor Yellow
            return
        }
    }
    
    Write-Host "Starting $Name..." -ForegroundColor Cyan
    $proc = Start-Process -FilePath $Bin -ArgumentList $Args -WorkingDirectory $BaseDir `
        -RedirectStandardOutput $logFile -RedirectStandardError "$logFile.err" -PassThru -WindowStyle Hidden
    $proc.Id | Out-File $pidFile
    Write-Host "$Name started (PID: $proc.Id)" -ForegroundColor Green
}

function Stop-Service {
    param([string]$Name)
    $pidFile = "$PidDir\$Name.pid"
    if (-not (Test-Path $pidFile)) {
        Write-Host "$Name not running (no PID file)" -ForegroundColor Yellow
        return
    }
    $pid = Get-Content $pidFile -ErrorAction SilentlyContinue
    if ($pid -and (Get-Process -Id $pid -ErrorAction SilentlyContinue)) {
        Stop-Process -Id $pid -Force
        Write-Host "$Name stopped (PID: $pid)" -ForegroundColor Green
    } else {
        Write-Host "$Name not running (stale PID file)" -ForegroundColor Yellow
    }
    Remove-Item $pidFile -ErrorAction SilentlyContinue
}

function Get-ServiceStatus {
    param([string]$Name)
    $pidFile = "$PidDir\$Name.pid"
    if (-not (Test-Path $pidFile)) { return @{Running=$false; PID=$null} }
    $pid = Get-Content $pidFile -ErrorAction SilentlyContinue
    $proc = Get-Process -Id $pid -ErrorAction SilentlyContinue
    if ($proc) {
        return @{Running=$true; PID=$pid; CPU=$proc.CPU; WS=$proc.WorkingSet64; StartTime=$proc.StartTime}
    } else {
        Remove-Item $pidFile -ErrorAction SilentlyContinue
        return @{Running=$false; PID=$null}
    }
}

switch ($Action) {
    "start" {
        $Service = $args[1]
        if (-not $Service) { Write-Error "start requires <service>"; exit 1 }
        switch ($Service) {
            "autonomous" { Start-Service "autonomous" "autonomous --daemon" }
            "mcp" { Start-Service "mcp" "mcp" }
            "spine" { Start-Service "spine" "spine" }
            "bridge" { Start-Service "bridge" "bridge" }
            "serve" { Start-Service "serve" "serve" }
            "all" {
                Start-Service "autonomous" "autonomous --daemon"
                Start-Service "mcp" "mcp"
                Start-Service "spine" "spine"
                Start-Service "bridge" "bridge"
                Start-Service "serve" "serve"
            }
            default { Write-Error "Unknown service: $Service"; exit 1 }
        }
    }
    "stop" {
        $Service = $args[1]
        if (-not $Service) { Write-Error "stop requires <service>"; exit 1 }
        switch ($Service) {
            "autonomous" { Stop-Service "autonomous" }
            "mcp" { Stop-Service "mcp" }
            "spine" { Stop-Service "spine" }
            "bridge" { Stop-Service "bridge" }
            "serve" { Stop-Service "serve" }
            "all" {
                Stop-Service "autonomous"
                Stop-Service "mcp"
                Stop-Service "spine"
                Stop-Service "bridge"
                Stop-Service "serve"
            }
            default { Write-Error "Unknown service: $Service"; exit 1 }
        }
    }
    "status" {
        $Services = @("autonomous", "mcp", "spine", "bridge", "serve")
        Write-Host "Chain Microscope Memory Services Status" -ForegroundColor Cyan
        Write-Host ""
        foreach ($svc in $Services) {
            $status = Get-ServiceStatus $svc
            if ($status.Running) {
                Write-Host "$svc : RUNNING (PID: $($status.PID), CPU: $($status.CPU), WS: $([math]::Round($status.WS/1MB,1)) MB, Started: $($status.StartTime))" -ForegroundColor Green
            } else {
                Write-Host "$svc : STOPPED" -ForegroundColor Red
            }
        }
    }
    "restart" {
        $Service = $args[1]
        if (-not $Service) { Write-Error "restart requires <service>"; exit 1 }
        Stop-Service $Service
        Start-Sleep -Seconds 2
        Start-Service $Service
    }
    default {
        Write-Error "Unknown action: $Action. Use: start, stop, status, restart"
        exit 1
    }
}

exit 0