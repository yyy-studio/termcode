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
        | sed -E 's/.*"tag_name": *"([^"]+)".*/\1/'
}

# --- interactive config setup -----------------------------------------------

ask() {
    # ask PROMPT DEFAULT VARNAME
    local prompt="$1" default="$2" varname="$3"
    printf '  \033[1;36m?\033[0m %s \033[2m(%s)\033[0m ' "$prompt" "$default"
    read -r answer </dev/tty
    answer="${answer:-$default}"
    eval "$varname=\$answer"
}

ask_choice() {
    # ask_choice PROMPT OPTIONS DEFAULT VARNAME
    # OPTIONS: space-separated list (e.g. "a b c")
    local prompt="$1" options="$2" default="$3" varname="$4"
    printf '  \033[1;36m?\033[0m %s\n' "$prompt"
    local i=1 opt
    for opt in $options; do
        if [ "$opt" = "$default" ]; then
            printf '    \033[1;32m%d) %s (default)\033[0m\n' "$i" "$opt"
        else
            printf '    %d) %s\n' "$i" "$opt"
        fi
        i=$((i + 1))
    done
    printf '  \033[1;36m>\033[0m Choose [1-%d]: ' "$((i - 1))"
    read -r choice </dev/tty
    if [ -z "$choice" ]; then
        eval "$varname=\$default"
        return
    fi
    local j=1
    for opt in $options; do
        if [ "$j" = "$choice" ]; then
            eval "$varname=\$opt"
            return
        fi
        j=$((j + 1))
    done
    eval "$varname=\$default"
}

ask_yn() {
    # ask_yn PROMPT DEFAULT(true/false) VARNAME
    local prompt="$1" default="$2" varname="$3"
    local hint
    if [ "$default" = "true" ]; then hint="Y/n"; else hint="y/N"; fi
    printf '  \033[1;36m?\033[0m %s [%s]: ' "$prompt" "$hint"
    read -r yn </dev/tty
    case "$yn" in
        [Yy]*) eval "$varname=true" ;;
        [Nn]*) eval "$varname=false" ;;
        *)     eval "$varname=\$default" ;;
    esac
}

setup_config() {
    printf '\n  \033[1m── Theme ──\033[0m\n'
    ask_choice "Color theme" "one-dark gruvbox-dark catppuccin-mocha lazygit" "one-dark" cfg_theme

    printf '\n  \033[1m── Editor ──\033[0m\n'
    ask "Tab size" "4" cfg_tab_size
    ask_yn "Use spaces for indentation" "true" cfg_insert_spaces
    ask_choice "Line numbers" "absolute relative relative_absolute none" "absolute" cfg_line_numbers
    ask_yn "Enable mouse" "true" cfg_mouse

    printf '\n  \033[1m── UI ──\033[0m\n'
    ask_yn "Show sidebar on startup" "true" cfg_sidebar_visible
    ask_yn "Show tree lines (├── └──)" "true" cfg_tree_style
    ask_yn "Show file type emoji icons" "true" cfg_emoji
    ask_yn "Respect .gitignore in file tree" "true" cfg_gitignore

    # Map line_numbers value to config format
    case "$cfg_line_numbers" in
        relative_absolute) cfg_line_numbers="relative_absolute" ;;
    esac

    mkdir -p "$CONFIG_DIR"
    cat > "${CONFIG_DIR}/config.toml" <<CONF
theme = "${cfg_theme}"

[editor]
tab_size = ${cfg_tab_size}
insert_spaces = ${cfg_insert_spaces}
line_numbers = "${cfg_line_numbers}"
scroll_off = 5
mouse_enabled = ${cfg_mouse}

[ui]
sidebar_width = 30
sidebar_visible = ${cfg_sidebar_visible}
show_minimap = false
show_tab_bar = true
show_top_bar = true
tree_style = ${cfg_tree_style}
show_file_type_emoji = ${cfg_emoji}
respect_gitignore = ${cfg_gitignore}
CONF

    info "Config saved to ${CONFIG_DIR}/config.toml"
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
    trap 'rm -rf "${tmpdir:-}"' EXIT

    info "Downloading ${archive_name}..."
    curl -fsSL "$url" -o "${tmpdir}/${archive_name}" \
        || err "Download failed. Check if release exists for ${target}"

    info "Extracting..."
    tar xzf "${tmpdir}/${archive_name}" -C "$tmpdir"

    # Install binary
    mkdir -p "$INSTALL_DIR"
    cp "${tmpdir}/termcode-${target}/termcode" "${INSTALL_DIR}/termcode"
    chmod +x "${INSTALL_DIR}/termcode"
    # macOS: remove quarantine attributes and ad-hoc sign so Gatekeeper won't kill the binary
    if [ "$(uname -s)" = "Darwin" ]; then
        xattr -c "${INSTALL_DIR}/termcode" 2>/dev/null || true
        codesign --force --sign - "${INSTALL_DIR}/termcode" 2>/dev/null || true
    fi
    info "Installed binary to ${INSTALL_DIR}/termcode"

    # Install runtime (overwrite built-in, preserve user config)
    mkdir -p "$RUNTIME_DIR"
    cp -r "${tmpdir}/termcode-${target}/runtime/themes"  "$RUNTIME_DIR/"
    cp -r "${tmpdir}/termcode-${target}/runtime/plugins" "$RUNTIME_DIR/"
    cp -r "${tmpdir}/termcode-${target}/runtime/queries" "$RUNTIME_DIR/"
    info "Installed runtime to ${RUNTIME_DIR}/"

    # Interactive config setup (only on first install)
    if [ ! -f "${CONFIG_DIR}/config.toml" ]; then
        info "First install detected — let's configure termcode!"
        setup_config
    else
        info "Existing config preserved: ${CONFIG_DIR}/config.toml"
    fi

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
