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
if (-not $Dest) { $Dest = Join-Path (Split-Path $src -Parent) "ccc-$Name" }
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
    -replace '^game_slug = .*', "game_slug = `"$Game`"" | Set-Content $toml
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
    cargo build --release -q
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }
    Write-Host "Created contest repo: $Dest`n`nChecking the codingcontest.org login and contest:"
    cargo ccc login
    $ready = if ($LASTEXITCODE -eq 0) { "Ready." } else {
        "Not ready yet: fix the login/contest issue above (log in to codingcontest.org in Firefox, start the contest or training there), then run ``cargo ccc login`` again."
    }
} finally { Pop-Location }

Write-Host @"

$ready Next:
  cd "$Dest"; claude            # start Claude Code in the contest repo
  /level                        # plays from the current level: fetch, solve, all, submit (you approve), next
"@
