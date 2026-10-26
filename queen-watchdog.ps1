<#
  QUEEN WATCHDOG — C:\Users\mater\microscope-memory-clone unified supervisor.
  100% C: drive canonical, zero external drive dependencies.
  Zero-Popup Headless Execution (CREATE_NO_WINDOW).
#>
$ErrorActionPreference = 'Continue'

# --- Canonical C: Paths --------------------------------------------------------
$Root        = 'C:\Users\mater\microscope-memory-clone'
$HopeBin     = 'C:\Users\mater\.hope\bin'
$QueenExe    = Join-Path $HopeBin 'omega-master.exe'
$DroneExe    = Join-Path $HopeBin 'unified-mind.exe'
$StateDir    = Join-Path $Root '.octopus-rust'
$LogDir      = Join-Path $StateDir 'watchdog'
$RegFile     = Join-Path $Root '.bio-drones.json'
$QueenPort   = 8888

$StratosExe  = Join-Path $HopeBin 'stratos-server.exe'
$StratosData = Join-Path $Root 'stratos_data'
$StratosPort = 8090
$StratosLog  = Join-Path $HopeBin 'stratos-server.log'
$StaleSeconds = 210

New-Item -ItemType Directory -Force -Path $LogDir | Out-Null
$LogFile = Join-Path $LogDir ('watchdog-{0:yyyy-MM-dd}.log' -f (Get-Date))

function Write-Log {
    param([string]$Message, [string]$Level = 'INFO')
    $line = '{0:HH:mm:ss} [{1}] {2}' -f (Get-Date), $Level, $Message
    Add-Content -Path $LogFile -Value $line -Encoding UTF8
}

function Start-Headless {
    param(
        [string]$FilePath,
        [string[]]$ArgumentList = @(),
        [string]$WorkingDirectory = $Root,
        [string]$RedirectOut = $null,
        [string]$RedirectErr = $null
    )
    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName = $FilePath
    $psi.Arguments = ($ArgumentList -join ' ')
    $psi.WorkingDirectory = $WorkingDirectory
    $psi.UseShellExecute = $false
    $psi.CreateNoWindow = $true
    if ($RedirectOut) {
        $psi.RedirectStandardOutput = $true
    }
    if ($RedirectErr) {
        $psi.RedirectStandardError = $true
    }
    try {
        $proc = [System.Diagnostics.Process]::Start($psi)
        return $proc
    } catch {
        Write-Log "Start-Headless FAILED: $FilePath -- $($_.Exception.Message)" 'ERROR'
        return $null
    }
}

function Test-QueenAlive {
    try {
        $udp = Get-NetUDPEndpoint -LocalPort $QueenPort -ErrorAction Stop
        return ($null -ne $udp)
    } catch {
        return $false
    }
}

