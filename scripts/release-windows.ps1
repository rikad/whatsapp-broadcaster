# Builds the Windows x64 .exe for a release tag and attaches it to the GitHub
# release that scripts/release.sh (run on a Mac) created for that tag.
#
# Usage: .\scripts\release-windows.ps1 [-Tag v2.0.0] [-NoPublish]
#   -Tag        tag to build; defaults to the newest v* tag on origin
#   -NoPublish  build into dist\ only; no upload
#
# Needs: Visual Studio Build Tools ("Desktop development with C++", which brings
# the Windows SDK and fxc.exe), rustup, git, and gh logged in (for publishing).
# If PowerShell refuses to run scripts:
#   powershell -ExecutionPolicy Bypass -File scripts\release-windows.ps1
param(
    [string]$Tag,
    [switch]$NoPublish
)

$ErrorActionPreference = 'Stop'

$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

$bin = 'whatsapp-broadcaster'
$asset = "$bin-windows-x64.exe"

function Step($msg) { Write-Host "`n==> $msg" -ForegroundColor Blue }

# Runs a native command and stops on a non-zero exit code. Uses $args rather
# than param() so flags like -q are passed through instead of bound here.
function Run {
    $rest = @($args | Select-Object -Skip 1)
    & $args[0] @rest
    if ($LASTEXITCODE -ne 0) { throw "$($args -join ' ') failed (exit $LASTEXITCODE)" }
}

# Runs a native command quietly and returns whether it succeeded. Windows
# PowerShell 5.1 turns redirected stderr into errors, which 'Stop' would throw.
function Succeeds {
    $ErrorActionPreference = 'Continue'
    $rest = @($args | Select-Object -Skip 1)
    & $args[0] @rest *> $null
    $LASTEXITCODE -eq 0
}

if (git status --porcelain) { throw 'Working tree is not clean; commit or stash first.' }

Step 'Fetching tags'
Run git fetch --tags --force origin
if (-not $Tag) {
    $Tag = git tag --list 'v*' --sort=-v:refname | Select-Object -First 1
    if (-not $Tag) { throw 'No v* tags found. Run scripts/release.sh on the Mac first.' }
}
if (-not (Succeeds git rev-parse -q --verify "refs/tags/$Tag")) { throw "Tag $Tag does not exist on origin." }

if (-not $NoPublish) {
    if (-not (Succeeds gh auth status)) { throw 'gh is not logged in (run: gh auth login).' }
    if (-not (Succeeds gh release view $Tag)) { throw "No GitHub release for $Tag yet. Run scripts/release.sh on the Mac first." }
}

# Build the tagged commit, then go back to where we were.
$previous = git symbolic-ref -q --short HEAD
if (-not $previous) { $previous = git rev-parse HEAD }
Step "Building $Tag"
Run git checkout -q --detach $Tag
try {
    Run cargo build --release --locked -p $bin
    New-Item -ItemType Directory -Force dist | Out-Null
    Copy-Item "target\release\$bin.exe" "dist\$asset" -Force
} finally {
    git checkout -q $previous
}
Get-Item "dist\$asset" | Format-Table Name, @{ n = 'MB'; e = { [math]::Round($_.Length / 1MB, 1) } } -AutoSize

if ($NoPublish) { return }

Step "Uploading to $Tag"
Run gh release upload $Tag "dist\$asset" --clobber
Write-Host "`nPublished $(gh release view $Tag --json url -q .url)"
