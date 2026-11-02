#requires -Version 7.4
param(
    [Parameter(Mandatory = $true)]
    [string]$MySqlArchive,

    [Parameter(Mandatory = $true)]
    [string]$VcRedistInstaller
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
$ProjectRoot = Split-Path -Parent $PSScriptRoot
$RuntimeRoot = Join-Path $ProjectRoot "src-tauri\resources\runtime"
$TemporaryRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("clinic-runtime-" + [guid]::NewGuid().ToString("N"))
$MySqlTemporary = Join-Path $TemporaryRoot "mysql"
$MySqlTarget = Join-Path $RuntimeRoot "mysql"
$VcRedistTarget = Join-Path $RuntimeRoot "vcredist"

function Assert-File([string]$Path, [string]$Label) {
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        throw "$Label was not found: $Path"
    }
}

Assert-File $MySqlArchive "Official MySQL ZIP archive"
Assert-File $VcRedistInstaller "Microsoft Visual C++ Redistributable installer"
if (-not [Environment]::Is64BitOperatingSystem -or $env:OS -ne "Windows_NT") {
    throw "Windows runtime preparation requires a Windows x64 build host."
}

try {
    New-Item -ItemType Directory -Force -Path $MySqlTemporary, $RuntimeRoot | Out-Null
    Expand-Archive -LiteralPath $MySqlArchive -DestinationPath $MySqlTemporary -Force

    if (Test-Path -LiteralPath $MySqlTarget) {
        Remove-Item -LiteralPath $MySqlTarget -Recurse -Force
    }
    if (Test-Path -LiteralPath $VcRedistTarget) {
        Remove-Item -LiteralPath $VcRedistTarget -Recurse -Force
    }

    $MySqlExe = Get-ChildItem -LiteralPath $MySqlTemporary -Filter "mysqld.exe" -File -Recurse | Select-Object -First 1
    if (-not $MySqlExe) {
        throw "mysqld.exe was not found in the supplied MySQL archive."
    }
    $MySqlRoot = Split-Path -Parent $MySqlExe.Directory.FullName
    Copy-Item -LiteralPath $MySqlRoot -Destination $MySqlTarget -Recurse -Force

    New-Item -ItemType Directory -Force -Path $VcRedistTarget | Out-Null
    Copy-Item -LiteralPath $VcRedistInstaller -Destination (Join-Path $VcRedistTarget "VC_redist.x64.exe") -Force

    & node (Join-Path $PSScriptRoot "validate-windows-runtime.mjs")
    if ($LASTEXITCODE -ne 0) {
        throw "Windows runtime validation failed. MySQL must be a valid x64 MySQL 8 runtime."
    }

    Write-Host "Windows MySQL runtime was prepared and validated in $RuntimeRoot"
}
finally {
    if (Test-Path -LiteralPath $TemporaryRoot) {
        Remove-Item -LiteralPath $TemporaryRoot -Recurse -Force
    }
}
