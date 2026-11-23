#!/bin/sh
# Kensaku installer for Linux and macOS.
#
#   curl -fsSL https://github.com/yjid/kensaku/releases/latest/download/install.sh | sh
#
# It reads the release manifest, downloads the right artifact, verifies its
# SHA-256 against the release's SHA256SUMS, and installs into your user account:
#
#   Linux  -> AppImage in ${XDG_DATA_HOME:-~/.local/share}/kensaku
#   macOS  -> kensaku.app in ~/Applications
#
# No root is required by default. Use --system for /Applications or the native
# .deb/.rpm package on Linux. The in-app updater handles later releases for the
# AppImage; .deb/.rpm installs update through the package manager.
#
# This script is deliberately plain POSIX sh. Read it first if you prefer:
#   curl -fsSLO https://github.com/yjid/kensaku/releases/latest/download/install.sh
#   less install.sh && sh install.sh

set -eu

REPO="${KENSAKU_REPO:-yjid/kensaku}"
APP_NAME="kensaku"
MANIFEST_URL="${KENSAKU_MANIFEST_URL:-}"
SUMS_URL="${KENSAKU_SUMS_URL:-}"
VERSION="latest"
PREFIX=""
MODE="user"
DRY_RUN=0
VERIFY_ONLY=0
ALLOW_UNSIGNED=0
NO_LINK=0

usage() {
  cat <<'EOF'
Kensaku installer

Usage:
  install.sh [options]

Options:
  --version <v>     Install a specific release, for example 1.0.19 or v1.0.19.
  --prefix <dir>    Linux: where to keep the AppImage.
  --system          Linux: install the .deb/.rpm with apt/dnf.
                    macOS: install to /Applications (asks for sudo).
  --allow-unsigned  macOS: clear the quarantine flag on the installed app.
                    Only use this if you trust the release and understand that
                    the app is not notarized yet.
  --no-link         Linux: do not create the ~/.local/bin/kensaku symlink.
  --dry-run         Print the resolved artifact and destination, download nothing.
  --verify-only     Download and verify the artifact, do not install it.
  --manifest <url>  Use a different latest.json (mirrors/testing).
  --sums <url>      Use a different SHA256SUMS (mirrors/testing).
  -h, --help        Show this help.

Examples:
  curl -fsSL .../install.sh | sh
  curl -fsSL .../install.sh | sh -s -- --system
  curl -fsSL .../install.sh | sh -s -- --version 1.0.19 --dry-run
EOF
}

die() {
  printf 'kensaku: %s\n' "$*" >&2
  exit 1
}

note() {
  printf '%s\n' "$*"
}

need() {
  command -v "$1" >/dev/null 2>&1 || die "Required command not found: $1"
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --version)
      [ "$#" -ge 2 ] || die "--version needs a value"
      VERSION="$2"
      shift 2
      ;;
    --version=*)
      VERSION="${1#*=}"
      shift
      ;;
    --prefix)
      [ "$#" -ge 2 ] || die "--prefix needs a value"
      PREFIX="$2"
      shift 2
      ;;
    --prefix=*)
      PREFIX="${1#*=}"
      shift
      ;;
    --system)
      MODE="system"
      shift
      ;;
    --allow-unsigned)
      ALLOW_UNSIGNED=1
      shift
      ;;
    --no-link)
      NO_LINK=1
      shift
      ;;
    --dry-run)
      DRY_RUN=1
      shift
      ;;
    --verify-only)
      VERIFY_ONLY=1
      shift
      ;;
    --manifest)
      [ "$#" -ge 2 ] || die "--manifest needs a value"
      MANIFEST_URL="$2"
      shift 2
      ;;
    --manifest=*)
      MANIFEST_URL="${1#*=}"
      shift
      ;;
    --sums)
      [ "$#" -ge 2 ] || die "--sums needs a value"
      SUMS_URL="$2"
      shift 2
      ;;
    --sums=*)
      SUMS_URL="${1#*=}"
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      die "Unknown option: $1 (try --help)"
      ;;
  esac
done

need uname
need mkdir
need mktemp
need sed
need grep
need awk
need head
need basename
need chmod

OS="$(uname -s)"
ARCH="$(uname -m)"

