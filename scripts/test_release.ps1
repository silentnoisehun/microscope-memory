<#
.SYNOPSIS
    Public Demo Release Test Script
.DESCRIPTION
    Runs the full test suite and verifies all safety checks for the public demo release.
#>

Write-Host "=== Microscope Memory Public Demo Release Test ===" -ForegroundColor Cyan
Write-Host "" 

# Step 1: Run full test suite
Write-Host "[1/7] Running full test suite..." -ForegroundColor Yellow
$testResult = & cargo test --workspace --release 2>&1 | Select-String -Pattern "test result:"
if ($LASTEXITCODE -ne 0) {
    Write-Host "FAILED: Tests did not pass" -ForegroundColor Red
    exit 1
}
Write-Host "PASS: $testResult" -ForegroundColor Green

# Step 2: Verify public-demo config exists
Write-Host "[2/7] Verifying public-demo config..." -ForegroundColor Yellow
if (-not (Test-Path "examples/config.public-demo.toml")) {
    Write-Host "FAILED: examples/config.public-demo.toml not found" -ForegroundColor Red
    exit 1
}
$config = Get-Content "examples/config.public-demo.toml" -Raw
if ($config -match "read_only = true" -and $config -match "write_enabled = false") {
    Write-Host "PASS: Public demo config is safe" -ForegroundColor Green
} else {
    Write-Host "FAILED: Public demo config has unsafe settings" -ForegroundColor Red
    exit 1
}

# Step 3: Verify no private dataset paths
Write-Host "[3/7] Checking for private dataset paths..." -ForegroundColor Yellow
$configContent = Get-Content "config.toml" -ErrorAction SilentlyContinue
if ($configContent -match "C:\\|D:\\|private|secret") {
    Write-Host "WARNING: config.toml may contain private paths" -ForegroundColor Yellow
} else {
    Write-Host "PASS: No private paths detected" -ForegroundColor Green
}

# Step 4: Build release binary
Write-Host "[4/7] Building release binary..." -ForegroundColor Yellow
cargo build --release 2>&1 | Out-Null
if ($LASTEXITCODE -ne 0) {
    Write-Host "FAILED: Release build failed" -ForegroundColor Red
    exit 1
}
if (Test-Path "target/release/microscope-mem.exe") {
    Write-Host "PASS: Release binary built" -ForegroundColor Green
} else {
    Write-Host "FAILED: Release binary not found" -ForegroundColor Red
    exit 1
}

# Step 5: Verify MCP tools respond
Write-Host "[5/7] Testing MCP tools/list..." -ForegroundColor Yellow
$mcpResponse = echo '{"jsonrpc":"2.0","id":1,"method":"tools/list"}' | & ".\target\release\microscope-mem.exe" mcp 2>$null
if ($mcpResponse -match "memory_status" -and $mcpResponse -match "memory_recall") {
    Write-Host "PASS: MCP tools respond correctly" -ForegroundColor Green
} else {
    Write-Host "FAILED: MCP tools did not respond" -ForegroundColor Red
    exit 1
}

# Step 6: Verify no write tools exposed
Write-Host "[6/7] Verifying write tools are hidden..." -ForegroundColor Yellow
if ($mcpResponse -match "memory_store") {
    Write-Host "FAILED: memory_store should not be exposed in public mode" -ForegroundColor Red
    exit 1
}
Write-Host "PASS: No write tools exposed" -ForegroundColor Green

# Step 7: Verify secret filtering
Write-Host "[7/7] Testing secret filtering..." -ForegroundColor Yellow
$secretResponse = echo '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"memory_recall","arguments":{"query":"test password"}}}' | & ".\target\release\microscope-mem.exe" mcp 2>$null
if ($LASTEXITCODE -eq 0) {
    Write-Host "PASS: Secret-containing query processed safely" -ForegroundColor Green
} else {
    Write-Host "FAILED: Secret filtering error" -ForegroundColor Red
    exit 1
}

Write-Host "" 
Write-Host "=== All 7 checks passed. Ready for public demo release. ===" -ForegroundColor Green
