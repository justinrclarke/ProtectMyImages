#!/bin/bash
#
# PMI (Protect My Images) Installation Script
# This script downloads and installs the pmi binary for Mac and Linux.
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/justinrclarke/pmi/master/install.sh | bash
#   wget -qO- https://raw.githubusercontent.com/justinrclarke/pmi/master/install.sh | bash
#

set -e

# Configuration
REPO="justinrclarke/pmi"
BINARY_NAME="pmi"
INSTALL_DIR="${PMI_INSTALL_DIR:-/usr/local/bin}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

error() {
    echo -e "${RED}[ERROR]${NC} $1"
    exit 1
}

# Detect OS
detect_os() {
    local os
    os="$(uname -s)"
    case "$os" in
        Linux*)  echo "linux" ;;
        Darwin*) echo "darwin" ;;
        MINGW*|MSYS*|CYGWIN*) echo "windows" ;;
        *) error "Unsupported operating system: $os" ;;
    esac
}

# Detect architecture
detect_arch() {
    local arch
    arch="$(uname -m)"
    case "$arch" in
        x86_64|amd64) echo "x86_64" ;;
        aarch64|arm64) echo "aarch64" ;;
        armv7l) echo "armv7" ;;
        *) error "Unsupported architecture: $arch" ;;
    esac
}

# Get the latest release version from GitHub
get_latest_version() {
    local version
    version=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" 2>/dev/null | grep '"tag_name"' | sed -E 's/.*"tag_name": *"([^"]+)".*/\1/')

    if [ -z "$version" ]; then
        error "Failed to fetch latest version. Check your internet connection or try again later."
    fi

    echo "$version"
}

# Download and install the binary
install_pmi() {
    local os arch version download_url tmp_dir

    os=$(detect_os)
    arch=$(detect_arch)

    info "Detected OS: $os"
    info "Detected architecture: $arch"

    info "Fetching latest version..."
    version=$(get_latest_version)
    info "Latest version: $version"

    # Construct download URL
    # Expected naming: pmi-{version}-{os}-{arch}.tar.gz
    # For Windows: pmi-{version}-windows-{arch}.zip
    local ext="tar.gz"
    if [ "$os" = "windows" ]; then
        ext="zip"
    fi

    download_url="https://github.com/${REPO}/releases/download/${version}/pmi-${version}-${os}-${arch}.${ext}"

    info "Downloading from: $download_url"

    # Create temporary directory
    tmp_dir=$(mktemp -d)
    trap "rm -rf '$tmp_dir'" EXIT

    # Download the archive
    if command -v curl &> /dev/null; then
        curl -fsSL "$download_url" -o "${tmp_dir}/pmi.${ext}" || error "Download failed. The release may not exist for your platform."
    elif command -v wget &> /dev/null; then
        wget -q "$download_url" -O "${tmp_dir}/pmi.${ext}" || error "Download failed. The release may not exist for your platform."
    else
        error "Neither curl nor wget found. Please install one of them."
    fi

    # Extract the archive
    info "Extracting archive..."
    cd "$tmp_dir"
    if [ "$ext" = "tar.gz" ]; then
        tar -xzf "pmi.${ext}"
    else
        unzip -q "pmi.${ext}"
    fi

    # Find the binary
    local binary_path
    if [ -f "${BINARY_NAME}" ]; then
        binary_path="${BINARY_NAME}"
    elif [ -f "${BINARY_NAME}.exe" ]; then
        binary_path="${BINARY_NAME}.exe"
    else
        error "Binary not found in the archive"
    fi

    # Check if install directory exists
    if [ ! -d "$INSTALL_DIR" ]; then
        warn "Install directory $INSTALL_DIR does not exist."
        info "Attempting to create it..."
        if ! mkdir -p "$INSTALL_DIR" 2>/dev/null; then
            warn "Cannot create $INSTALL_DIR. Trying with sudo..."
            sudo mkdir -p "$INSTALL_DIR" || error "Failed to create install directory"
        fi
    fi

    # Install the binary
    info "Installing to $INSTALL_DIR..."
    if [ -w "$INSTALL_DIR" ]; then
        cp "$binary_path" "$INSTALL_DIR/"
        chmod +x "$INSTALL_DIR/$BINARY_NAME"
    else
        warn "Elevated permissions required for $INSTALL_DIR"
        sudo cp "$binary_path" "$INSTALL_DIR/"
        sudo chmod +x "$INSTALL_DIR/$BINARY_NAME"
    fi

    # Verify installation
    if command -v "$BINARY_NAME" &> /dev/null; then
        success "pmi has been installed successfully!"
        info "Version: $($BINARY_NAME --version 2>/dev/null || echo 'unknown')"
        info "Location: $(command -v $BINARY_NAME)"
    else
        warn "pmi installed to $INSTALL_DIR/$BINARY_NAME"
        warn "Make sure $INSTALL_DIR is in your PATH"
        echo ""
        echo "Add this to your shell configuration file (.bashrc, .zshrc, etc.):"
        echo "  export PATH=\"\$PATH:$INSTALL_DIR\""
    fi
}

# Alternative: build from source if Rust is installed
build_from_source() {
    info "Building from source..."

    if ! command -v cargo &> /dev/null; then
        error "Rust is not installed. Install it from https://rustup.rs/"
    fi

    local tmp_dir
    tmp_dir=$(mktemp -d)
    trap "rm -rf '$tmp_dir'" EXIT

    cd "$tmp_dir"

    info "Cloning repository..."
    git clone --depth 1 "https://github.com/${REPO}.git" pmi || error "Failed to clone repository"

    cd pmi

    info "Building release binary..."
    cargo build --release || error "Build failed"

    # Install the binary
    info "Installing to $INSTALL_DIR..."
    local binary_path="target/release/${BINARY_NAME}"

    if [ -w "$INSTALL_DIR" ]; then
        cp "$binary_path" "$INSTALL_DIR/"
        chmod +x "$INSTALL_DIR/$BINARY_NAME"
    else
        warn "Elevated permissions required for $INSTALL_DIR"
        sudo cp "$binary_path" "$INSTALL_DIR/"
        sudo chmod +x "$INSTALL_DIR/$BINARY_NAME"
    fi

    success "pmi has been built and installed successfully!"
}

# Main
main() {
    echo ""
    echo "  ____  __  __ ___ "
    echo " |  _ \\|  \\/  |_ _|"
    echo " | |_) | |\\/| || | "
    echo " |  __/| |  | || | "
    echo " |_|   |_|  |_|___|"
    echo ""
    echo " Protect My Images - Installation Script"
    echo ""

    # Check for --source flag to build from source
    if [ "${1:-}" = "--source" ]; then
        build_from_source
    else
        install_pmi
    fi

    echo ""
    echo "Usage:"
    echo "  pmi <image.jpg>           Strip metadata from a single image"
    echo "  pmi <directory>           Process all images in a directory"
    echo "  pmi --help                Show all options"
    echo ""
}

main "$@"
