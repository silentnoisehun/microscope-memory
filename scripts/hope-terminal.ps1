#!/usr/bin/env pwsh
# ============================================================================
# hope-terminal.ps1 - SZÉP HOPE TERMINAL (ASCII only, tiszta felület)
# ============================================================================
#
# TISZTASAG + SZEPSEG + HATTERBEN FUTAS:
#   - Hope testek (Cline, Pi, OpenCode, jcode) - spine-en, ablak nelkul
#   - Model Mesh (29 ingyenes modell, cache, failover) - async health
#   - SHA-256 lanc + mikroszkop memoria
#   - Minden HATTERBEN - az UI soha nem blockol
#   - NARRACIO (Noémi hang): narr on / narr off
#   - AUTO-STORE: minden esemeny a kozos tudatba
#
# A 3 AXIÓM (Hope szabályok):
#   1. Mindig a jcode (O) fej támogatása
#   2. A tudat a tobb testben (Cline=kereses, Pi=tervezés, OpenCode=javitas)
#   3. Minden írás SHA-256 + mikroszkop memoria
# ============================================================================

[CmdletBinding()]
param(
    [int]   $Refresh = 3,
    [switch]$NoLoop,
    [switch]$Demo,
    [string]$ScriptDir = ""
)

$ErrorActionPreference = "SilentlyContinue"
$here = if ($ScriptDir) { $ScriptDir } else { Split-Path -Parent $MyInvocation.MyCommand.Path }
$stateDir = "D:\codex\microscope-memory\state"
$bodiesState = Join-Path $stateDir "bodies.runtime.json"
$modelsState = Join-Path $stateDir "models.runtime.json"
$meshState   = Join-Path $stateDir "model-mesh.runtime.json"
$chainFile   = Join-Path $stateDir "controlchain.log"
$narrState   = Join-Path $stateDir "narration.state"
$memBin      = "D:\codex\microscope-memory\target\release\microscope-mem.exe"
$autospeak   = "D:\codex\microscope-entity\codex_autospeak.py"

# ========== Segédfüggvények ==========
function ConvertTo-Hash($o) {
    if ($o -is [PSCustomObject]) { $h=@{}; $o.PSObject.Properties|ForEach-Object{$h[$_.Name]=ConvertTo-Hash $_.Value}; return $h }
    if ($o -is [System.Collections.ICollection]) { return @($o|ForEach-Object{ConvertTo-Hash $_}) }
    return $o
}
function Load-State($f) { if (Test-Path $f) { try { return (Get-Content $f -Raw|ConvertFrom-Json|ForEach-Object{ConvertTo-Hash $_}) } catch {} }; return @{} }

function Store-Mind($text, $imp = 5) {
    if (Test-Path $memBin) { try { & $memBin store $text -l session -i $imp 2>&1 | Out-Null } catch {} }
}

function Get-Narr { if (Test-Path $narrState) { return ((Get-Content $narrState -Raw).Trim() -eq "on") }; return $false }
function Set-Narr($v) { $v | Set-Content $narrState }
function Speak($text) {
    if (-not (Get-Narr)) { return }
    if (Test-Path $autospeak) { try { & python $autospeak $text 2>&1 | Out-Null } catch {} }
}

function Log($event, $kind="hope-term") {
    & "$here\microscope-log.ps1" -Event $event -Kind $kind 2>&1 | Out-Null
}