case "$ARCH" in
  x86_64|amd64) ARCH_KEY="x86_64"; DEB_ARCH="amd64"; RPM_ARCH="x86_64" ;;
  arm64|aarch64) ARCH_KEY="aarch64"; DEB_ARCH="arm64"; RPM_ARCH="aarch64" ;;
  *) die "Unsupported architecture: $ARCH" ;;
esac

case "$VERSION" in
  latest) VERSION_TAG="latest" ;;
  v*) VERSION_TAG="$VERSION"; VERSION="${VERSION#v}" ;;
  *) VERSION_TAG="v$VERSION" ;;
esac

if [ -z "$MANIFEST_URL" ]; then
  if [ "$VERSION" = "latest" ]; then
    MANIFEST_URL="https://github.com/$REPO/releases/latest/download/latest.json"
  else
    MANIFEST_URL="https://github.com/$REPO/releases/download/v$VERSION/latest.json"
  fi
fi

case "$OS" in
  Linux|Darwin) ;;
  *) die "This installer supports Linux and macOS. On Windows use install.ps1." ;;
esac

need curl
need tar

# sha256 prints the lowercase hex digest of one file.
sha256() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$1" | awk '{print $1}'
  else
    die "Neither sha256sum nor shasum is available"
  fi
}

TMP=""
cleanup() {
  if [ -n "$TMP" ] && [ -d "$TMP" ]; then
    rm -rf "$TMP"
  fi
}
trap cleanup EXIT HUP INT TERM

download() {
  curl --proto '=https' --tlsv1.2 -fsSL "$1" -o "$2"
}

note "Fetching $MANIFEST_URL"
TMP="$(mktemp -d "${TMPDIR:-/tmp}/kensaku-install.XXXXXX")"
MANIFEST="$TMP/latest.json"
case "$MANIFEST_URL" in
  http://*|https://*) download "$MANIFEST_URL" "$MANIFEST" || die "Could not download the release manifest" ;;
  *)
    [ -f "$MANIFEST_URL" ] || die "Manifest not found: $MANIFEST_URL"
    cp "$MANIFEST_URL" "$MANIFEST"
    ;;
esac

MANIFEST_VERSION="$(sed -n 's/.*"version"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' "$MANIFEST" | head -n1)"
[ -n "$MANIFEST_VERSION" ] || die "The release manifest has no version"

if [ "$VERSION" = "latest" ]; then
  VERSION="$MANIFEST_VERSION"
elif [ "$VERSION" != "$MANIFEST_VERSION" ]; then
  die "Manifest is for $MANIFEST_VERSION, not $VERSION"
fi

platform_url() {
  awk -v key="\"$1\"" '
    index($0, key) { found=1 }
    found && /"url"/ {
      line=$0
      sub(/^.*"url"[[:space:]]*:[[:space:]]*"/, "", line)
      sub(/".*$/, "", line)
      print line
      exit
    }
  ' "$MANIFEST"
}

case "$OS" in
  Linux)
    KEY="linux-$ARCH_KEY"
    URL="$(platform_url "$KEY")"
    [ -n "$URL" ] || die "No Linux artifact for $ARCH in the release manifest"
    ;;
  Darwin)
    KEY="darwin-$ARCH_KEY"
    URL="$(platform_url "$KEY")"
    [ -n "$URL" ] || die "No macOS artifact for $ARCH in the release manifest"
    ;;
esac

ASSET="$(basename "$URL")"
if [ -z "$SUMS_URL" ]; then
  SUMS_URL="https://github.com/$REPO/releases/download/v$VERSION/SHA256SUMS"
fi

if [ "$DRY_RUN" = "1" ]; then
  note "Version:   $VERSION"
  note "Artifact:  $URL"
  note "SHA256SUMS: $SUMS_URL"
  if [ "$OS" = "Linux" ] && [ "$MODE" = "system" ]; then
    note "Mode:      native .deb/.rpm package via the system package manager"
  elif [ "$OS" = "Darwin" ] && [ "$MODE" = "system" ]; then
    note "Install:   /Applications/kensaku.app (sudo)"
  else
    note "Install:   user account (no root)"
  fi
  exit 0
fi

note "Downloading Kensaku $VERSION ($ASSET)"
ARTIFACT="$TMP/$ASSET"
download "$URL" "$ARTIFACT" || die "Could not download $ASSET"

