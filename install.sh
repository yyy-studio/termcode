#!/bin/sh
# termcode installer
# Usage: curl -fsSL https://raw.githubusercontent.com/yyy-studio/termcode/main/install.sh | sh

set -eu

REPO="yyy-studio/termcode"
INSTALL_DIR="${HOME}/.local/bin"
CONFIG_DIR="${HOME}/.config/termcode"
RUNTIME_DIR="${CONFIG_DIR}/runtime"

# --- helpers ----------------------------------------------------------------

info() {
    printf '  \033[1;34m>\033[0m %s\n' "$@"
}

warn() {
    printf '  \033[1;33m!\033[0m %s\n' "$@"
}

err() {
    printf '  \033[1;31mx\033[0m %s\n' "$@" >&2
    exit 1
}

need() {
    command -v "$1" >/dev/null 2>&1 || err "'$1' is required but not found"
}

# --- detect platform --------------------------------------------------------

detect_platform() {
    local os arch

    os="$(uname -s)"
    arch="$(uname -m)"

    case "$os" in
        Darwin) os="apple-darwin" ;;
        Linux)  os="unknown-linux-gnu" ;;
        *)      err "Unsupported OS: $os" ;;
    esac

    case "$arch" in
        x86_64|amd64)   arch="x86_64" ;;
        arm64|aarch64)   arch="aarch64" ;;
        *)               err "Unsupported architecture: $arch" ;;
    esac

    echo "${arch}-${os}"
}

# --- fetch latest release tag -----------------------------------------------

latest_tag() {
    curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
        | grep '"tag_name"' \
        | sed -E 's/.*"tag_name":\s*"([^"]+)".*/\1/'
}

# --- main -------------------------------------------------------------------

main() {
    need curl
    need tar

    local target tag archive_name url tmpdir

    target="$(detect_platform)"
    info "Detected platform: ${target}"

    info "Fetching latest release..."
    tag="$(latest_tag)"
    if [ -z "$tag" ]; then
        err "Could not determine latest release tag"
    fi
    info "Latest release: ${tag}"

    archive_name="termcode-${target}.tar.gz"
    url="https://github.com/${REPO}/releases/download/${tag}/${archive_name}"

    tmpdir="$(mktemp -d)"
    trap 'rm -rf "$tmpdir"' EXIT

    info "Downloading ${archive_name}..."
    curl -fsSL "$url" -o "${tmpdir}/${archive_name}" \
        || err "Download failed. Check if release exists for ${target}"

    info "Extracting..."
    tar xzf "${tmpdir}/${archive_name}" -C "$tmpdir"

    # Install binary
    mkdir -p "$INSTALL_DIR"
    cp "${tmpdir}/termcode-${target}/termcode" "${INSTALL_DIR}/termcode"
    chmod +x "${INSTALL_DIR}/termcode"
    info "Installed binary to ${INSTALL_DIR}/termcode"

    # Install runtime (overwrite built-in, preserve user config)
    mkdir -p "$RUNTIME_DIR"
    cp -r "${tmpdir}/termcode-${target}/runtime/themes"  "$RUNTIME_DIR/"
    cp -r "${tmpdir}/termcode-${target}/runtime/plugins" "$RUNTIME_DIR/"
    cp -r "${tmpdir}/termcode-${target}/runtime/queries" "$RUNTIME_DIR/"
    info "Installed runtime to ${RUNTIME_DIR}/"

    # Check PATH
    case ":${PATH}:" in
        *":${INSTALL_DIR}:"*) ;;
        *)
            warn "${INSTALL_DIR} is not in your PATH"
            warn "Add this to your shell profile:"
            warn "  export PATH=\"\${HOME}/.local/bin:\${PATH}\""
            ;;
    esac

    printf '\n  \033[1;32m%s\033[0m %s\n\n' "termcode ${tag}" "installed successfully!"
}

main
