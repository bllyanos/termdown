#!/bin/sh
set -eu

repository='https://github.com/bllyanos/termdown.git'
branch='main'
install_root=${TERMDOWN_INSTALL_ROOT:-"${HOME:-}/.local"}

usage() {
    cat <<'EOF'
Install termdown from the GitHub repository.

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

if ! command -v cargo >/dev/null 2>&1; then
    printf '%s\n' 'error: Cargo is required; install Rust from https://rustup.rs/' >&2
    exit 1
fi

printf 'Installing termdown from %s (%s) into %s/bin...\n' "$repository" "$branch" "$install_root"
mkdir -p "$install_root/bin"
cargo install \
    --git "$repository" \
    --branch "$branch" \
    --root "$install_root" \
    --locked \
    --force

printf '\ntermdown installed at %s/bin/termdown\n' "$install_root"
case ":${PATH:-}:" in
    *:"$install_root/bin":*) ;;
    *) printf 'Add %s/bin to PATH to run termdown directly.\n' "$install_root" ;;
esac