note "Verifying SHA-256"
SUMS="$TMP/SHA256SUMS"
download "$SUMS_URL" "$SUMS" || die "Could not download SHA256SUMS"
EXPECTED="$(awk -v name="$ASSET" '
  {
    file=$2
    sub(/^\*/, "", file)
    if (file == name) { print $1; exit }
  }
' "$SUMS")"
[ -n "$EXPECTED" ] || die "$ASSET is not listed in SHA256SUMS"
ACTUAL="$(sha256 "$ARTIFACT")"
[ "$EXPECTED" = "$ACTUAL" ] || die "SHA-256 mismatch for $ASSET (expected $EXPECTED, got $ACTUAL)"
note "SHA-256 OK"

if [ "$VERIFY_ONLY" = "1" ]; then
  note "Verified $ASSET. Nothing installed (--verify-only)."
  exit 0
fi

if [ "$OS" = "Linux" ] && [ "$MODE" = "system" ]; then
  # Native package path. Arch has no official package yet; use the AppImage.
  if command -v apt-get >/dev/null 2>&1; then
    PKG_EXT="deb"
    PKG_ARCH_TOKEN="$DEB_ARCH"
    PKG_INSTALL="apt-get install -y"
  elif command -v dnf >/dev/null 2>&1; then
    PKG_EXT="rpm"
    PKG_ARCH_TOKEN="$RPM_ARCH"
    PKG_INSTALL="dnf install -y"
  elif command -v yum >/dev/null 2>&1; then
    PKG_EXT="rpm"
    PKG_ARCH_TOKEN="$RPM_ARCH"
    PKG_INSTALL="yum install -y"
  else
    die "No supported package manager found. Run without --system to install the AppImage."
  fi

  # Ask the release for the exact asset name instead of guessing Tauri's
  # .deb/.rpm filename. Fall back to the known convention if the API is rate
  # limited or unavailable.
  PKG_URL=""
  RELEASE_JSON="$TMP/release.json"
  if curl --proto '=https' --tlsv1.2 -fsSL \
      "https://api.github.com/repos/$REPO/releases/tags/v$VERSION" \
      -o "$RELEASE_JSON" 2>/dev/null; then
    PKG_URL="$(grep -o "https://github.com/$REPO/releases/download/v$VERSION/[^\"]*\.$PKG_EXT" "$RELEASE_JSON" \
      | grep "$PKG_ARCH_TOKEN" | head -n1)"
  fi
  if [ -z "$PKG_URL" ]; then
    case "$PKG_EXT" in
      deb) PKG_URL="https://github.com/$REPO/releases/download/v$VERSION/kensaku_${VERSION}_${DEB_ARCH}.deb" ;;
      rpm) PKG_URL="https://github.com/$REPO/releases/download/v$VERSION/kensaku-${VERSION}-1.${RPM_ARCH}.rpm" ;;
    esac
  fi
  PKG_NAME="$(basename "$PKG_URL")"

  note "Downloading $PKG_NAME"
  PKG="$TMP/$PKG_NAME"
  download "$PKG_URL" "$PKG" || die "Could not download $PKG_NAME (is this architecture published?)"
  PKG_EXPECTED="$(awk -v name="$PKG_NAME" '
    { file=$2; sub(/^\*/, "", file); if (file == name) { print $1; exit } }
  ' "$SUMS")"
  [ -n "$PKG_EXPECTED" ] || die "$PKG_NAME is not listed in SHA256SUMS"
  PKG_ACTUAL="$(sha256 "$PKG")"
  [ "$PKG_EXPECTED" = "$PKG_ACTUAL" ] || die "SHA-256 mismatch for $PKG_NAME"
  note "Installing $PKG_NAME (sudo)"
  sudo $PKG_INSTALL "$PKG" || die "Package installation failed"
  note "Kensaku $VERSION installed through the system package manager."
  note "Future updates for this install come from the package manager, not the in-app updater."
  exit 0
fi

