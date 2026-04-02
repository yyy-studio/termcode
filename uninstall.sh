#!/bin/sh
# termcode uninstaller
# Usage: curl -fsSL https://raw.githubusercontent.com/yyy-studio/termcode/main/uninstall.sh | sh

set -eu

INSTALL_DIR="${HOME}/.local/bin"
CONFIG_DIR="${HOME}/.config/termcode"
BINARY="${INSTALL_DIR}/termcode"

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

ask_yn() {
    local prompt="$1" default="$2"
    local hint
    if [ "$default" = "y" ]; then hint="Y/n"; else hint="y/N"; fi
    printf '  \033[1;36m?\033[0m %s [%s]: ' "$prompt" "$hint"
    read -r yn </dev/tty
    case "$yn" in
        [Yy]*) return 0 ;;
        [Nn]*) return 1 ;;
        *)
            if [ "$default" = "y" ]; then return 0; else return 1; fi
            ;;
    esac
}

# --- main -------------------------------------------------------------------

main() {
    printf '\n  \033[1;31mtermcode uninstaller\033[0m\n\n'

    # Check if anything is installed
    local found=false
    if [ -f "$BINARY" ]; then found=true; fi
    if [ -d "$CONFIG_DIR" ]; then found=true; fi

    if [ "$found" = "false" ]; then
        info "Nothing to uninstall — termcode is not installed."
        exit 0
    fi

    # Show what will be removed
    info "The following will be removed:"
    if [ -f "$BINARY" ]; then
        printf '    - %s\n' "$BINARY"
    fi
    if [ -d "${CONFIG_DIR}/runtime" ]; then
        printf '    - %s (runtime: themes, plugins, queries)\n' "${CONFIG_DIR}/runtime"
    fi
    printf '\n'

    if ! ask_yn "Proceed with uninstall?" "n"; then
        info "Cancelled."
        exit 0
    fi

    # Remove binary
    if [ -f "$BINARY" ]; then
        rm "$BINARY"
        info "Removed binary: ${BINARY}"
    fi

    # Remove runtime (built-in data, always safe to remove)
    if [ -d "${CONFIG_DIR}/runtime" ]; then
        rm -rf "${CONFIG_DIR}/runtime"
        info "Removed runtime: ${CONFIG_DIR}/runtime"
    fi

    # Ask about user config
    if [ -f "${CONFIG_DIR}/config.toml" ] || [ -d "${CONFIG_DIR}/themes" ] || [ -d "${CONFIG_DIR}/plugins" ]; then
        printf '\n'
        info "User config found:"
        if [ -f "${CONFIG_DIR}/config.toml" ]; then
            printf '    - %s\n' "${CONFIG_DIR}/config.toml"
        fi
        if [ -d "${CONFIG_DIR}/themes" ]; then
            printf '    - %s\n' "${CONFIG_DIR}/themes/"
        fi
        if [ -d "${CONFIG_DIR}/plugins" ]; then
            printf '    - %s\n' "${CONFIG_DIR}/plugins/"
        fi
        printf '\n'

        if ask_yn "Also remove user config?" "n"; then
            rm -rf "$CONFIG_DIR"
            info "Removed config directory: ${CONFIG_DIR}"
        else
            info "User config preserved: ${CONFIG_DIR}"
        fi
    fi

    # Remove config dir if empty
    if [ -d "$CONFIG_DIR" ] && [ -z "$(ls -A "$CONFIG_DIR" 2>/dev/null)" ]; then
        rmdir "$CONFIG_DIR"
        info "Removed empty directory: ${CONFIG_DIR}"
    fi

    printf '\n  \033[1;32m%s\033[0m\n\n' "termcode uninstalled successfully!"
}

main
