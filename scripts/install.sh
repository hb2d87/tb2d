#!/bin/sh
set -eu

repo="${TB2D_REPO:-hb2d87/tb2d}"
install_dir="${TB2D_INSTALL_DIR:-$HOME/.local/bin}"
version="${TB2D_VERSION:-latest}"

usage() {
  cat <<'EOF'
Install the latest TB2D release binary.

Usage: install.sh [--repo OWNER/REPO] [--version vX.Y.Z] [--install-dir PATH]

Environment overrides: TB2D_REPO, TB2D_VERSION, TB2D_INSTALL_DIR
EOF
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --repo|--version|--install-dir)
      if [ "$#" -lt 2 ]; then
        printf 'error: %s requires a value\n' "$1" >&2
        exit 2
      fi
      case "$1" in
        --repo) repo="$2" ;;
        --version) version="$2" ;;
        --install-dir) install_dir="$2" ;;
      esac
      shift 2
      ;;
    -h|--help) usage; exit 0 ;;
    *) printf 'error: unknown option: %s\n' "$1" >&2; usage >&2; exit 2 ;;
  esac
done

case "$(uname -s)-$(uname -m)" in
  Linux-x86_64) platform="linux-x86_64" ;;
  Darwin-arm64) platform="macos-aarch64" ;;
  *) printf 'error: unsupported platform: %s-%s\n' "$(uname -s)" "$(uname -m)" >&2; exit 1 ;;
esac

if [ "$version" = "latest" ]; then
  version="$(
    curl -fsSLI "https://github.com/$repo/releases/latest" |
      sed -n 's#^[Ll]ocation: .*/tag/\([^[:space:]]*\).*#\1#p' |
      tr -d '\r' |
      tail -n 1
  )"
  if [ -z "$version" ]; then
    printf 'error: could not resolve the latest release for %s\n' "$repo" >&2
    exit 1
  fi
fi

archive="tb2d-$version-$platform.tar.gz"
url="https://github.com/$repo/releases/download/$version/$archive"
tmp_dir="$(mktemp -d)"
trap 'rm -rf "$tmp_dir"' EXIT HUP INT TERM

printf 'Downloading %s\n' "$url"
curl -fsSL "$url" -o "$tmp_dir/$archive"
tar -xzf "$tmp_dir/$archive" -C "$tmp_dir"
mkdir -p "$install_dir"
cp "$tmp_dir/tb2d-$version-$platform/tb2d" "$install_dir/tb2d"
chmod +x "$install_dir/tb2d"

printf 'Installed tb2d to %s/tb2d\n' "$install_dir"
case ":$PATH:" in
  *":$install_dir:"*) ;;
  *) printf 'Add %s to PATH to run tb2d directly.\n' "$install_dir" ;;
esac
