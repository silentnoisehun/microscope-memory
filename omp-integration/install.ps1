<#
  install.ps1 - wire the Microscope Memory engine into omp (oh-my-pi).

  Installs:
    1. the hook       -> ~/.omp/agent/hooks/pre/microscope-memory.ts
    2. the MCP server -> ~/.omp/agent/mcp.json   (model facing memory_* tools)
    3. the bridge cfg -> ~/.omp/agent/microscope-memory-bridge.toml
       (the canonical config with auto_rebuild = false, so a store can never
        start a ~20 s index rebuild inside a hook handler)

  Disables the earlier session-start-only extension so the context is not
  injected twice.

  ASCII only on purpose: PowerShell 5.1 reads BOM-less UTF-8 as ANSI and
  non-ASCII bytes can turn into quote characters and break the parser.

  Usage:
    powershell -ExecutionPolicy Bypass -File omp-integration\install.ps1
    powershell -ExecutionPolicy Bypass -File omp-integration\install.ps1 -Uninstall
#>

[CmdletBinding()]
param(
  [switch]$Uninstall
)

$ErrorActionPreference = 'Stop'

$OmpDir = Join-Path $env:USERPROFILE '.omp\agent'
$HookDir = Join-Path $OmpDir 'hooks\pre'
$HookTarget = Join-Path $HookDir 'microscope-memory.ts'
$McpTarget = Join-Path $OmpDir 'mcp.json'
$BridgeCfg = Join-Path $OmpDir 'microscope-memory-bridge.toml'
$LegacyExt = Join-Path $OmpDir 'extensions\microscope-memory.ts'
$LegacyDisabled = Join-Path $OmpDir 'extensions\microscope-memory.ts.disabled'

$RepoRoot = Split-Path -Parent $PSScriptRoot
$Source = Join-Path $PSScriptRoot 'microscope-memory.ts'
$McpSource = Join-Path $PSScriptRoot 'mcp.json'

# The bridge owns the memory of THIS repo, not the machine wide project
# memory. The canonical D:\codex instance is shared by every other tool and
# its session layer is capped (layer_retention_entries), so an agent that
# writes one entry per turn would push the real history out of it.
$BridgeBase = $env:MICROSCOPE_BRIDGE_BASE
if (-not $BridgeBase) {
  $BridgeBase = 'C:\Users\mater\AppData\Local\microscope-memory\config.toml'
}

$BinCandidates = @(
  (Join-Path $RepoRoot 'target\release\microscope-mem.exe'),
  'C:\Users\mater\microscope-memory-clone\target\release\microscope-mem.exe',
  $env:MICROSCOPE_BIN
)

function Write-Utf8NoBom([string]$Path, [string]$Content) {
  $enc = New-Object System.Text.UTF8Encoding($false)
  [System.IO.File]::WriteAllText($Path, $Content, $enc)
}

Write-Host ''
Write-Host '=== Microscope Memory -> omp bridge installer ==='
Write-Host ''

# --- uninstall ------------------------------------------------------------
if ($Uninstall) {
  foreach ($p in @($HookTarget, $McpTarget, $BridgeCfg)) {
    if (Test-Path $p) {
      Remove-Item -LiteralPath $p -Force
      Write-Host ("removed: " + $p)
    }
  }
  if (Test-Path $LegacyDisabled) {
    Rename-Item -LiteralPath $LegacyDisabled -NewName 'microscope-memory.ts' -Force
    Write-Host 're-enabled the previous extension'
  }
  Write-Host ''
  Write-Host 'uninstall done.'
  exit 0
}

# --- 1. hook --------------------------------------------------------------
if (-not (Test-Path $Source)) {
  Write-Error ("hook source not found: " + $Source)
  exit 1
}
if (-not (Test-Path $HookDir)) {
  New-Item -ItemType Directory -Path $HookDir -Force | Out-Null
}
Copy-Item -LiteralPath $Source -Destination $HookTarget -Force
Write-Host ("hook:      " + $HookTarget)
Write-Host ("           " + (Get-Item $HookTarget).Length + " bytes")

