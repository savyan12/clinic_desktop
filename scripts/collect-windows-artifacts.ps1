#requires -Version 7.4
param([Parameter(Mandatory = $true)][string]$DownloadsFile)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
$ProjectRoot = Split-Path -Parent $PSScriptRoot
$Config = Get-Content -LiteralPath (Join-Path $ProjectRoot "src-tauri\tauri.conf.json") -Raw | ConvertFrom-Json
$Downloads = Get-Content -LiteralPath $DownloadsFile -Raw | ConvertFrom-Json
$TargetRoot = Join-Path $ProjectRoot "src-tauri\target\release"
$BundleRoot = Join-Path $TargetRoot "bundle\nsis"
$Installers = @(Get-ChildItem -LiteralPath $BundleRoot -Filter "*.exe" -File)
if ($Installers.Count -ne 1) {
    throw "Expected exactly one NSIS .exe in $BundleRoot; found $($Installers.Count). MSI is not a release substitute."
}
if ($Installers[0].Length -eq 0) { throw "The NSIS installer is empty." }

# Check the rendered NSIS script, not merely tauri.conf.json or a cached download.
$Scripts = @(Get-ChildItem -LiteralPath (Join-Path $TargetRoot "nsis") -Filter "installer.nsi" -File -Recurse)
if ($Scripts.Count -ne 1) { throw "Expected one rendered NSIS installer script for the x64 build." }
$InstallerScript = Get-Content -LiteralPath $Scripts[0].FullName -Raw
if ($InstallerScript -notmatch '(?m)^!define ARCH "x64"' -or
    $InstallerScript -notmatch '(?m)^!define INSTALLWEBVIEW2MODE "offlineInstaller"') {
    throw "The generated installer is not an x64 NSIS bundle with offline WebView2."
}
if ($InstallerScript -notmatch '(?m)^!define WEBVIEW2INSTALLERPATH "([^"\r\n]+)"') {
    throw "The generated NSIS script has no offline WebView2 payload."
}
$EmbeddedWebViewPath = $Matches[1]
if (-not [string]::Equals($EmbeddedWebViewPath, $Downloads.webview2.path, [StringComparison]::OrdinalIgnoreCase)) {
    throw "Tauri selected a different WebView2 payload after validation. Re-run the build to validate the new Microsoft release."
}
$EmbeddedHash = (Get-FileHash -LiteralPath $EmbeddedWebViewPath -Algorithm SHA256).Hash.ToLowerInvariant()
if ($EmbeddedHash -ne $Downloads.webview2.sha256) { throw "The WebView2 payload changed after validation." }
foreach ($File in @("mysqld.exe", "mysql.exe", "mysqldump.exe", "mysqladmin.exe", "VC_redist.x64.exe")) {
    if (-not $InstallerScript.Contains($File)) { throw "The NSIS resource list is missing $File." }
}
foreach ($Forbidden in @("runtime\php", "runtime\backend", "php.exe", "artisan", "composer.phar")) {
    if ($InstallerScript.Contains($Forbidden)) { throw "The NSIS resource list contains a retired runtime: $Forbidden." }
}

$ArtifactRoot = Join-Path $ProjectRoot "release-artifacts"
if (Test-Path -LiteralPath $ArtifactRoot) { Remove-Item -LiteralPath $ArtifactRoot -Recurse -Force }
New-Item -ItemType Directory -Path $ArtifactRoot | Out-Null
$InstallerName = "Clinic-System_$($Config.version)_x64-setup.exe"
$InstallerPath = Join-Path $ArtifactRoot $InstallerName
Copy-Item -LiteralPath $Installers[0].FullName -Destination $InstallerPath
$Hash = (Get-FileHash -LiteralPath $InstallerPath -Algorithm SHA256).Hash.ToLowerInvariant()
"$Hash  $InstallerName" | Set-Content -LiteralPath "$InstallerPath.sha256" -Encoding utf8NoBOM

$Runtimes = [ordered]@{}
foreach ($Name in @("mysql", "vcredist", "webview2")) {
    $Runtime = $Downloads.$Name
    $Runtimes[$Name] = [ordered]@{ version = $Runtime.version; url = $Runtime.url; sha256 = $Runtime.sha256 }
}
$Manifest = [ordered]@{
    applicationVersion = $Config.version
    architecture = "x64"
    bundle = "nsis"
    commit = $env:GITHUB_SHA
    installer = $InstallerName
    installerSha256 = $Hash
    runtimes = $Runtimes
    nodeVersion = $env:NODE_VERSION
    rustVersion = $env:RUST_VERSION
    windowsCompatibility = "Clean Windows 10 and Windows 11 acceptance testing is required separately. A CI build is not certification."
}
$Manifest | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $ArtifactRoot "runtime-versions.json") -Encoding utf8NoBOM
Write-Host "NSIS release files collected in $ArtifactRoot"
if ($env:GITHUB_STEP_SUMMARY) {
    @"
### Windows x64 NSIS build

- Application: $($Config.version)
- Backend: embedded Rust; MySQL: $($Runtimes.mysql.version)
- VC++ x64: $($Runtimes.vcredist.version); offline WebView2: $($Runtimes.webview2.version)
- Installer: $InstallerName
- SHA-256: $Hash
- Clean Windows 10/11 installation, activation, offline operation, and USB restore still require manual acceptance.
"@ | Add-Content -LiteralPath $env:GITHUB_STEP_SUMMARY
}
