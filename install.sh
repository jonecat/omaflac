#!/usr/bin/env bash
# ==============================================================================
# OmaFLAC — Cross-Platform Installer (Linux & macOS)
# Repository: https://github.com/jonecat/omaflac
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/jonecat/omaflac/main/install.sh | bash
# ==============================================================================

set -euo pipefail

REPO_URL="https://github.com/jonecat/omaflac.git"
INSTALL_DIR="${OMAFLAC_INSTALL_DIR:-$HOME/.local/bin}"
BIN_NAME="omaflac"

# Colors & formatting
if [ -t 1 ]; then
    BOLD="\033[1m"
    DIM="\033[2m"
    GREEN="\033[38;2;158;198;106m"
    CYAN="\033[38;2;125;207;255m"
    YELLOW="\033[38;2;224;175;104m"
    RED="\033[38;2;247;118;142m"
    RESET="\033[0m"
else
    BOLD=""
    DIM=""
    GREEN=""
    CYAN=""
    YELLOW=""
    RED=""
    RESET=""
fi

print_banner() {
    printf "\n"
    printf "  ${GREEN}${BOLD} ██████╗ ███╗   ███╗ █████╗ ${CYAN}███████╗██╗      █████╗  ██████╗${RESET}\n"
    printf "  ${GREEN}${BOLD}██╔═══██╗████╗ ████║██╔══██╗${CYAN}██╔════╝██║     ██╔══██╗██╔════╝${RESET}\n"
    printf "  ${GREEN}${BOLD}██║   ██║██╔████╔██║███████║${CYAN}█████╗  ██║     ███████║██║     ${RESET}\n"
    printf "  ${GREEN}${BOLD}██║   ██║██║╚██╔╝██║██╔══██║${CYAN}██╔══╝  ██║     ██╔══██║██║     ${RESET}\n"
    printf "  ${GREEN}${BOLD}╚██████╔╝██║ ╚═╝ ██║██║  ██║${CYAN}██║     ███████╗██║  ██║╚██████╗${RESET}\n"
    printf "  ${GREEN}${BOLD} ╚═════╝ ╚═╝     ╚═╝╚═╝  ╚═╝${CYAN}╚═╝     ╚══════╝╚═╝  ╚═╝ ╚═════╝${RESET}\n"
    printf "  ${DIM}Audiophile Terminal Music Player  •  Created by Jon Lawrence${RESET}\n\n"
}

info() {
    printf "  ${CYAN}▸${RESET} %b\n" "$1"
}

success() {
    printf "  ${GREEN}✓${RESET} %b\n" "$1"
}

warn() {
    printf "  ${YELLOW}!${RESET} %b\n" "$1"
}

error() {
    printf "  ${RED}✗${RESET} %b\n" "$1" >&2
}

print_banner

# Ensure common tool directories (~/.cargo/bin, /opt/homebrew/bin, /usr/local/bin) are on PATH
for dir in "$HOME/.cargo/bin" "/opt/homebrew/bin" "/usr/local/bin"; do
    if [ -d "$dir" ] && [[ ":$PATH:" != *":$dir:"* ]]; then
        export PATH="$dir:$PATH"
    fi
done

# 1. Detect OS and Linux Distribution
OS_TYPE="$(uname -s)"
ARCH_TYPE="$(uname -m)"
DISTRO="unknown"

case "$OS_TYPE" in
    Linux*)
        PLATFORM="linux"
        if [ -f /etc/os-release ]; then
            # shellcheck disable=SC1091
            . /etc/os-release
            DISTRO="${ID:-unknown}"
            DISTRO_LIKE="${ID_LIKE:-}"
        fi
        ;;
    Darwin*)
        PLATFORM="macos"
        ;;
    *)
        error "Unsupported operating system: $OS_TYPE"
        printf "  OmaFLAC supports Linux (Omarchy, Arch, Debian/Ubuntu, Fedora, etc.) and macOS.\n"
        exit 1
        ;;
esac

info "Detected platform: ${BOLD}${OS_TYPE} (${ARCH_TYPE})${RESET}${DISTRO:+ [distro: $DISTRO]}"

