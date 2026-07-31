#!/bin/sh
set -eu

repository='bllyanos/termdown'
release_asset='termdown-linux-x86_64.tar.gz'
install_root=${TERMDOWN_INSTALL_ROOT:-"${HOME:-}/.local"}

usage() {
    cat <<'EOF'
Install the latest termdown GitHub release.

Environment:
  TERMDOWN_INSTALL_ROOT  Installation prefix (default: ~/.local)
EOF
}

if [ "${1:-}" = "--help" ] || [ "${1:-}" = "-h" ]; then
    usage
    exit 0
fi

if [ "$#" -ne 0 ]; then
    printf '%s\n' 'error: unexpected arguments; use --help for usage' >&2
    exit 2
fi

if [ -z "${HOME:-}" ]; then
    printf '%s\n' 'error: HOME is not set' >&2
    exit 1
fi

case "$(uname -s):$(uname -m)" in
    Linux:x86_64|Linux:amd64) ;;
    *)
        printf '%s\n' 'error: prebuilt releases currently support Linux x86_64 only' >&2
        exit 1
        ;;
esac

for command in curl tar install mktemp sha256sum; do
    if ! command -v "$command" >/dev/null 2>&1; then
        printf 'error: required command not found: %s\n' "$command" >&2
        exit 1
    fi
done

download_url="https://github.com/$repository/releases/latest/download/$release_asset"
temporary_directory=$(mktemp -d "${TMPDIR:-/tmp}/termdown.XXXXXX")
trap 'rm -rf "$temporary_directory"' EXIT HUP INT TERM

printf 'Downloading the latest termdown release...\n'
curl --proto '=https' --tlsv1.2 --fail --location --silent --show-error \
    "$download_url" \
    --output "$temporary_directory/$release_asset"
curl --proto '=https' --tlsv1.2 --fail --location --silent --show-error \
    "$download_url.sha256" \
    --output "$temporary_directory/$release_asset.sha256"

(
    cd "$temporary_directory"
    sha256sum -c "$release_asset.sha256"
)

tar -xzf "$temporary_directory/$release_asset" -C "$temporary_directory"
mkdir -p "$install_root/bin"
install -m 0755 \
    "$temporary_directory/termdown/termdown" \
    "$install_root/bin/termdown"

printf '\ntermdown installed at %s/bin/termdown\n' "$install_root"
case ":${PATH:-}:" in
    *:"$install_root/bin":*) ;;
    *) printf 'Add %s/bin to PATH to run termdown directly.\n' "$install_root" ;;
esac
