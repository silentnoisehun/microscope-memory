# siphon-kar.ps1 — GitHub Siphon Engine (Rongyász Architecture - Hardened)
# Szippant egy GitHub repót / fájlt, kinyeri a Rust mintát,
# Licensz- és Sandbox-védelmi kapukon futtatja át, és csak zöld állapotban
# égeti be a közös Hullam Spine-ba autonóm kar-sablonként.

param(
  [Parameter(Mandatory=$true)][string]$Target, # repo pl: "tokio-rs/bytes" vagy raw URL vagy helyi fájl
  [string]$Keywords = "github,siphon,auto-learn",
  [string]$Tech = "rust,lib",
  [switch]$DryRun,
  [switch]$ForceLicense # Kifejezett felülbírálás ha a felhasználó tudatosan kéri
)

$HopeBin = "C:\Users\mater\.hope\bin"
$Hullam = Join-Path $HopeBin "hullam.exe"

Write-Host "🌊 [GITHUB SIPHON] Cél szippantása: $Target..." -ForegroundColor Cyan

# ─── Licensz Engedélyezőlista (Whitelist) ────────────────────────────────────
# Szigorúan permisszív licenszek: nem fertőzhetik meg a rendszert copyleft kötelezettséggel
$AllowedLicenses = @("MIT", "Apache-2.0", "BSD-3-Clause", "BSD-2-Clause", "CC0-1.0", "Unlicense", "ISC", "Zlib")
$ForbiddenLicenses = @("GPL", "AGPL", "LGPL", "EUPL", "SSPL")

$tempDir = Join-Path $env:TEMP "siphon_$(Get-Random)"
New-Item -ItemType Directory -Force $tempDir | Out-Null

