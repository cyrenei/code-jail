#!/bin/sh
# codejail installer - downloads a pre-built binary from GitHub Releases.
# Usage: curl -sSf https://raw.githubusercontent.com/cyrenei/containment/main/install.sh | sh
#
# Environment variables:
#   CODEJAIL_VERSION      - version to install (default: latest)
#   CODEJAIL_INSTALL_DIR  - installation directory (default: ~/.codejail/bin)

set -eu

REPO="cyrenei/containment"
INSTALL_DIR="${CODEJAIL_INSTALL_DIR:-$HOME/.codejail/bin}"

main() {
    need_cmd curl
    need_cmd tar
    need_cmd uname

    local _os _arch _target _version _url _checksum_url

    _os="$(detect_os)"
    _arch="$(detect_arch)"
    _target="${_os}-${_arch}"

    printf "Detected platform: %s\n" "$_target"

    _version="$(resolve_version)"
    printf "Installing codejail %s\n" "$_version"

    _url="https://github.com/${REPO}/releases/download/${_version}/codejail-${_target}.tar.gz"
    _checksum_url="https://github.com/${REPO}/releases/download/${_version}/checksums-sha256.txt"

    _tmpdir="$(mktemp -d)"
    trap 'rm -rf "$_tmpdir"' EXIT

    printf "Downloading codejail-%s.tar.gz...\n" "$_target"
    curl -sSfL "$_url" -o "$_tmpdir/codejail.tar.gz" || {
        err "download failed - check that release ${_version} exists at https://github.com/${REPO}/releases"
    }

    printf "Downloading checksums...\n"
    curl -sSfL "$_checksum_url" -o "$_tmpdir/checksums-sha256.txt" || {
        err "checksum download failed"
    }

    printf "Verifying SHA256 checksum...\n"
    verify_checksum "$_tmpdir" "codejail-${_target}.tar.gz"

    printf "Extracting...\n"
    tar xzf "$_tmpdir/codejail.tar.gz" -C "$_tmpdir"

    # Find the binary inside the extracted archive
    local _bin=""
    for _candidate in "$_tmpdir"/*/codejail "$_tmpdir"/codejail; do
        if [ -f "$_candidate" ]; then
            _bin="$_candidate"
            break
        fi
    done
    if [ -z "$_bin" ]; then
        err "could not find codejail binary in archive"
    fi

    mkdir -p "$INSTALL_DIR"
    cp "$_bin" "$INSTALL_DIR/codejail"
    chmod +x "$INSTALL_DIR/codejail"

    printf "\ncodejail %s installed to %s/codejail\n" "$_version" "$INSTALL_DIR"

    if ! echo "$PATH" | tr ':' '\n' | grep -qx "$INSTALL_DIR"; then
        local _line="export PATH=\"${INSTALL_DIR}:\$PATH\""
        local _profile
        _profile="$(detect_profile)"

        if [ -n "$_profile" ] && [ -t 0 ]; then
            printf "\n%s is not in your PATH.\n" "$INSTALL_DIR"
            printf "Add it to %s? [y/N] " "$_profile"
            read -r _answer </dev/tty
            case "$_answer" in
                [yY]|[yY][eE][sS])
                    printf '\n# Added by codejail installer\n%s\n' "$_line" >> "$_profile"
                    printf "Added to %s. Restart your shell or run:\n" "$_profile"
                    printf "  source %s\n" "$_profile"
                    ;;
                *)
                    printf "\nTo add manually:\n  %s\n" "$_line"
                    printf "Add that line to %s to persist across sessions.\n" "$_profile"
                    ;;
            esac
        else
            printf "\nAdd codejail to your PATH for this session:\n"
            printf "  %s\n" "$_line"
            printf "\nTo persist across sessions, add that line to your shell profile (~/.bashrc, ~/.zshrc, etc.)\n"
        fi
    fi

    printf "\nVerify: codejail --version\n"
}

detect_os() {
    local _uname
    _uname="$(uname -s)"
    case "$_uname" in
        Linux)  echo "linux" ;;
        Darwin) echo "macos" ;;
        *)      err "unsupported OS: $_uname" ;;
    esac
}

detect_arch() {
    local _uname
    _uname="$(uname -m)"
    case "$_uname" in
        x86_64|amd64)   echo "amd64" ;;
        aarch64|arm64)  echo "arm64" ;;
        *)              err "unsupported architecture: $_uname" ;;
    esac
}

resolve_version() {
    if [ -n "${CODEJAIL_VERSION:-}" ]; then
        echo "$CODEJAIL_VERSION"
        return
    fi
    local _location
    _location="$(curl -sSf -o /dev/null -w '%{redirect_url}' "https://github.com/${REPO}/releases/latest")" || {
        err "could not determine latest version - set CODEJAIL_VERSION explicitly or check https://github.com/${REPO}/releases"
    }
    local _tag="${_location##*/}"
    if [ -z "$_tag" ]; then
        err "could not determine latest version - no releases found. Set CODEJAIL_VERSION explicitly."
    fi
    echo "$_tag"
}

verify_checksum() {
    local _dir="$1" _filename="$2"
    local _expected _actual

    _expected="$(grep "$_filename" "$_dir/checksums-sha256.txt" | awk '{print $1}')"
    if [ -z "$_expected" ]; then
        err "no checksum found for $_filename in checksums-sha256.txt"
    fi

    if command -v sha256sum >/dev/null 2>&1; then
        _actual="$(sha256sum "$_dir/codejail.tar.gz" | awk '{print $1}')"
    elif command -v shasum >/dev/null 2>&1; then
        _actual="$(shasum -a 256 "$_dir/codejail.tar.gz" | awk '{print $1}')"
    else
        err "no sha256sum or shasum found - cannot verify checksum"
    fi

    if [ "$_expected" != "$_actual" ]; then
        err "checksum mismatch!
  expected: $_expected
  actual:   $_actual
The downloaded file may be corrupted or tampered with. Aborting."
    fi

    printf "Checksum verified: %s\n" "$_actual"
}

detect_profile() {
    local _shell
    _shell="$(basename "${SHELL:-/bin/sh}")"
    case "$_shell" in
        zsh)  echo "$HOME/.zshrc" ;;
        bash)
            if [ -f "$HOME/.bashrc" ]; then
                echo "$HOME/.bashrc"
            elif [ -f "$HOME/.bash_profile" ]; then
                echo "$HOME/.bash_profile"
            else
                echo "$HOME/.profile"
            fi
            ;;
        *)    echo "$HOME/.profile" ;;
    esac
}

need_cmd() {
    if ! command -v "$1" >/dev/null 2>&1; then
        err "required command not found: $1"
    fi
}

err() {
    printf "error: %s\n" "$1" >&2
    exit 1
}

main "$@"
