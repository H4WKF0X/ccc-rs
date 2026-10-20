#!/usr/bin/env pwsh
# Create a new contest repo from this template (Windows mirror of new-contest.sh).
#   ./new-contest.ps1 <name> [dest] [-Game <slug>]
param(
    [Parameter(Mandatory = $true, Position = 0)] [string]$Name,
    [Parameter(Position = 1)] [string]$Dest = '',
    [string]$Game = ''
)
$ErrorActionPreference = 'Stop'
if ($Name -notmatch '^[A-Za-z0-9._-]+$') { throw "name may only contain letters, digits, '.', '_' and '-'" }

$src = $PSScriptRoot
$folder = if ($Name.StartsWith('ccc-')) { $Name } else { "ccc-$Name" }
if (-not $Dest) { $Dest = Join-Path (Split-Path $src -Parent) $folder }
if ((Test-Path $Dest) -and (Get-ChildItem -Force $Dest | Select-Object -First 1)) { throw "$Dest already exists and is not empty" }
New-Item -ItemType Directory -Force -Path $Dest | Out-Null
$Dest = (Resolve-Path $Dest).Path

# Copy what git would commit (respects .gitignore, so no target/, work/, ccc.local.toml, *.har).
$files = git -C $src ls-files --cached --others --exclude-standard
if ($LASTEXITCODE -ne 0) { throw "git ls-files failed; is the template a git repo?" }
foreach ($rel in $files) {
    if ($rel -in @('new-contest.sh', 'new-contest.ps1')) { continue }
    $target = Join-Path $Dest $rel
    New-Item -ItemType Directory -Force -Path (Split-Path $target -Parent) | Out-Null
    Copy-Item -LiteralPath (Join-Path $src $rel) -Destination $target
}

$toml = Join-Path $Dest 'ccc.toml'
(Get-Content $toml) `
    -replace '^name = .*', "name = `"$Name`"" `
    -replace '^game_slug = .*', "game_slug = `"$Game`"" `
    -replace '^created = .*', "created = `"$([DateTimeOffset]::UtcNow.ToUnixTimeSeconds())`"" | Set-Content $toml
# Unique package and binary names: contest repos share one build directory (see .cargo/config.toml).
$id = $Name -replace '\.', '-'
foreach ($f in @('harness/Cargo.toml', 'solution/Cargo.toml')) {
    $path = Join-Path $Dest $f
    (Get-Content $path) -replace 'harness-template', "harness-$id" -replace 'solution-template', "solution-$id" `
        -replace '"ccc-template"', "`"ccc-$id`"" | Set-Content $path
}

Push-Location $Dest
try {
    git init -q -b main
    git add -A
    git commit -q -m "contest $Name (from ccc-rs template)"
    # Build everything now (release + tests), so nothing compiles at contest time.
    cargo build --release -q
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }
    cargo test --no-run -q 2>$null
    Write-Host "Created contest repo: $Dest (built, nothing left to compile)`n"
    cargo ccc login
} finally { Pop-Location }

Write-Host @"

When the contest starts (runbook: docs/CONTEST-DAY.md):
  1. Open the game in the browser (Firefox, logged in to codingcontest.org).
  2. cd "$Dest"; cargo ccc login       -> must say LOGIN OK
  3. claude, then /level                -> fetch, check, solve, submit (you approve), next level ...
  LOGIN FAILED? cargo ccc mode manual   -> zips and uploads go through Downloads; then claude + /level
"@