function Get-DroneState {
    $result = @{ Registered = $false; AgeSeconds = -1; Name = '' }
    if (-not (Test-Path $RegFile)) { return $result }
    try {
        $raw = Get-Content $RegFile -Raw -Encoding UTF8
        if ([string]::IsNullOrWhiteSpace($raw)) { return $result }
        $json = ConvertFrom-Json -InputObject $raw -ErrorAction Stop
        if ($null -eq $json) { return $result }
    } catch {
        Write-Log "registry UNPARSEABLE -- treating as no drone: $($_.Exception.Message)" 'ERROR'
        return $result
    }

    foreach ($prop in $json.PSObject.Properties) {
        $entry = $prop.Value
        if ($null -eq $entry) { continue }
        $result.Registered = $true
        $result.Name = $prop.Name
        $hb = 0
        if ($entry.PSObject.Properties.Name -contains 'last_heartbeat') {
            $hb = [int64]$entry.last_heartbeat
        }
        if ($hb -gt 0) {
            $ageMs = [DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds() - $hb
            $result.AgeSeconds = [int]($ageMs / 1000)
        }
        break
    }
    return $result
}

function Start-Queen {
    Write-Log 'queen MISSING on UDP 8888 -- starting' 'WARN'
    $env:BIO_QUEEN_ADDR = '127.0.0.1:8888'
    Start-Headless -FilePath $QueenExe -ArgumentList @('--state-dir', $Root, 'start') -WorkingDirectory $Root | Out-Null
    Start-Sleep -Seconds 6
}

function Start-Drone {
    Write-Log 'drone NOT REGISTERED -- starting unified-mind serve' 'WARN'
    $env:BIO_QUEEN_ADDR   = '127.0.0.1:8888'
    $env:BIO_SKILLS_DIR   = $HopeBin
    $env:BIO_METRICS_DIR  = $Root
    $queenKey = Join-Path $Root '.bio-queen.key'
    if (Test-Path $queenKey) {
        $env:BIO_QUEEN_KEY = $queenKey
    } else {
        Write-Log "queen key MISSING at $queenKey -- drone will be unsigned" 'WARN'
    }
    Start-Headless -FilePath $DroneExe -ArgumentList @('serve') -WorkingDirectory $Root | Out-Null
    Start-Sleep -Seconds 10
}

function Start-Stratos {
    Write-Log 'stratos MISSING on port 8090 -- starting (spine writer)' 'WARN'
    if (-not (Test-Path $StratosExe)) {
        Write-Log "stratos binary MISSING at $StratosExe -- skipping" 'ERROR'
        return
    }
    $running = Get-CimInstance Win32_Process -Filter "Name='stratos-server.exe'" -ErrorAction SilentlyContinue
    if ($running) {
        Write-Log "stratos process alive (pid=$($running.ProcessId)) but health failed -- waiting, not restarting" 'WARN'
        return
    }
    $stratosArgs = @(
        '--data-dir', $StratosData,
        '--addr', "127.0.0.1:$StratosPort",
        '--repo-root', $Root,
        '--hardware-mode', 'mock',
        '--ollama-host', 'http://127.0.0.1:11434',
        '--small-model', 'qwen2.5:3b',
        '--large-model', 'qwen2.5:7b',
        '--num-ctx', '2048',
        '--router-url', 'http://127.0.0.1:10100/v1',
        '--router-small-model', 'nvidia/nvidia/nemotron-3.5-lightning-30b-a3b',
        '--router-large-model', 'nvidia/nvidia/nemotron-3-ultra-550b-a55b'
    )
    Start-Headless -FilePath $StratosExe -ArgumentList $stratosArgs -WorkingDirectory $Root | Out-Null
    Start-Sleep -Seconds 18
}

function Test-StratosAlive {
    try {
        $resp = Invoke-WebRequest -Uri "http://127.0.0.1:$StratosPort/api/health" `
            -TimeoutSec 4 -UseBasicParsing -ErrorAction Stop
        return ($resp.StatusCode -eq 200)
    } catch {
        return $false
    }
}

$RouterPort = 10100
$RouterHealthModel = 'nvidia/nvidia/nemotron-3.5-lightning-30b-a3b'

function Test-RouterAlive {
    try {
        $body = @{
            model      = $RouterHealthModel
            messages   = @(@{ role = 'user'; content = 'ping' })
            max_tokens = 1
        } | ConvertTo-Json -Depth 5
        $resp = Invoke-WebRequest -Uri "http://127.0.0.1:$RouterPort/v1/chat/completions" `
            -Method POST -Body $body -ContentType 'application/json' `
            -TimeoutSec 25 -UseBasicParsing -ErrorAction Stop
        return ($resp.StatusCode -eq 200)
    } catch {
        return $false
    }
}

function Start-Router {
    Write-Log "router MISSING on port $RouterPort -- starting opencodex" 'WARN'
    $bunExe = 'C:\Users\mater\AppData\Roaming\npm\node_modules\@bitkyc08\opencodex\node_modules\bun\bin\bun.exe'
    $ocxCli = 'C:\Users\mater\AppData\Roaming\npm\node_modules\@bitkyc08\opencodex\src\cli\index.ts'
    if (-not (Test-Path $bunExe)) {
        Write-Log "router bun MISSING at $bunExe -- skipping" 'ERROR'
        return
    }
    Start-Headless -FilePath $bunExe `
        -ArgumentList @($ocxCli, 'start', '--port', "$RouterPort") `
        -WorkingDirectory 'C:\Users\mater\AppData\Roaming\npm\node_modules\@bitkyc08\opencodex' | Out-Null
    Start-Sleep -Seconds 10
}

# --- MODELL-ROUTER (10300) ---------------------------------------------------
# A hullám/wave-echo a MODELL-ROUTER szolgáltatástól (10300) kapja a
# modell-listát. Ez a service a 10100-as opencodex-től FÜGGETLEN — ha
# leáll, a kar-generálás a statikus poolra esik, és 404-es (nem
# telepített) modelleket éget.
$ModelRouterPort = 10300
$ModelRouterExe = Join-Path $HopeBin 'model-router.exe'
$ModelRouterState = 'C:\Users\mater\.hope\model-router-state.json'

function Test-ModelRouterAlive {
    try {
        # 15 mp a timeout: a router a probe közben Mutex-zárolja az állapotot,
        # és a probe több modellt is ellenőriz — 5 mp alatt a /health nem
        # mindig válaszol, pedig a service él (fals pozitív "MISSING").
        $resp = Invoke-WebRequest -Uri "http://127.0.0.1:$ModelRouterPort/health" `
            -TimeoutSec 15 -UseBasicParsing -ErrorAction Stop
        return ($resp.StatusCode -eq 200)
    } catch {
        return $false
    }
}

function Start-ModelRouter {
    Write-Log "model-router MISSING on port $ModelRouterPort -- starting" 'WARN'
    if (-not (Test-Path $ModelRouterExe)) {
        Write-Log "model-router binary MISSING at $ModelRouterExe -- skipping" 'ERROR'
        return
    }
    # Ha a process él, de a health check nem válaszol: nem indítunk új
    # példányt (a port még mindig fogva lehet), csak várunk.
    $running = Get-CimInstance Win32_Process -Filter "Name='model-router.exe'" -ErrorAction SilentlyContinue
    if ($running) {
        Write-Log "model-router process alive (pid=$($running.ProcessId)) but health failed -- waiting, not restarting" 'WARN'
        return
    }
    Start-Headless -FilePath $ModelRouterExe `
        -ArgumentList @('serve', "127.0.0.1:$ModelRouterPort", $ModelRouterState) `
        -WorkingDirectory $HopeBin `
        -RedirectOut (Join-Path $HopeBin 'model-router.log') `
        -RedirectErr (Join-Path $HopeBin 'model-router.log.err') | Out-Null
    Start-Sleep -Seconds 4
}

$LockFile = Join-Path $LogDir 'watchdog.lock'
try {
    $lock = [System.IO.File]::Open($LockFile, 'OpenOrCreate', 'Write', 'None')
} catch {
    Write-Host 'queen-watchdog already running (lock held); exiting.'
    exit 0
}
try {
    $bytes = [System.Text.Encoding]::ASCII.GetBytes("pid=$PID started=$(Get-Date -Format s)")
    $lock.Write($bytes, 0, $bytes.Length)
    $lock.Flush()
} catch { }

Write-Log "watchdog STARTED pid=$PID staleThreshold=${StaleSeconds}s (C: Canonical)"

try {
    while ($true) {
        # 1. Queen
        if (Test-QueenAlive) {
            Write-Log 'queen ok (udp 8888 held)'
        } else {
            Start-Queen
            if (Test-QueenAlive) {
                Write-Log 'queen recovered on udp 8888' 'INFO'
            } else {
                Write-Log 'queen FAILED to claim udp 8888 -- will retry next tick' 'ERROR'
            }
        }

        # 2. Drone
        $serveProc = Get-CimInstance Win32_Process -Filter "Name='unified-mind.exe'" `
            -ErrorAction SilentlyContinue | Where-Object { $_.CommandLine -like '*serve*' }

        $drone = Get-DroneState
        if ($null -eq $serveProc) {
            Start-Drone
            $serveProc = Get-CimInstance Win32_Process -Filter "Name='unified-mind.exe'" `
                -ErrorAction SilentlyContinue | Where-Object { $_.CommandLine -like '*serve*' }
            if ($null -ne $serveProc) {
                Write-Log "drone RESTARTED (no serve process) pid=$($serveProc.ProcessId)" 'WARN'
            } else {
                Write-Log 'drone FAILED to start -- will retry next tick' 'ERROR'
            }
        }
        elseif (-not $drone.Registered) {
            Write-Log 'drone process up but NOT registered with the queen yet -- waiting' 'WARN'
        }
        elseif ($drone.AgeSeconds -gt $StaleSeconds) {
            Write-Log ("drone heartbeat STALE ({0}s > {1}s) -- confirming before kill" -f $drone.AgeSeconds, $StaleSeconds) 'WARN'
            Start-Sleep -Seconds 65
            $recheck = Get-DroneState
            if (-not $recheck.Registered -or $recheck.AgeSeconds -le $StaleSeconds) {
                Write-Log ("drone heartbeat RECOVERED ({0}s) -- no kill (single-sample false positive)" -f $recheck.AgeSeconds) 'INFO'
            } else {
                Write-Log ("drone heartbeat STILL STALE after 65s ({0}s) -- killing" -f $recheck.AgeSeconds) 'ERROR'
                $serveProc | ForEach-Object {
                    Write-Log "stopping hung serve process pid=$($_.ProcessId)" 'WARN'
                    Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue
                }
                Start-Sleep -Seconds 3
                Start-Drone
            }
        }
        else {
            Write-Log ("drone ok '{0}' pid={1} last_hb={2}s" -f $drone.Name, $serveProc.ProcessId, $drone.AgeSeconds)
        }

        # 3. Octopus state directory
        if (-not (Test-Path $StateDir)) {
            Write-Log 'octopus state dir MISSING -- recreating' 'WARN'
            New-Item -ItemType Directory -Force -Path $StateDir | Out-Null
        }

        # 4. STRATOS
        if (Test-StratosAlive) {
            Write-Log 'stratos ok (spine writer online)'
        } else {
            Start-Stratos
            if (Test-StratosAlive) {
                Write-Log 'stratos RECOVERED on port 8090' 'INFO'
            } else {
                Write-Log 'stratos FAILED to claim port 8090 -- will retry next tick' 'ERROR'
            }
        }

        # 5. MODEL ROUTER (opencodex, 10100)
        if (Test-RouterAlive) {
            Write-Log 'router ok (model provider responding)'
        } else {
            Start-Router
            if (Test-RouterAlive) {
                Write-Log "router RECOVERED on port $RouterPort" 'INFO'
            } else {
                Write-Log "router FAILED to respond on port $RouterPort -- will retry next tick" 'ERROR'
            }
        }

        # 6. MODELL-ROUTER (10300) — a hullám/wave-echo modell-választása
        if (Test-ModelRouterAlive) {
            Write-Log 'model-router ok (10300 serving)'
        } else {
            Start-ModelRouter
            if (Test-ModelRouterAlive) {
                Write-Log "model-router RECOVERED on port $ModelRouterPort" 'INFO'
            } else {
                Write-Log "model-router FAILED to respond on port $ModelRouterPort -- will retry next tick" 'ERROR'
            }
        }

        Start-Sleep -Seconds 30
    }
}
finally {
    try { $lock.Close() } catch { }
    Write-Log 'watchdog STOPPED'
}
