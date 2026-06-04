#!/bin/sh
set -eu

repo="${TB2D_REPO:-hb2d87/tb2d}"
install_dir="${TB2D_INSTALL_DIR:-$HOME/.local/bin}"
config_dir="${TB2D_CONFIG_DIR:-${XDG_CONFIG_HOME:-$HOME/.config}/tb2d}"
version="${TB2D_VERSION:-latest}"
path_update="${TB2D_PATH_UPDATE:-auto}"

usage() {
  cat <<'EOF'
Install the latest tb2d release binary and starter YAML config.

Usage: install.sh [--repo OWNER/REPO] [--version vX.Y.Z] [--install-dir PATH] [--config-dir PATH] [--no-path-update]

Environment overrides: TB2D_REPO, TB2D_VERSION, TB2D_INSTALL_DIR, TB2D_CONFIG_DIR, TB2D_PATH_UPDATE
EOF
}

profile_path() {
  shell_name="${SHELL##*/}"
  case "$shell_name" in
    zsh) printf '%s\n' "$HOME/.zshrc" ;;
    bash) printf '%s\n' "$HOME/.bashrc" ;;
    *) printf '%s\n' "$HOME/.profile" ;;
  esac
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --repo|--version|--install-dir|--config-dir)
      if [ "$#" -lt 2 ]; then
        printf 'error: %s requires a value\n' "$1" >&2
        exit 2
      fi
      case "$1" in
        --repo) repo="$2" ;;
        --version) version="$2" ;;
        --install-dir) install_dir="$2" ;;
        --config-dir) config_dir="$2" ;;
      esac
      shift 2
      ;;
    --no-path-update) path_update="never"; shift ;;
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
mkdir -p "$config_dir"
if [ ! -f "$config_dir/default.yaml" ]; then
  cp "$tmp_dir/tb2d-$version-$platform/default.yaml" "$config_dir/default.yaml"
fi
if [ ! -f "$config_dir/web-reader.yaml" ]; then
  cp "$tmp_dir/tb2d-$version-$platform/web-reader.yaml" "$config_dir/web-reader.yaml"
fi

printf 'Installed tb2d to %s/tb2d\n' "$install_dir"
printf 'Installed starter YAML configs to %s\n' "$config_dir"
printf 'Edit the default config with: tb2d --config\n'
case ":$PATH:" in
  *":$install_dir:"*) ;;
  *)
    if [ "$path_update" = "never" ]; then
      printf 'Add %s to PATH to run tb2d directly.\n' "$install_dir"
    else
      profile_file="$(profile_path)"
      mkdir -p "$(dirname "$profile_file")"
      if [ ! -f "$profile_file" ] || ! grep -F "export PATH=\"$install_dir:\$PATH\"" "$profile_file" >/dev/null 2>&1; then
        {
          printf '\n# Added by tb2d installer\n'
          printf 'export PATH="%s:$PATH"\n' "$install_dir"
        } >> "$profile_file"
      fi
      printf 'Added %s to PATH in %s\n' "$install_dir" "$profile_file"
      printf 'Open a new terminal, or run: export PATH="%s:$PATH"\n' "$install_dir"
    fi
    ;;
esac
