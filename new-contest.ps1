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

Push-Location $Dest
try {
    git init -q -b main
    git add -A
    git commit -q -m "contest $Name (from ccc-rs template)"
    cargo build --release -q
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }
} finally { Pop-Location }

Write-Host @"
Created contest repo: $Dest

Next:
  cd "$Dest"
  claude                       # start Claude Code here
  cargo ccc ingest <zip>       # after downloading a level (or no args: newest zip in Downloads)
  /solve 1                     # in Claude Code
  cargo ccc all 1              # validate on the example, then write the real outputs
  cargo ccc submit 1           # manual mode: lists the files to upload in the browser
"@
