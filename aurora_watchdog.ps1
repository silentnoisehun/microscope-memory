$ErrorActionPreference = 'SilentlyContinue'
# === EGYPELDANY-VEDELEM: ha mar fut egy watchdog, ez a peldany azonnal kilep ===
$mutex = New-Object System.Threading.Mutex($false, 'Global\AuroraWatchdogMutex'); if ($mutex -and -not $mutex.WaitOne(0)) { exit 0 }

$exe = 'C:\Users\mater\.hope\bin\hchat.exe'
$backend = 'gemini'
$model = 'gemini-3.8-flash'
$logDir = Join-Path $env:LOCALAPPDATA 'Hope'
$logFile = Join-Path $logDir 'aurora_watchdog.log'
$pauseFile = Join-Path $logDir 'watchdog.paused'

if (-not (Test-Path $logDir)) {
    New-Item -ItemType Directory -Path $logDir | Out-Null
}

function Write-WatchdogLog {
    param([string]$Message)
    $timestamp = Get-Date -Format 'yyyy-MM-dd HH:mm:ss'
    Add-Content -Path $logFile -Value "[$timestamp] $Message"
}

# Headless Process Launcher -- CreateNoWindow=true, zero conhost flash.
function Start-Headless {
    param(
        [string]$FilePath,
        [string[]]$ArgumentList = @(),
        [string]$WorkingDirectory = '.'
    )
    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName = $FilePath
    $psi.Arguments = ($ArgumentList -join ' ')
    $psi.WorkingDirectory = $WorkingDirectory
    $psi.UseShellExecute = $false
    $psi.CreateNoWindow = $true
    try {
        $proc = [System.Diagnostics.Process]::Start($psi)
        return $proc
    } catch {
        Write-WatchdogLog "Start-Headless FAILED: $FilePath -- $($_.Exception.Message)"
        return $null
    }
}