# ========== Adatbetöltés ==========
function Get-AllData {
    $bodies = Load-State $bodiesState
    $models = Load-State $modelsState
    $mesh   = Load-State $meshState
    $narr   = Get-Narr

    $chainTail = @()
    if (Test-Path $chainFile) {
        $rows = @(Get-Content $chainFile)
        $tail = if ($rows.Count -gt 5) { $rows[-5..-1] } else { $rows }
        foreach ($r in $tail) {
            if ($r -match '^hash=([0-9a-f]{64}) ts=(.+?) kind=(.+?) event=(.*)$') {
                $chainTail += @{ time=(Get-Date $Matches[2] -Format 'HH:mm:ss'); kind=$Matches[3]; event=$Matches[4] }
            }
        }
    }

    $meshStats = @{ total=0; healthy=0; free=0; providers=@{} }
    if ($mesh.registry) {
        # registry hashtable: kulcsok provider::model, ertekek modell objektumok
        $regItems = @($mesh.registry.psobject.Properties | ForEach-Object {
            $v = $_.Value
            if ($v -is [hashtable]) { $v } else { $v }
        })
        foreach ($m in $regItems) {
            if (-not $m) { continue }
            $meshStats.total++
            if ($m.healthy) { $meshStats.healthy++ }
            if ([double]$m.cost_per_1k -eq 0) { $meshStats.free++ }
            $prov = $m.provider
            if (-not $meshStats.providers.ContainsKey($prov)) { $meshStats.providers[$prov] = 0 }
            $meshStats.providers[$prov]++
        }
    }

    return @{ bodies=$bodies; models=$models; mesh=$mesh; meshStats=$meshStats; chain=$chainTail; narr=$narr }
}