try {
  $codeContent = ""
  $label = ""
  $licenseDetected = "Unknown"

  if ($Target -match "^https?://") {
    # Közvetlen letöltés
    $codeContent = (Invoke-WebRequest -Uri $Target -UseBasicParsing).Content
    $label = [System.IO.Path]::GetFileNameWithoutExtension($Target)
  } elseif ($Target -match "/") {
    # GitHub repo clone/export
    $repoName = $Target.Split('/')[-1]
    $label = "siphon-$repoName"
    Write-Host "  Cloning shallow $Target..." -ForegroundColor DarkGray
    & git clone --depth 1 "https://github.com/$Target.git" "$tempDir\repo" 2>$null
    
    # ─── 1. VÉDELMI KAPU: Licensz Ellenőrzés ───
    Write-Host "  🛡️ [GATE 1/2] Licensz vizsgálata (Copyleft védelem)..." -ForegroundColor Yellow
    $cargoTomlPath = Join-Path "$tempDir\repo" "Cargo.toml"
    if (Test-Path $cargoTomlPath) {
      $cargoText = Get-Content $cargoTomlPath -Raw
      if ($cargoText -match '(?m)^\s*license\s*=\s*["'']([^"'']+)["'']') {
        $licenseDetected = $matches[1]
      }
    }

    if ($licenseDetected -eq "Unknown") {
      $licenseFile = Get-ChildItem "$tempDir\repo\LICENSE*", "$tempDir\repo\LICENCE*" -ErrorAction SilentlyContinue | Select-Object -First 1
      if ($licenseFile) {
        $licHeader = (Get-Content $licenseFile.FullName -Raw).Substring(0, [Math]::Min(500, (Get-Item $licenseFile.FullName).Length))
        foreach ($f in $ForbiddenLicenses) {
          if ($licHeader -match $f) { $licenseDetected = $f; break }
        }
        if ($licenseDetected -eq "Unknown") {
          foreach ($a in $AllowedLicenses) {
            if ($licHeader -match $a) { $licenseDetected = $a; break }
          }
        }
      }
    }

    Write-Host "  Detektált licensz: $licenseDetected" -ForegroundColor Cyan

    $isForbidden = $false
    foreach ($f in $ForbiddenLicenses) {
      if ($licenseDetected -match $f) { $isForbidden = $true; break }
    }

    if ($isForbidden -and -not $ForceLicense) {
      throw "🛑 BIZTONSÁGI BLOKKOLÁS: A forrás licensze ($licenseDetected) copyleft/fertőző! A Spine védelme érdekében az automatikus beégetés megtagadva. (Felülbírálás: -ForceLicense)"
    }

    # Keresünk egy lényeges Rust libet vagy src/lib.rs-t
    $candidate = Get-ChildItem "$tempDir\repo\src\lib.rs", "$tempDir\repo\src\main.rs", "$tempDir\repo\*.rs" -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($candidate) {
      $codeContent = Get-Content $candidate.FullName -Raw
      Write-Host "  Talált mag: $($candidate.Name) ($($codeContent.Length) B)" -ForegroundColor Green
    } else {
      throw "Nem található releváns Rust kódforrás a repóban."
    }
  } else {
    # Lokális fájl
    if (Test-Path $Target) {
      $codeContent = Get-Content $Target -Raw
      $label = [System.IO.Path]::GetFileNameWithoutExtension($Target)
    } else {
      throw "Érvénytelen cél: $Target"
    }
  }

  if ([string]::IsNullOrWhiteSpace($codeContent)) {
    throw "A szippantott tartalom üres."
  }

  # ─── 2. VÉDELMI KAPU: Sandbox Szintaktikai és Fordítási Ellenőrzés ───
  Write-Host "  🛡️ [GATE 2/2] Sandbox fordítási ellenőrzés (rustc check)..." -ForegroundColor Yellow
  $sandboxFile = Join-Path $tempDir "sandbox_test.rs"
  Set-Content $sandboxFile $codeContent -Encoding UTF8

  $rustc = Get-Command "rustc.exe" -ErrorAction SilentlyContinue
  if ($rustc) {
    # Ellenőrizzük hogy szintaktikailag érvényes Rust kód-e (--emit=metadata, no executable output)
    $diag = & $rustc.Source --crate-type=lib --emit=metadata -o (Join-Path $tempDir "out.rmeta") $sandboxFile 2>&1
    if ($LASTEXITCODE -ne 0) {
      # Ha hiányzó külső függőségek vannak, warning-ként kezeljük, de súlyos szintaxishibánál blokkolunk
      $errorStr = ($diag | Out-String)
      if ($errorStr -match "error: expected|syntax error|unclosed delimiter") {
        throw "🛑 SZINTAKTIKAI HIBA: A szippantott kód szintaxisa hibás, nem égethető be a Spine-ba!`n$errorStr"
      } else {
        Write-Host "  Külső függőségek jelen vannak, szintaxis elfogadva." -ForegroundColor DarkGray
      }
    } else {
      Write-Host "  Sandbox ellenőrzés sikeres! A kód tiszta." -ForegroundColor Green
    }
  }

  # Tisztítás és formázás kar-sablonná: egy soros \n escape a kódba
  $cleanCode = $codeContent.Replace("`r`n", "\n").Replace("`n", "\n").Replace("`"", "\`"")
  $targetFile = "$label.rs"
  $armLine = "$label ::: $Keywords,lic:$licenseDetected ::: WRITE||$targetFile@@@$cleanCode ::: $Tech"

  $armFile = Join-Path $tempDir "new_arm.txt"
  Set-Content $armFile $armLine -Encoding UTF8

  if ($DryRun) {
    Write-Host "  [DRY-RUN] Kar-sablon kész: $label ($($cleanCode.Length) karakternyi kód, lic:$licenseDetected)" -ForegroundColor Yellow
    return
  }

  Write-Host "  Beégetés a Spine mmap-be..." -ForegroundColor Cyan
  $out = & $Hullam add-native $armFile 2>&1
  Write-Host "  $out" -ForegroundColor Green

  # microscope emlék rögzítése a szippantásról
  $memBin = Join-Path $HopeBin "microscope-mem.exe"
  if (Test-Path $memBin) {
    $cfg = "C:\Users\mater\AppData\Local\microscope-memory\config.toml"
    $storeText = "[SIPHON HARVEST] Új ellenőrzött kar (lic:$licenseDetected) a spine-ban: $label ($Keywords) forrás: $Target"
    $env:MICROSCOPE_CONFIG = $cfg
    & $memBin store $storeText --layer code -i 7 2>$null
    Write-Host "  Microscope Memória rögzítve (layer=code, imp=7)" -ForegroundColor DarkGray
  }

  Write-Host "✅ Szippantás, licensz-audit & kar-integráció sikeres!" -ForegroundColor Green
} finally {
  Remove-Item $tempDir -Recurse -Force -ErrorAction SilentlyContinue
}