function Test-AuroraRunning {
    # FORRÁS: a daemon SAJÁT mutex-e (ugyanaz, amit a hchat --daemon lefoglal).
    # Nem WMI: a hosszú életű watchdog processz WMI-je émiósan elhasal
    # (a Get-CimInstance néma hiba miatt nullát ad vissza -> a watchdog 30 mpente
    # feleslegesen újraindít, a második példány pedig a mutex miatt azonnal hal).
    # Az OpenMutex NEM hoz létre semmit, így nincs race a daemon indulásával.
    if (-not ('WdMutex' -as [type])) {
        Add-Type -Namespace WdMutex -Name Native -MemberDefinition @'
[DllImport("kernel32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
public static extern IntPtr OpenMutex(uint dwDesiredAccess, bool bInheritHandle, string lpName);
[DllImport("kernel32.dll")]
public static extern bool CloseHandle(IntPtr hObject);
'@ -ErrorAction SilentlyContinue
    }
    try {
        # MUTEX_ALL_ACCESS = 0x001F0001
        $h = [WdMutex.Native]::OpenMutex(0x001F0001, $false, 'Global\AuroraHopeDaemonMutex')
        if ($h -ne [IntPtr]::Zero) {
            [void][WdMutex.Native]::CloseHandle($h)
            return $true
        }
        return $false
    } catch {
        # tartalék a WMI-re, DE a hibát most már kiírjuk (a régi néma elnyelte)
        $process = Get-CimInstance Win32_Process -Filter "Name='hchat.exe'" -ErrorAction SilentlyContinue
        return (($process | Where-Object { $_.CommandLine -match '--daemon' }) -ne $null)
    }
}


# ¦¦ GENOME / SILENT WORKER felugyeles ¦¦¦¦¦¦¦¦¦¦¦¦¦¦¦¦¦¦¦¦¦¦¦¦¦¦¦¦¦¦¦¦¦¦¦¦¦
# A `hope_py_genome_*` tool-ok (status/verify/rules/audit) egy gRPC szerverre
# hivatkoznak a localhost:50051-on. Ha ez haldokolik, a Silent Worker fék
# hangan marad — ezért a watchdog MELLETTE is figyeli.
$hopeExe = 'C:\Users\mater\.hope\bin\hope.exe'
$hopeDir = Split-Path -Parent $hopeExe

function Test-GenomeRunning {
    return (@(Get-NetTCPConnection -LocalPort 50051 -State Listen -ErrorAction SilentlyContinue).Count -gt 0)
}

function Start-GenomeIfDead {
    if (Test-GenomeRunning) { return }
    if (-not (Test-Path $hopeExe)) { return }
    $env:RUST_LOG = 'error'
    Start-Headless -FilePath $hopeExe -ArgumentList @('serve') -WorkingDirectory $hopeDir
    Start-Sleep -Seconds 4
    if (Test-GenomeRunning) {
        Write-WatchdogLog 'Genome gRPC (50051) ujrainditva - hope.exe serve.'
    } else {
        Write-WatchdogLog 'Genome gRPC (50051) NEM indult el - a py-genome_* toolok halottak.'
    }
}

# --- CODEX VOICE PIPELINE (C:\Users\mater\microscope-memory-clone\voice-pipeline) -----------------------
# A PWA a hanggeneralast innen kéri (localhost:50059/speak_sync, hu-HU-NoemiNeural).
# Ha ez haldokolik, a felolvasas a Rust /tts-re esik vissza -- ezert a
# watchdog MELLETTE is figyeli, a genomhoz hasonloan.
$voiceDir = 'C:\Users\mater\microscope-memory-clone\voice-pipeline'
$voiceScript = Join-Path $voiceDir 'hope_server.py'
$voicePort = 50059

function Test-VoicePipeline {
    # a CSARDAKHOZ nem a processz nevet nezem: ha a porton hall valami,
    # a szolgaltatas megvan. (A ketto egyutt a duplikatumot is kiszuri.)
    return (@(Get-NetTCPConnection -LocalPort $voicePort -State Listen -ErrorAction SilentlyContinue).Count -gt 0)
}

function Start-VoicePipelineIfDead {
    if (Test-VoicePipeline) { return }
    # a pipeline sajat tiltofaja (a Rust is ezt ellenorzi)
    if (Test-Path (Join-Path $voiceDir '_mute')) { return }
    if (-not (Test-Path $voiceScript)) {
        Write-WatchdogLog 'Codex voice pipeline NINCS (hope_server.py hiányzik) - a PWA Rust /tts-re esik vissza.'
        return
    }
    $py = (Get-Command python -ErrorAction SilentlyContinue)
    if (-not $py) { $py = (Get-Command py -ErrorAction SilentlyContinue) }
    if (-not $py) {
        Write-WatchdogLog 'Codex voice pipeline: nincs python a PATH-ban - nem indithato.'
        return
    }
    Start-Headless -FilePath $py.Source -ArgumentList @($voiceScript) -WorkingDirectory $voiceDir | Out-Null
    # az edge-tts elindulasa lassabb, ezert hosszabb varno
    $ok = $false
    1..8 | ForEach-Object {
        Start-Sleep -Milliseconds 900
        if (Test-VoicePipeline) { $ok = $true }
    }
    if ($ok) {
        Write-WatchdogLog "Codex voice pipeline ($voicePort) ujrainditva - hope_server.py."
    } else {
        Write-WatchdogLog "Codex voice pipeline ($voicePort) NEM indult el - a PWA Rust /tts-re esik vissza."
    }
}

# Kornyezeti valtozok a registry-bol (hogy az inditott folyamat is lassa oket)
$env:GEMINI_API_KEYS = [Environment]::GetEnvironmentVariable('GEMINI_API_KEYS','User')
$env:NASA_API_KEY = [Environment]::GetEnvironmentVariable('NASA_API_KEY','User')
$env:GROQ_API_KEY = [Environment]::GetEnvironmentVariable('GROQ_API_KEY','User')
Write-WatchdogLog 'Watchdog indul (egypeldany-vedelemmel: Global\AuroraWatchdogMutex).'

while ($true) {
    if (Test-Path $pauseFile) {
        # Hot-swap folyamatban: a watchdog nem indit es nem ol (a szkript kezeli)
        Start-Sleep -Seconds 5
        continue
    }
    if (-not (Test-AuroraRunning)) {
        Write-WatchdogLog 'Aurora daemon nem talalhato - ujrainditas...'
        $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName = $exe
    $psi.Arguments = "--daemon --daemon-backend $backend --daemon-model $model"
    $psi.UseShellExecute = $false
    $psi.CreateNoWindow = $true
    $proc = [System.Diagnostics.Process]::Start($psi)
        # a mutex ellenorzo 2 mp mULVA dönt: ha nem sikerult elindulni (pl. egy
        # masik peldany mar fut), ezt rogtetni kell, nem "elinditva"-t irni
        Start-Sleep -Seconds 2
        if ($proc.HasExited) {
            Write-WatchdogLog "Aurora daemon NEM indult el (exit $($proc.ExitCode)) - a singleton mutex mar foglalt."
        } else {
            Write-WatchdogLog "Aurora daemon elinditva (PID $($proc.Id))."
        }
    }
    Start-GenomeIfDead
    Start-VoicePipelineIfDead
    Start-Sleep -Seconds 30
}


