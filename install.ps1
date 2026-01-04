#Requires -Version 5.1
<#
.SYNOPSIS
    PMI (Protect My Images) Installation Script for Windows

.DESCRIPTION
    This script downloads and installs the pmi binary for Windows.

.PARAMETER InstallDir
    The directory where pmi will be installed. Defaults to $env:LOCALAPPDATA\pmi

.PARAMETER AddToPath
    Whether to add the install directory to the user's PATH. Defaults to $true

.PARAMETER Version
    Specific version to install. Defaults to 'latest'

.EXAMPLE
    # Install from PowerShell:
    irm https://raw.githubusercontent.com/justinrclarke/pmi/master/install.ps1 | iex

    # Or download and run:
    .\install.ps1

    # Install to a custom directory:
    .\install.ps1 -InstallDir "C:\Tools\pmi"

.NOTES
    Author: PMI Team
    License: Apache-2.0
#>

param(
    [string]$InstallDir = "$env:LOCALAPPDATA\pmi",
    [bool]$AddToPath = $true,
    [string]$Version = "latest"
)

$ErrorActionPreference = "Stop"

# Configuration
$Repo = "justinrclarke/pmi"
$BinaryName = "pmi.exe"

function Write-Info {
    param([string]$Message)
    Write-Host "[INFO] " -ForegroundColor Blue -NoNewline
    Write-Host $Message
}

function Write-Success {
    param([string]$Message)
    Write-Host "[SUCCESS] " -ForegroundColor Green -NoNewline
    Write-Host $Message
}

function Write-Warn {
    param([string]$Message)
    Write-Host "[WARN] " -ForegroundColor Yellow -NoNewline
    Write-Host $Message
}

function Write-Error {
    param([string]$Message)
    Write-Host "[ERROR] " -ForegroundColor Red -NoNewline
    Write-Host $Message
    exit 1
}

function Get-Architecture {
    $arch = $env:PROCESSOR_ARCHITECTURE
    switch ($arch) {
        "AMD64" { return "x86_64" }
        "ARM64" { return "aarch64" }
        "x86"   { return "i686" }
        default { Write-Error "Unsupported architecture: $arch" }
    }
}

function Get-LatestVersion {
    try {
        $response = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest" -UseBasicParsing
        return $response.tag_name
    }
    catch {
        Write-Error "Failed to fetch latest version. Check your internet connection."
    }
}