# 2. Check Dependencies
MISSING_DEPS=()
MISSING_RUST=0
MISSING_XCODE=0

# Check git
if ! command -v git >/dev/null 2>&1; then
    MISSING_DEPS+=("git")
fi

# Check C compiler / linker
if [ "$PLATFORM" = "macos" ]; then
    if ! xcode-select -p >/dev/null 2>&1 && ! command -v cc >/dev/null 2>&1; then
        MISSING_XCODE=1
        MISSING_DEPS+=("Xcode Command Line Tools (cc)")
    fi
else
    if ! command -v cc >/dev/null 2>&1 && ! command -v gcc >/dev/null 2>&1 && ! command -v clang >/dev/null 2>&1; then
        MISSING_DEPS+=("C compiler (gcc / clang / base-devel)")
    fi
fi

# Linux-specific audio build dependencies (pkg-config + ALSA development headers for CPAL)
if [ "$PLATFORM" = "linux" ]; then
    if ! command -v pkg-config >/dev/null 2>&1; then
        MISSING_DEPS+=("pkg-config")
    fi

    ALSA_FOUND=0
    if command -v pkg-config >/dev/null 2>&1 && pkg-config --exists alsa 2>/dev/null; then
        ALSA_FOUND=1
    elif [ -f /usr/include/alsa/asoundlib.h ] || [ -f /usr/local/include/alsa/asoundlib.h ]; then
        ALSA_FOUND=1
    fi

    if [ "$ALSA_FOUND" -eq 0 ]; then
        MISSING_DEPS+=("ALSA development headers (alsa-lib / libasound2-dev)")
    fi
fi

# Check Rust toolchain (cargo & rustc)
if ! command -v cargo >/dev/null 2>&1 || ! command -v rustc >/dev/null 2>&1; then
    MISSING_RUST=1
    MISSING_DEPS+=("Rust toolchain (cargo & rustc)")
fi

# 3. Report Missing Prerequisites with Actionable Instructions
if [ "${#MISSING_DEPS[@]}" -gt 0 ]; then
    printf "\n"
    error "${BOLD}Missing required build dependencies:${RESET}"
    for dep in "${MISSING_DEPS[@]}"; do
        printf "    ${RED}•${RESET} %s\n" "$dep"
    done

    printf "\n  ${BOLD}Please install the required dependencies first, then re-run this script:${RESET}\n\n"

    if [ "$PLATFORM" = "macos" ]; then
        if [ "$MISSING_XCODE" -eq 1 ]; then
            printf "  ${CYAN}# 1. Install Xcode Command Line Tools (includes git & C linker):${RESET}\n"
            printf "  xcode-select --install\n\n"
        elif ! command -v git >/dev/null 2>&1; then
            printf "  ${CYAN}# Install git (via Xcode Command Line Tools or Homebrew):${RESET}\n"
            printf "  xcode-select --install   ${DIM}# or: brew install git${RESET}\n\n"
        fi
    else
        # Linux distro-specific package manager suggestions
        if [[ "$DISTRO" == "arch" || "$DISTRO" == "omarchy" || "$DISTRO" == "manjaro" || "$DISTRO" == "endeavouros" || "$DISTRO_LIKE" == *"arch"* ]] || command -v pacman >/dev/null 2>&1; then
            printf "  ${CYAN}# Omarchy / Arch Linux:${RESET}\n"
            printf "  sudo pacman -S --needed base-devel pkgconf alsa-lib git rust\n\n"
        elif [[ "$DISTRO" == "ubuntu" || "$DISTRO" == "debian" || "$DISTRO" == "pop" || "$DISTRO" == "linuxmint" || "$DISTRO_LIKE" == *"debian"* || "$DISTRO_LIKE" == *"ubuntu"* ]] || command -v apt-get >/dev/null 2>&1; then
            printf "  ${CYAN}# Debian / Ubuntu / Pop!_OS / Mint:${RESET}\n"
            printf "  sudo apt update && sudo apt install -y build-essential pkg-config libasound2-dev git curl\n\n"
        elif [[ "$DISTRO" == "fedora" || "$DISTRO" == "rhel" || "$DISTRO" == "centos" || "$DISTRO_LIKE" == *"fedora"* || "$DISTRO_LIKE" == *"rhel"* ]] || command -v dnf >/dev/null 2>&1; then
            printf "  ${CYAN}# Fedora / RHEL:${RESET}\n"
            printf "  sudo dnf install -y gcc make pkgconf-pkg-config alsa-lib-devel git curl\n\n"
        elif [[ "$DISTRO" == "opensuse"* || "$DISTRO_LIKE" == *"suse"* ]] || command -v zypper >/dev/null 2>&1; then
            printf "  ${CYAN}# openSUSE:${RESET}\n"
            printf "  sudo zypper install -y gcc make pkg-config alsa-devel git curl\n\n"
        elif [[ "$DISTRO" == "alpine" ]] || command -v apk >/dev/null 2>&1; then
            printf "  ${CYAN}# Alpine Linux:${RESET}\n"
            printf "  sudo apk add build-base pkgconf alsa-lib-dev git curl\n\n"
        elif [[ "$DISTRO" == "void" ]] || command -v xbps-install >/dev/null 2>&1; then
            printf "  ${CYAN}# Void Linux:${RESET}\n"
            printf "  sudo xbps-install -S base-devel pkg-config alsa-lib-devel git curl\n\n"
        else
            printf "  ${CYAN}# Install system packages using your distribution's package manager:${RESET}\n"
            printf "  - git, C compiler (gcc/clang), pkg-config, and ALSA development headers (alsa-lib / libasound2-dev)\n\n"
        fi
    fi

    if [ "$MISSING_RUST" -eq 1 ]; then
        printf "  ${CYAN}# Install the Rust toolchain via rustup (Linux & macOS):${RESET}\n"
        printf "  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh\n"
        printf "  source \"\$HOME/.cargo/env\"\n\n"
    fi

    exit 1