if [ "$OS" = "Linux" ]; then
  if [ -z "$PREFIX" ]; then
    PREFIX="${XDG_DATA_HOME:-$HOME/.local/share}/kensaku"
  fi
  BIN_DIR="${XDG_BIN_HOME:-$HOME/.local/bin}"
  APPLICATIONS_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/applications"
  ICON_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/icons/hicolor/512x512/apps"

  mkdir -p "$PREFIX" "$APPLICATIONS_DIR" "$ICON_DIR"
  TARGET="$PREFIX/kensaku.AppImage"
  mv "$ARTIFACT" "$TARGET"
  chmod 755 "$TARGET"

  # The AppImage bundles its own icon. Extraction does not need FUSE.
  (
    cd "$TMP"
    "$TARGET" --appimage-extract '*.png' >/dev/null 2>&1
  ) || true
  ICON_SRC="$(find "$TMP/squashfs-root" -type f -name '*.png' 2>/dev/null | head -n1 || true)"
  if [ -n "$ICON_SRC" ]; then
    cp "$ICON_SRC" "$ICON_DIR/kensaku.png"
    ICON_PATH="$ICON_DIR/kensaku.png"
  else
    ICON_PATH="kensaku"
  fi

  cat > "$APPLICATIONS_DIR/kensaku.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=Kensaku
Comment=Fast local search for files, documents and images
Exec="$TARGET"
Icon="$ICON_PATH"
Terminal=false
Categories=Utility;FileTools;
StartupWMClass=kensaku
EOF
  chmod 644 "$APPLICATIONS_DIR/kensaku.desktop"

  if [ "$NO_LINK" != "1" ]; then
    mkdir -p "$BIN_DIR"
    ln -sf "$TARGET" "$BIN_DIR/kensaku"
    case ":$PATH:" in
      *":$BIN_DIR:"*) ;;
      *) note "Add $BIN_DIR to PATH to run 'kensaku' from a terminal." ;;
    esac
  fi

  if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$APPLICATIONS_DIR" >/dev/null 2>&1 || true
  fi

  note "Kensaku $VERSION installed to $TARGET"
  note "Launch it from your application menu or run: $TARGET"
  note "Later releases update in-app (AppImage)."
  exit 0
fi

# macOS
if [ -z "$PREFIX" ]; then
  if [ "$MODE" = "system" ]; then
    PREFIX="/Applications"
  else
    PREFIX="$HOME/Applications"
  fi
fi
mkdir -p "$PREFIX"

# Stop an older copy so replacing the bundle is not half-written.
osascript -e 'tell application "Kensaku" to quit' >/dev/null 2>&1 || true
osascript -e 'tell application "kensaku" to quit' >/dev/null 2>&1 || true

WORK="$TMP/app"
mkdir -p "$WORK"
tar -xzf "$ARTIFACT" -C "$WORK" || die "Could not extract $ASSET"
APP_SRC="$(find "$WORK" -maxdepth 2 -type d -name '*.app' | head -n1 || true)"
[ -n "$APP_SRC" ] || die "The macOS archive did not contain an .app bundle"
APP_NAME="$(basename "$APP_SRC")"
APP_DEST="$PREFIX/$APP_NAME"

if [ "$MODE" = "system" ]; then
  note "Installing to $APP_DEST (sudo)"
  sudo rm -rf "$APP_DEST"
  sudo ditto "$APP_SRC" "$APP_DEST"
else
  rm -rf "$APP_DEST"
  ditto "$APP_SRC" "$APP_DEST"
fi

if [ "$ALLOW_UNSIGNED" = "1" ]; then
  note "Clearing the quarantine flag on $APP_DEST (--allow-unsigned)"
  if [ "$MODE" = "system" ]; then
    sudo xattr -dr com.apple.quarantine "$APP_DEST" 2>/dev/null || true
  else
    xattr -dr com.apple.quarantine "$APP_DEST" 2>/dev/null || true
  fi
elif xattr -p com.apple.quarantine "$APP_DEST" >/dev/null 2>&1; then
  note "macOS tagged the app as downloaded. If it will not open:"
  note "  System Settings -> Privacy & Security -> Open Anyway"
  note "  or right-click the app in Finder -> Open -> Open"
  note "Re-run with --allow-unsigned to clear the tag for this app only."
fi

note "Kensaku $VERSION installed to $APP_DEST"
note "Open it from Launchpad, Spotlight, or Finder."
note "macOS builds are not notarized yet; folder-access prompts may repeat after upgrades."
