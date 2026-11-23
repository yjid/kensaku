# Kensaku installer for Windows.
#
#   powershell -ExecutionPolicy ByPass -c "irm https://github.com/yjid/kensaku/releases/latest/download/install.ps1 | iex"
#
# It reads the release manifest, downloads the signed NSIS installer, verifies
# its SHA-256 against the release's SHA256SUMS and its Authenticode signature,
# then runs it in the current user's account. No elevation is required.
#
# Prefer to inspect it first:
#   irm https://github.com/yjid/kensaku/releases/latest/download/install.ps1 -OutFile install.ps1
#   notepad install.ps1
#   powershell -ExecutionPolicy ByPass -File .\install.ps1
#
# Parameters:
#   -Version 1.0.19     Install a specific release (default: latest).
#   -Silent             Run NSIS with /S instead of the passive /P progress.
#   -DryRun             Resolve and print the URL/hash/signature; install nothing.
#   -VerifyOnly         Download and verify; install nothing.
#   -SkipSignatureCheck Only for local/test builds; release installers are signed.

#Requires -Version 5.1
[CmdletBinding()]
param(
    [string]$Version = "latest",
    [switch]$Silent,
    [switch]$DryRun,
    [switch]$VerifyOnly,
    [switch]$SkipSignatureCheck,
    [string]$ManifestUrl = "",
    [string]$SumsUrl = ""
)

$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"
$Repo = "yjid/kensaku"

function Fail([string]$Message) {
    Write-Error "kensaku: $Message"
    exit 1
}

$architecture = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture
switch ($architecture) {
    ([System.Runtime.InteropServices.Architecture]::X64) {
        $platformKey = "windows-x86_64"
    }
    ([System.Runtime.InteropServices.Architecture]::Arm64) {
        $platformKey = "windows-aarch64"
    }
    default {
        Fail "Unsupported Windows architecture: $architecture"
    }
}

if ([string]::IsNullOrWhiteSpace($ManifestUrl)) {
    if ($Version -eq "latest") {
        $ManifestUrl = "https://github.com/$Repo/releases/latest/download/latest.json"
    }
    else {
        $tag = $Version.TrimStart("v")
        $ManifestUrl = "https://github.com/$Repo/releases/download/v$tag/latest.json"
    }
}

Write-Host "Fetching $ManifestUrl"
try {
    $manifest = Invoke-RestMethod -Uri $ManifestUrl -UseBasicParsing -Headers @{
        "User-Agent" = "kensaku-installer"
    }
}
catch {
    Fail "Could not download the release manifest. $($_.Exception.Message)"
}

$releaseVersion = [string]$manifest.version
if ([string]::IsNullOrWhiteSpace($releaseVersion)) {
    Fail "The release manifest has no version."
}
if ($Version -ne "latest" -and $Version.TrimStart("v") -ne $releaseVersion) {
    Fail "The manifest is for $releaseVersion, not $Version."
}

$entry = $manifest.platforms.$platformKey
if ($null -eq $entry -or [string]::IsNullOrWhiteSpace([string]$entry.url)) {
    Fail "No Windows installer for $platformKey in the release manifest."
}

$installerUrl = [string]$entry.url
$installerName = [System.IO.Path]::GetFileName(([Uri]$installerUrl).AbsolutePath)
if ([string]::IsNullOrWhiteSpace($SumsUrl)) {
    $SumsUrl = "https://github.com/$Repo/releases/download/v$releaseVersion/SHA256SUMS"
}

if ($DryRun) {
    Write-Host "Version:    $releaseVersion"
    Write-Host "Installer:  $installerUrl"
    Write-Host "SHA256SUMS: $SumsUrl"
    Write-Host "Mode:       $(if ($Silent) { 'silent (/S /UPDATE)' } else { 'passive (/P /UPDATE)' })"
    exit 0
}

$temp = Join-Path $env:TEMP ("kensaku-install-" + [Guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Path $temp -Force | Out-Null

try {
    $installerPath = Join-Path $temp $installerName
    Write-Host "Downloading Kensaku $releaseVersion ($installerName)"
    Invoke-WebRequest -Uri $installerUrl -OutFile $installerPath -UseBasicParsing

    Write-Host "Verifying SHA-256"
    $sumsPath = Join-Path $temp "SHA256SUMS"
    Invoke-WebRequest -Uri $SumsUrl -OutFile $sumsPath -UseBasicParsing
    $expected = $null
    foreach ($line in Get-Content -LiteralPath $sumsPath) {
        $parts = $line.Trim() -split "\s+", 2
        if ($parts.Count -eq 2) {
            $name = $parts[1].TrimStart("*")
            if ($name -eq $installerName) {
                $expected = $parts[0].ToLowerInvariant()
                break
            }
        }
    }
    if ([string]::IsNullOrWhiteSpace($expected)) {
        Fail "$installerName is not listed in SHA256SUMS."
    }
    $actual = (Get-FileHash -LiteralPath $installerPath -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($actual -ne $expected) {
        Fail "SHA-256 mismatch for $installerName (expected $expected, got $actual)."
    }
    Write-Host "SHA-256 OK"

    Write-Host "Verifying Authenticode signature"
    $signature = Get-AuthenticodeSignature -LiteralPath $installerPath
    if ($signature.Status -ne "Valid") {
        if ($SkipSignatureCheck) {
            Write-Warning "Signature status is $($signature.Status); continuing because -SkipSignatureCheck was passed."
        }
        else {
            Fail "The installer signature is $($signature.Status), not Valid. Re-download from GitHub Releases."
        }
    }
    else {
        Write-Host "Signed by $($signature.SignerCertificate.Subject)"
    }

    if ($VerifyOnly) {
        Write-Host "Verified $installerName. Nothing installed (-VerifyOnly)."
        exit 0
    }

    $nsisArgs = if ($Silent) { @("/S", "/UPDATE") } else { @("/P", "/UPDATE") }
    Write-Host "Installing Kensaku $releaseVersion (current user, no admin prompt)"
    $process = Start-Process -FilePath $installerPath -ArgumentList $nsisArgs -Wait -PassThru
    if ($process.ExitCode -ne 0) {
        Fail "The installer exited with code $($process.ExitCode)."
    }

    Write-Host ""
    Write-Host "Kensaku $releaseVersion is installed. Open it from the Start menu."
    Write-Host "Later releases update in-app; no installer is needed."
}
finally {
    Remove-Item -LiteralPath $temp -Recurse -Force -ErrorAction SilentlyContinue
}