fi

success "All build prerequisites found ($(rustc --version))"

# 4. Prepare Source Directory & Build
TMP_DIR=""
cleanup() {
    if [ -n "$TMP_DIR" ] && [ -d "$TMP_DIR" ]; then
        rm -rf "$TMP_DIR"
    fi
}
trap cleanup EXIT

# If running from inside an existing OmaFLAC git checkout, build directly unless OMAFLAC_FORCE_CLONE=1
if [ "${OMAFLAC_FORCE_CLONE:-0}" != "1" ] && [ -f "./Cargo.toml" ] && grep -q 'name = "omaflac"' "./Cargo.toml" 2>/dev/null; then
    SRC_DIR="$(pwd)"
    info "Building from local repository at ${BOLD}${SRC_DIR}${RESET}..."
else
    TMP_DIR="$(mktemp -d 2>/dev/null || mktemp -d -t 'omaflac-build')"
    SRC_DIR="$TMP_DIR/omaflac"
    info "Cloning OmaFLAC from ${BOLD}${REPO_URL}${RESET}..."
    git clone --depth 1 "$REPO_URL" "$SRC_DIR"
fi

info "Compiling OmaFLAC in release mode (this may take a minute)..."
(
    cd "$SRC_DIR"
    cargo build --release
)

# 5. Install Binary
mkdir -p "$INSTALL_DIR"
cp "$SRC_DIR/target/release/$BIN_NAME" "$INSTALL_DIR/$BIN_NAME"
chmod +x "$INSTALL_DIR/$BIN_NAME"

printf "\n"
success "${BOLD}OmaFLAC has been installed to ${INSTALL_DIR}/${BIN_NAME}${RESET}"

# 6. Check if INSTALL_DIR is on PATH
if [[ ":$PATH:" != *":$INSTALL_DIR:"* ]]; then
    printf "\n"
    warn "${BOLD}${INSTALL_DIR}${RESET} is not currently in your ${BOLD}PATH${RESET}."
    printf "  Add it to your shell configuration file (~/.bashrc, ~/.zshrc, or ~/.config/fish/config.fish):\n\n"
    printf "    ${CYAN}export PATH=\"\$HOME/.local/bin:\$PATH\"${RESET}\n\n"
else
    printf "\n  Run ${GREEN}${BOLD}omaflac${RESET} to launch the player!\n\n"
fi
