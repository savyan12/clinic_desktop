#requires -Version 7.4
param([Parameter(Mandatory = $true)][string]$Destination)

$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"
Set-StrictMode -Version Latest
if (-not $IsWindows -or -not [Environment]::Is64BitProcess) {
    throw "Download and authenticate release runtimes on Windows x64."
}
$Lock = Get-Content -LiteralPath (Join-Path $PSScriptRoot "windows-runtime.lock.json") -Raw | ConvertFrom-Json
New-Item -ItemType Directory -Force -Path $Destination | Out-Null
$Destination = (Resolve-Path -LiteralPath $Destination).Path

function Get-VerifiedArchive($Package) {
    $Path = Join-Path $Destination $Package.filename
    foreach ($Url in $Package.urls) {
        if (([uri]$Url).Scheme -ne "https") { throw "Runtime downloads require HTTPS: $Url" }
        try {
            Invoke-WebRequest -Uri $Url -OutFile $Path -MaximumRetryCount 3 -RetryIntervalSec 5
        }
        catch {
            if ($Url -eq $Package.urls[-1]) { throw }
            Write-Host "Primary vendor URL unavailable; trying the same pinned archive in the vendor archive directory."
            continue
        }
        $Hash = (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
        if ($Hash -ne $Package.sha256) {
            throw "SHA-256 mismatch for $($Package.filename). Expected $($Package.sha256), received $Hash. Do not bypass this check."
        }
        return [ordered]@{ path = $Path; version = $Package.version; url = $Url; sha256 = $Hash }
    }
}

function Assert-MicrosoftSignature([string]$Path) {
    $Signature = Get-AuthenticodeSignature -LiteralPath $Path
    if ($Signature.Status -ne "Valid" -or $Signature.SignerCertificate.Subject -notmatch '(^|,\s*)O=Microsoft Corporation(,|$)') {
        throw "Microsoft Authenticode verification failed for $Path (status: $($Signature.Status))."
    }
}

$Downloads = [ordered]@{}
foreach ($Name in @("mysql", "vcredist")) {
    Write-Host "Downloading and verifying $Name $($Lock.$Name.version)"
    $Downloads[$Name] = Get-VerifiedArchive $Lock.$Name
}
Assert-MicrosoftSignature $Downloads.vcredist.path
$VcVersion = (Get-Item -LiteralPath $Downloads.vcredist.path).VersionInfo.FileVersionRaw.ToString()
if ($VcVersion -ne $Lock.vcredist.version) { throw "VC++ file version differs from the runtime lock: $VcVersion" }

# Tauri's existing offlineInstaller mode resolves this Microsoft URL at build time.
# Populate its actual cache path and authenticate the payload before Tauri embeds it.
$WebViewUrl = "https://go.microsoft.com/fwlink/?linkid=2124701"
$Response = Invoke-WebRequest -Uri $WebViewUrl -Method Head -MaximumRetryCount 3 -RetryIntervalSec 5
$ResolvedUrl = $Response.BaseResponse.RequestMessage.RequestUri.AbsoluteUri
if ($ResolvedUrl -notmatch '^https://msedge\.sf\.dl\.delivery\.mp\.microsoft\.com/filestreamingservice/files/([a-fA-F0-9-]+)/([^/?]+\.exe)$') {
    throw "Unexpected official WebView2 redirect: $ResolvedUrl. Review the vendor URL and the locked Tauri bundler before updating this check."
}
$WebViewDirectory = Join-Path $env:LOCALAPPDATA "tauri\x64\$($Matches[1])"
$WebViewPath = Join-Path $WebViewDirectory $Matches[2]
New-Item -ItemType Directory -Force -Path $WebViewDirectory | Out-Null
Invoke-WebRequest -Uri $ResolvedUrl -OutFile $WebViewPath -MaximumRetryCount 3 -RetryIntervalSec 5
Assert-MicrosoftSignature $WebViewPath
$WebViewVersion = (Get-Item -LiteralPath $WebViewPath).VersionInfo.ProductVersion
if ($WebViewVersion -notmatch '^\d+\.\d+\.\d+\.\d+$') {
    throw "Cannot determine the bundled WebView2 version: $WebViewVersion"
}
$Downloads.webview2 = [ordered]@{
    path = $WebViewPath
    version = $WebViewVersion
    url = $ResolvedUrl
    sha256 = (Get-FileHash -LiteralPath $WebViewPath -Algorithm SHA256).Hash.ToLowerInvariant()
}
$Downloads | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $Destination "downloads.json") -Encoding utf8NoBOM
Write-Host "Verified runtime downloads recorded in $Destination\downloads.json"