function Install-Pmi {
    Write-Host ""
    Write-Host "  ____  __  __ ___ " -ForegroundColor Cyan
    Write-Host " |  _ \|  \/  |_ _|" -ForegroundColor Cyan
    Write-Host " | |_) | |\/| || | " -ForegroundColor Cyan
    Write-Host " |  __/| |  | || | " -ForegroundColor Cyan
    Write-Host " |_|   |_|  |_|___|" -ForegroundColor Cyan
    Write-Host ""
    Write-Host " Protect My Images - Installation Script" -ForegroundColor Cyan
    Write-Host ""

    $arch = Get-Architecture
    Write-Info "Detected architecture: $arch"

    # Get version
    if ($Version -eq "latest") {
        Write-Info "Fetching latest version..."
        $Version = Get-LatestVersion
    }
    Write-Info "Version: $Version"

    # Construct download URL
    $downloadUrl = "https://github.com/$Repo/releases/download/$Version/pmi-$Version-windows-$arch.zip"
    Write-Info "Downloading from: $downloadUrl"

    # Create temp directory
    $tempDir = Join-Path $env:TEMP "pmi-install-$(Get-Random)"
    New-Item -ItemType Directory -Path $tempDir -Force | Out-Null

    try {
        # Download the archive
        $zipPath = Join-Path $tempDir "pmi.zip"
        Write-Info "Downloading..."

        try {
            Invoke-WebRequest -Uri $downloadUrl -OutFile $zipPath -UseBasicParsing
        }
        catch {
            Write-Error "Download failed. The release may not exist for your platform.`nURL: $downloadUrl"
        }

        # Extract the archive
        Write-Info "Extracting..."
        Expand-Archive -Path $zipPath -DestinationPath $tempDir -Force

        # Find the binary
        $binaryPath = Get-ChildItem -Path $tempDir -Filter "pmi.exe" -Recurse | Select-Object -First 1 -ExpandProperty FullName
        if (-not $binaryPath) {
            Write-Error "Binary not found in the archive"
        }

        # Create install directory
        if (-not (Test-Path $InstallDir)) {
            Write-Info "Creating install directory: $InstallDir"
            New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
        }

        # Copy binary
        $destPath = Join-Path $InstallDir $BinaryName
        Write-Info "Installing to: $destPath"
        Copy-Item -Path $binaryPath -Destination $destPath -Force

        # Add to PATH
        if ($AddToPath) {
            $userPath = [Environment]::GetEnvironmentVariable("Path", [EnvironmentVariableTarget]::User)
            if ($userPath -notlike "*$InstallDir*") {
                Write-Info "Adding $InstallDir to PATH..."
                $newPath = "$userPath;$InstallDir"
                [Environment]::SetEnvironmentVariable("Path", $newPath, [EnvironmentVariableTarget]::User)
                $env:Path = "$env:Path;$InstallDir"
                Write-Success "Added to PATH"
            }
            else {
                Write-Info "$InstallDir is already in PATH"
            }
        }

        # Verify installation
        Write-Success "pmi has been installed successfully!"
        Write-Info "Location: $destPath"

        # Try to show version
        try {
            $versionOutput = & $destPath --version 2>&1
            Write-Info "Installed version: $versionOutput"
        }
        catch {
            # Ignore errors from version check
        }

    }
    finally {
        # Cleanup
        if (Test-Path $tempDir) {
            Remove-Item -Path $tempDir -Recurse -Force -ErrorAction SilentlyContinue
        }
    }

    Write-Host ""
    Write-Host "Usage:" -ForegroundColor Cyan
    Write-Host "  pmi <image.jpg>           Strip metadata from a single image"
    Write-Host "  pmi <directory>           Process all images in a directory"
    Write-Host "  pmi --help                Show all options"
    Write-Host ""

    if ($AddToPath) {
        Write-Warn "You may need to restart your terminal for PATH changes to take effect."
    }
}

# Alternative: build from source using cargo
function Build-FromSource {
    Write-Info "Building from source..."

    # Check for cargo
    try {
        $null = Get-Command cargo -ErrorAction Stop
    }
    catch {
        Write-Error "Rust is not installed. Install it from https://rustup.rs/"
    }

    $tempDir = Join-Path $env:TEMP "pmi-build-$(Get-Random)"
    New-Item -ItemType Directory -Path $tempDir -Force | Out-Null

    try {
        Set-Location $tempDir

        Write-Info "Cloning repository..."
        git clone --depth 1 "https://github.com/$Repo.git" pmi
        if ($LASTEXITCODE -ne 0) {
            Write-Error "Failed to clone repository"
        }

        Set-Location pmi

        Write-Info "Building release binary..."
        cargo build --release
        if ($LASTEXITCODE -ne 0) {
            Write-Error "Build failed"
        }

        # Create install directory
        if (-not (Test-Path $InstallDir)) {
            New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
        }

        # Copy binary
        $binaryPath = "target\release\pmi.exe"
        $destPath = Join-Path $InstallDir $BinaryName
        Copy-Item -Path $binaryPath -Destination $destPath -Force

        Write-Success "pmi has been built and installed successfully!"
    }
    finally {
        Set-Location $env:USERPROFILE
        if (Test-Path $tempDir) {
            Remove-Item -Path $tempDir -Recurse -Force -ErrorAction SilentlyContinue
        }
    }
}

# Run installation
Install-Pmi