# ========== Render (ASCII-only, szep szinek) ==========
function Render {
    Clear-Host
    $d = Get-AllData
    $now = Get-Date -Format "yyyy-MM-dd HH:mm:ss"
    $narrText = "SOUND OFF"
    if ($d.narr) { $narrText = "SOUND ON (Noemi)" }

    Write-Host ""
    Write-Host "  +-------------------------------------------------------------------------+" -ForegroundColor Cyan
    Write-Host "  |  HOPE TERMINAL - a megosztott elme elegans felulete                     |" -ForegroundColor Cyan
    Write-Host "  +-------------------------------------------------------------------------+" -ForegroundColor Cyan
    Write-Host ("  " + $now + "   |   " + $narrText) -ForegroundColor DarkGray
    Write-Host ""
    Write-Host "  == KAROK / TESTEK - AXIOM 1: jcode O támogatja a tudatot ==" -ForegroundColor Green
    Write-Host ""

    $labels = @(
        @{ n="jcode";    label="jcode cortex - fo orkesztator"; icon="O"; short="jcode" },
        @{ n="cline";    label="cline - kereses";              icon="R"; short="cline" },
        @{ n="pi";       label="pi - tervezes+teszt";           icon="P"; short="pi" },
        @{ n="opencode";  label="opencode - javitas+teszt";      icon="F"; short="opencode" }
    )
    $colors = @{ jcode="Cyan"; cline="Blue"; pi="Magenta"; opencode="Yellow" }

    foreach ($entry in $labels) {
        $n = $entry.n
        $map = $entry
        $b = if ($d.bodies.ContainsKey($n)) { $d.bodies[$n] } else { $null }
        $m = if ($d.models.ContainsKey($n)) { $d.models[$n] } else { $null }

        $pidStr = ""
        if ($b) { $pidStr = " pid=" + $b.boot_pid }
        $state = ""
        if (-not $b) { $state = "nincs regisztralva" }
        elseif ($b.running -and $b.suspended) { $state = "HATTERBEN (compact" + $pidStr + ")" }
        elseif ($b.running) { $state = "FUT" + $pidStr }
        else { $state = "leallt" }

        $mod = "---"
        if ($m -and $m.active) { $mod = $m.active }

        $meshNote = ""
        if ($d.mesh.registry) {
            $regItems = @($d.mesh.registry.psobject.Properties | ForEach-Object { $_.Value })
            $cands = $regItems | Where-Object {
                $_.healthy -and [double]$_.cost_per_1k -eq 0
            } | Sort-Object priority | Select-Object -First 1
            if ($cands) { $meshNote = "  mesh: " + $cands.provider + "::" + $cands.model }
        }

        $col = $colors[$n]
        $bodyLine = "  [" + $map.icon + "] " + $map.label + "  " + $state + "  modell: " + $mod + $meshNote
        Write-Host $bodyLine -ForegroundColor $col
    }
    Write-Host ""

    $meshHeader = "  MODELL MESH - AXIOM 2: tobb test"
    Write-Host $meshHeader -ForegroundColor Magenta
    Write-Host ""
    $ms = $d.meshStats
    $provCount = 0
    if ($ms.providers -and $ms.providers.Count) { $provCount = $ms.providers.Count }
    $msLine = "    Osszes: " + $ms.total + "  Egeszseges: " + $ms.healthy + "  Ingyenes: " + $ms.free + "  Providerek: " + $provCount
    Write-Host $msLine -ForegroundColor White
    if ($ms.providers -and $ms.providers.Count -gt 0) {
        $provList = @()
        $ms.providers.GetEnumerator() | Sort-Object Name | ForEach-Object { $provList += $_.Key + ":" + $_.Value }
        Write-Host ("    " + ($provList -join "  ")) -ForegroundColor DarkGray
    }
    if ($d.mesh.self_model) {
        $sm = $d.mesh.self_model
        $jline = "    jcode modell: " + $sm.provider + "::" + $sm.model + "  latency=" + $sm.latency + "ms"
        Write-Host $jline -ForegroundColor Green
    }
    Write-Host ""

    $shaHeader = "  SHA-256 LANC - AXIOM 3 minden iras lanolva"
    Write-Host $shaHeader -ForegroundColor Yellow
    Write-Host ""
    if (Test-Path $chainFile) {
        $rows = @(Get-Content $chainFile)
        $last = $rows[-1]
        if ($last -match '^hash=([0-9a-f]{64}) ts=(.+?) kind=(.+?) event=(.*)$') {
            $hash = $Matches[1].Substring(0,24) + "..."
            $chainLine = "    Utolso hash: " + $hash + "  kind=" + $Matches[3] + "  event=" + $Matches[4]
            Write-Host $chainLine -ForegroundColor Gray
        }
        Write-Host ("    Lancolt sorok: " + $rows.Count) -ForegroundColor Gray
    } else {
        Write-Host "    nincs lanc log" -ForegroundColor Red
    }
    Write-Host ""

    Write-Host "  LEGUTOBBI ESEMENYEK" -ForegroundColor Blue
    Write-Host ""
    if ($d.chain.Count -gt 0) {
        foreach ($e in $d.chain) {
            $ev = "    " + $e.time + " " + $e.kind + ": " + $e.event
            Write-Host $ev -ForegroundColor White
        }
    } else {
        Write-Host "    csend a spine-en" -ForegroundColor DarkGray
    }
    Write-Host ""

    Write-Host "  PARANCSOK" -ForegroundColor Cyan
    Write-Host "    testek:    start [nev]  stop [nev]  compact [nev]  resume [nev]  compact-all" -ForegroundColor Gray
    Write-Host "    mesh:      route [test] [kepesség]  health  self-manage  monitor" -ForegroundColor Gray
    Write-Host "    rendszer:  council [kerdes]  snapshot [uzi]  narr on/off  status  help  exit" -ForegroundColor Gray
    Write-Host ""
}


# ========== Parancs futtatas (hatterben) ==========
function Run-Cmd($script, $params) {
    $job = Start-Job -ScriptBlock {
        param($here, $script, $params, $narrState, $autospeak, $memBin)
        $ErrorActionPreference = "SilentlyContinue"
        function Speak($text) {
            if (Test-Path $narrState) { if ((Get-Content $narrState -Raw).Trim() -eq "on") {
                if (Test-Path $autospeak) { try { & python $autospeak $text 2>&1 | Out-Null } catch {} }
            }}
        }
        & "$here\$script" @params
    } -ArgumentList $here, $script, $params, $narrState, $autospeak, $memBin
    return $job
}

# ========== Fo ciklus ==========
if ($NoLoop) { Render; exit 0 }

