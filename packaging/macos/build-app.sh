#!/bin/bash
# Crea "DevLogica Tools.app" con firma ad-hoc (nessun account sviluppatore richiesto).
# Uso:  ./packaging/macos/build-app.sh            → per l'architettura di questo Mac
#       ./packaging/macos/build-app.sh universal  → Apple Silicon + Intel in un'unica app
set -euo pipefail
cd "$(dirname "$0")/../.."

APP="dist/DevLogica Tools.app"
BIN=devlogica-tools
VERSION=$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)

if [[ "${1:-}" == "universal" ]]; then
  rustup target add aarch64-apple-darwin x86_64-apple-darwin
  cargo build --release --target aarch64-apple-darwin
  cargo build --release --target x86_64-apple-darwin
  mkdir -p target/universal
  lipo -create -output "target/universal/$BIN" \
    "target/aarch64-apple-darwin/release/$BIN" "target/x86_64-apple-darwin/release/$BIN"
  BUILT="target/universal/$BIN"
else
  cargo build --release
  BUILT="target/release/$BIN"
fi

rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$BUILT" "$APP/Contents/MacOS/$BIN"
sed "s/__VERSION__/$VERSION/g" packaging/macos/Info.plist > "$APP/Contents/Info.plist"

# Icona .icns a partire dal PNG 1024×1024
ICONSET=$(mktemp -d)/AppIcon.iconset
mkdir -p "$ICONSET"
for s in 16 32 128 256 512; do
  sips -z $s $s assets/app-icon-1024.png --out "$ICONSET/icon_${s}x${s}.png" >/dev/null
  sips -z $((s*2)) $((s*2)) assets/app-icon-1024.png --out "$ICONSET/icon_${s}x${s}@2x.png" >/dev/null
done
iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/AppIcon.icns"

# Firma ad-hoc: obbligatoria su Apple Silicon, non richiede certificati.
codesign --force --deep --sign - "$APP"

echo "Creata: $APP"