# The previous extension injected the session context as well; two bridges
# would inject it twice.
if (Test-Path $LegacyExt) {
  if (Test-Path $LegacyDisabled) { Remove-Item -LiteralPath $LegacyDisabled -Force }
  Rename-Item -LiteralPath $LegacyExt -NewName 'microscope-memory.ts.disabled' -Force
  Write-Host 'disabled:  the previous extensions/microscope-memory.ts'
}

# --- 2. MCP server for the model -----------------------------------------
if (-not (Test-Path $McpTarget)) {
  Copy-Item -LiteralPath $McpSource -Destination $McpTarget -Force
  Write-Host ("mcp:       " + $McpTarget + " (created)")
} else {
  # Merge into an existing file instead of overwriting it.
  $doc = $null
  try { $doc = Get-Content $McpTarget -Raw | ConvertFrom-Json } catch { $doc = $null }
  if ($null -eq $doc) {
    Copy-Item -LiteralPath $McpSource -Destination $McpTarget -Force
    Write-Host ("mcp:       " + $McpTarget + " (recreated)")
  } else {
    $server = (Get-Content $McpSource -Raw | ConvertFrom-Json).mcpServers.microscope
    if ($null -eq $doc.mcpServers) {
      $doc | Add-Member -NotePropertyName 'mcpServers' -NotePropertyValue ([pscustomobject]@{}) -Force
    }
    $doc.mcpServers | Add-Member -NotePropertyName 'microscope' -NotePropertyValue $server -Force
    Write-Utf8NoBom $McpTarget ($doc | ConvertTo-Json -Depth 10)
    Write-Host ("mcp:       " + $McpTarget + " (merged)")
  }
}

# --- 3. bridge config (auto_rebuild off) ----------------------------------
if (Test-Path $BridgeBase) {
  $cfg = Get-Content $BridgeBase -Raw
  # The canonical config uses CRLF, so the pattern must not anchor on $.
  $cfg = $cfg -replace '(?m)^([ \t]*)auto_rebuild[ \t]*=[ \t]*true', '$1auto_rebuild = false'
  Write-Utf8NoBom $BridgeCfg $cfg
  $check = Select-String -Path $BridgeCfg -Pattern 'auto_rebuild[ \t]*=[ \t]*(true|false)' | Select-Object -First 1
  if ($check -and $check.Line -match 'auto_rebuild[ \t]*=[ \t]*false') {
    Write-Host ("config:    " + $BridgeCfg + "  (from " + $BridgeBase + ", auto_rebuild = false)")
  } else {
    Write-Host ("config:    " + $BridgeCfg + "  (WARNING: auto_rebuild could not be switched off)")
  }
} else {
  Write-Host ("config:    bridge base config not found at " + $BridgeBase)
  Write-Host '           the bridge falls back to MICROSCOPE_CONFIG'
}

# --- 4. verify ------------------------------------------------------------
Write-Host ''
Write-Host '--- verification ---'
$Bin = $BinCandidates | Where-Object { $_ -and (Test-Path $_) } | Select-Object -First 1
if ($Bin) {
  Write-Host ("binary:    " + $Bin)
  $env:MICROSCOPE_CONFIG = $BridgeBase
  $probe = & $Bin auto-context --compact 2>&1 | Out-String
  if ($LASTEXITCODE -eq 0) {
    Write-Host ("engine:    OK (" + $probe.Trim().Length + " chars of auto-context)")
  } else {
    Write-Host ("engine:    FAILED (exit " + $LASTEXITCODE + ")")
    Write-Host $probe
  }
} else {
  Write-Host 'binary:    NOT FOUND - build it first: cargo build --release'
}

Write-Host ''
Write-Host '--- how to verify the wiring ---'
Write-Host '  omp -p "/extensions"      -> the hook is listed'
Write-Host '  omp -p "/mcp list"        -> the microscope server is connected'
Write-Host '  omp -p "/mcp test microscope"'
Write-Host '  omp -p "/mscope-status"   -> bridge and engine diagnostics'
Write-Host '  trace file: C:\Users\mater\.omp\agent\microscope-memory.log'
Write-Host ''
Write-Host 'done. start a new omp session to pick the hook up.'
Write-Host ''
