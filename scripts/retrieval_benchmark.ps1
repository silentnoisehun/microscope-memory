<#
.SYNOPSIS
  Benchmark the retrieval path of a live Microscope Memory instance from the
  command line: lexical baseline versus hybrid retrieval, at several
  semantic_rank_gain values.

.DESCRIPTION
  The engine ships a built-in evaluation diagnostic: with
  MICROSCOPE_EVAL_DIAG=1 and MICROSCOPE_EVAL_MATCH=<needle> the CLI reports
  where the expected memory ended up in the final ranking and whether it came
  from the lexical side, the vector side or both:

      EVALDIAG final=pos=3 lexical=false vector=true source=vector

  This script drives that diagnostic over a fixture of statements, once per
  variant, and writes a TSV that scripts/aggregate_retrieval_benchmark.py
  turns into R@1 / R@5 / R@10 / MRR per language and per variant.

  The lexical baseline is measured with provider = "none": the index
  fingerprint then does not match, the semantic candidates are empty and the
  engine runs exactly the same rank_candidates code path without them.

  The needle must be a real substring of the stored block text and must only
  occur in blocks that carry the same memory -- the hierarchy keeps one
  statement inside a larger session text at one depth and on its own at
  another.  scripts/make_benchmark_needles.py derives verified needles from
  the instance itself.

.PARAMETER Config
  Path to the instance config.toml.

.PARAMETER Fixture
  JSON file with {"cases": [{"id", "expected", "queries": [{"lang", "q"}]}]}.

.PARAMETER OutFile
  TSV to write. Default: bench_results.tsv.

.EXAMPLE
  pwsh -File scripts/retrieval_benchmark.ps1 `
      -Config "$env:USERPROFILE\.codex\microscope-memory\config.toml" `
      -Fixture tests/fixtures/relevance/real_corpus_cases.json
#>
param(
    [Parameter(Mandatory = $true)][string]$Config,
    [Parameter(Mandatory = $true)][string]$Fixture,
    [string]$OutFile = 'bench_results.tsv',
    [string]$Binary = ''
)

$ErrorActionPreference = 'Continue'

if (-not $Binary) {
    $Binary = Join-Path $env:USERPROFILE '.hope\bin\microscope-mem.exe'
    if (-not (Test-Path $Binary)) {
        $Binary = 'microscope-mem.exe'
    }
}

$variants = @(
    @{ name = 'lexical'; provider = 'none';    gain = '1.0' },
    @{ name = 'gain0.5'; provider = 'candle'; gain = '0.5' },
    @{ name = 'gain1.0'; provider = 'candle'; gain = '1.0' },
    @{ name = 'gain1.5'; provider = 'candle'; gain = '1.5' },
    @{ name = 'gain2.0'; provider = 'candle'; gain = '2.0' }
)

$cases = Get-Content $Fixture -Raw -Encoding UTF8 | ConvertFrom-Json
$utf8NoBom = New-Object System.Text.UTF8Encoding($false)

Write-Output "### variant configs (derived from $Config) ###"
$variantConfigs = @{}
foreach ($v in $variants) {
    $path = [System.IO.Path]::GetTempFileName() + '.toml'
    $text = [System.IO.File]::ReadAllText($Config, [System.Text.Encoding]::UTF8)
    $text = [System.Text.RegularExpressions.Regex]::Replace(
        $text, '(?m)^provider\s*=\s*"[^"]*"', ('provider = "' + $v.provider + '"'))
    $text = [System.Text.RegularExpressions.Regex]::Replace(
        $text, '(?m)^semantic_rank_gain\s*=\s*[0-9.]+', ('semantic_rank_gain = ' + $v.gain))
    [System.IO.File]::WriteAllText($path, $text, $utf8NoBom)
    $variantConfigs[$v.name] = $path
    Write-Output ("   " + $v.name + "  provider=" + $v.provider + " gain=" + $v.gain)
}

$rows = New-Object System.Collections.Generic.List[string]
$rows.Add("case_id`tlang`tvariant`tquery`tneedle`tevaldiag")

$env:MICROSCOPE_NO_LEARN = '1'
$i = 0
foreach ($c in $cases) {
    foreach ($q in $c.queries) {
        $needle = if ($c.PSObject.Properties.Name -contains 'needle') { $c.needle } else { $c.expected }
        foreach ($v in $variants) {
            $env:MICROSCOPE_CONFIG = $variantConfigs[$v.name]
            $env:MICROSCOPE_EVAL_DIAG = '1'
            $env:MICROSCOPE_EVAL_MATCH = $needle
            $out = & $Binary recall $q.q 10 2>&1 | Out-String
            $diag = ($out -split "`n" | Where-Object { $_ -match 'EVALDIAG' } | Select-Object -First 1)
            if ($null -eq $diag) { $diag = 'EVALDIAG final=NO_DIAG_LINE' }
            $diag = ($diag -replace '.*EVALDIAG\s+', '').Trim()
            $rows.Add("$($c.id)`t$($q.lang)`t$($v.name)`t$($q.q)`t$needle`t$diag")
            $i++
        }
    }
    if (($i % 30) -eq 0) { Write-Output ("   ... $i runs done") }
}

[System.IO.File]::WriteAllLines($OutFile, $rows, $utf8NoBom)
Write-Output ("done: $i runs -> " + $OutFile)
