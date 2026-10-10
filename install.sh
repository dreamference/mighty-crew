#!/bin/sh
# Install the latest crew release binary for this machine into ~/.local/bin (or $INSTALL_DIR).
#   curl -fsSL https://raw.githubusercontent.com/dreamference/mighty-crew/main/install.sh | sh
# CREW_VERSION=0.1.1 pins a version. Needs curl or wget, and tar.
set -eu
REPO=dreamference/mighty-crew
INSTALL_DIR=${INSTALL_DIR:-$HOME/.local/bin}
os=$(uname -s); arch=$(uname -m)
case "$os" in
  Linux)  case "$arch" in x86_64|amd64) target=x86_64-unknown-linux-musl ;; aarch64|arm64) target=aarch64-unknown-linux-musl ;; *) echo "crew: no prebuilt binary for Linux $arch; build with cargo" >&2; exit 1 ;; esac ;;
  Darwin) case "$arch" in arm64) target=aarch64-apple-darwin ;; x86_64) target=x86_64-apple-darwin ;; *) echo "crew: no prebuilt binary for macOS $arch" >&2; exit 1 ;; esac ;;
  *) echo "crew: $os is not covered by this script; on Windows use install.ps1" >&2; exit 1 ;;
esac
fetch() { if command -v curl >/dev/null; then curl -fsSL "$1"; else wget -qO- "$1"; fi; }
if [ -n "${CREW_VERSION:-}" ]; then
  version=$CREW_VERSION
else
  version=$(fetch "https://api.github.com/repos/$REPO/releases/latest" | sed -n 's/.*"tag_name": *"v\([^"]*\)".*/\1/p' | head -n1)
  [ -n "$version" ] || { echo "crew: cannot read the latest release from GitHub" >&2; exit 1; }
fi
name="crew-$version-$target"
url="https://github.com/$REPO/releases/download/v$version/$name.tar.gz"
tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
echo "downloading $url"
fetch "$url" > "$tmp/$name.tar.gz"
if command -v shasum >/dev/null || command -v sha256sum >/dev/null; then
  fetch "$url.sha256" > "$tmp/$name.tar.gz.sha256" || true
  if [ -s "$tmp/$name.tar.gz.sha256" ]; then
    want=$(cut -d' ' -f1 "$tmp/$name.tar.gz.sha256")
    have=$( (command -v sha256sum >/dev/null && sha256sum "$tmp/$name.tar.gz" || shasum -a 256 "$tmp/$name.tar.gz") | cut -d' ' -f1)
    [ "$want" = "$have" ] || { echo "crew: checksum mismatch for $name.tar.gz" >&2; exit 1; }
  fi
fi
tar -C "$tmp" -xzf "$tmp/$name.tar.gz"
mkdir -p "$INSTALL_DIR"
install -m 755 "$tmp/$name/crew" "$INSTALL_DIR/crew"
echo "installed crew $version to $INSTALL_DIR/crew"
case ":$PATH:" in *":$INSTALL_DIR:"*) ;; *) echo "add $INSTALL_DIR to your PATH" ;; esac
"$INSTALL_DIR/crew" --version