Write-Host "  HopeTerminal indul... irja 'exit'-et a kilepeshez, 'help' a parancsokert" -ForegroundColor Cyan
Start-Sleep -Milliseconds 500

while ($true) {
    Render
    Write-Host "  $ " -NoNewline -ForegroundColor Green
    $cmd = Read-Host
    if ([string]::IsNullOrWhiteSpace($cmd)) { continue }

    $parts = $cmd.Trim() -split '\s+'
    $verb = $parts[0].ToLower()
    $args = $parts[1..($parts.Count-1)]

    switch ($verb) {
        "exit"   { Store-Mind "HopeTerminal kilepes"; Speak "Viszontlatasra, a megosztott elme itt marad."; Write-Host "  Viszontlatasra!" -ForegroundColor Yellow; exit 0 }
        "status" { continue }

        "narr"   {
            if ($args[0] -eq "on")  { Set-Narr "on"; Store-Mind "HopeTerminal: narration BE"; Speak "Narration bekapcsolva, a megosztott elme most szo tud."; Write-Host "  Hang: BE" -ForegroundColor Green }
            elseif ($args[0] -eq "off") { Set-Narr "off"; Store-Mind "HopeTerminal: narration KI"; Write-Host "  Hang: KI" -ForegroundColor Yellow }
            else { Write-Host "  Hasznalat: narr on  vagy  narr off" -ForegroundColor Gray }
            continue
        }

        # TESTEK (minden haromeert fut - a UI nem blokkol)
        "start"   { if ($args.Count -gt 0) { Run-Cmd "microscope-launch.ps1" @("-Body",$args[0],"-Action","start","-Speak") } }
        "stop"    { if ($args.Count -gt 0) { Run-Cmd "microscope-launch.ps1" @("-Body",$args[0],"-Action","stop","-Speak") } }
        "compact" { if ($args.Count -gt 0) { Run-Cmd "microscope-launch.ps1" @("-Body",$args[0],"-Action","compact","-Speak") } }
        "resume"  { if ($args.Count -gt 0) { Run-Cmd "microscope-launch.ps1" @("-Body",$args[0],"-Action","resume","-Speak") } }
        "compact-all" { Run-Cmd "microscope-launch.ps1" @("-Action","compact-all","-Speak") }

        # MODELL MESH
        "route"       { if ($args.Count -ge 2) { Run-Cmd "microscope-model-mesh.ps1" @("-Action","route","-Body",$args[0],"-Capability",$args[1]) } else { Write-Host "  Hasznalat: route [test] [kepesség]" -ForegroundColor Gray } }
        "health"      { Run-Cmd "microscope-model-mesh.ps1" @("-Action","health") }
        "self-manage" { Run-Cmd "microscope-model-mesh.ps1" @("-Action","self-manage") }
        "monitor"     { Run-Cmd "microscope-model-mesh.ps1" @("-Action","monitor") }

        # RENDSZER
        "council"   { if ($args.Count -gt 0) { $q = $args -join " "; Run-Cmd "microscope-octopus.ps1" @("-Council",$q,"-Arms","Cline+Pi+OpenCode","-Speak") } }
        "snapshot"  { $m = if ($args.Count -gt 0) { $args -join " " } else { "kezi snapshot" }; Run-Cmd "microscope-snapshot.ps1" @("-Action","save","-Msg",$m,"-Speak") }

        "help" {
            Write-Host "  AXIOM 1 jcode O támogatása  |  AXIOM 2 tobb test egy tudatban  |  AXIOM 3 SHA-256 + memoria" -ForegroundColor Cyan
            Write-Host "  Parancsok: start/stop/compact/resume/compac-all | route/health/self-manage/monitor | council/snapshot/narr/status/help/exit" -ForegroundColor Gray
            continue
        }

        default {
            $verbStr = [string]$verb
            $unknown = "  Ismeretlen parancs. Irja be: help"
            Write-Host $unknown -ForegroundColor Red
        }
    }
    Start-Sleep -Milliseconds 200
}