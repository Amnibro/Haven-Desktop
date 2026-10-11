#!/bin/bash
cd "$(dirname "$0")"

echo ""
echo " Haven Desktop — macOS App & DMG Builder"
echo " ======================================"
echo ""

if ! command -v node >/dev/null 2>&1; then
  echo "[ERROR] Node.js not found. Install from https://nodejs.org"
  read -p "Press Enter to exit..."
  exit 1
fi

if ! command -v cargo >/dev/null 2>&1; then
  echo "[ERROR] Cargo/Rust not found. Install from https://rustup.rs"
  read -p "Press Enter to exit..."
  exit 1
fi

VERSION=$(node -p "require('./src-tauri/tauri.conf.json').version")

# Sign with a Developer ID from the login keychain when one is installed.
# Set APPLE_SIGNING_IDENTITY yourself to pick a different certificate.
if [ -z "$APPLE_SIGNING_IDENTITY" ]; then
  APPLE_SIGNING_IDENTITY=$(security find-identity -v -p codesigning 2>/dev/null \
    | sed -n 's/.*"\(Developer ID Application: .*\)"/\1/p' | head -1)
fi
if [ -n "$APPLE_SIGNING_IDENTITY" ]; then
  export APPLE_SIGNING_IDENTITY
  echo "Signing with: $APPLE_SIGNING_IDENTITY"
else
  echo "No Developer ID certificate found; the app will be unsigned."
fi
echo ""

echo "[1/3] Installing npm dependencies..."
npm install || { echo "[ERROR] npm install failed"; exit 1; }

echo "[2/3] Building macOS application bundle..."
if [ -z "$TAURI_SIGNING_PRIVATE_KEY" ]; then
  npm run tauri build -- --bundles app -c '{"bundle": {"createUpdaterArtifacts": false}}'
else
  npm run tauri build -- --bundles app
fi

echo "[3/3] Creating macOS disk image (DMG) with drag-and-drop installer..."
APP_BUNDLE="src-tauri/target/release/bundle/macos/Haven.app"
DMG_OUT="src-tauri/target/release/bundle/dmg/Haven_${VERSION}_macOS.dmg"
mkdir -p "src-tauri/target/release/bundle/dmg"

if [ -d "$APP_BUNDLE" ]; then
  DMG_DIR=$(mktemp -d)
  cp -R "$APP_BUNDLE" "$DMG_DIR/"
  ln -s /Applications "$DMG_DIR/Applications"
  hdiutil create -volname "Haven" -srcfolder "$DMG_DIR" -ov -format UDZO "$DMG_OUT"
  rm -rf "$DMG_DIR"
  if [ -n "$APPLE_SIGNING_IDENTITY" ]; then
    codesign --force --sign "$APPLE_SIGNING_IDENTITY" "$DMG_OUT"
  fi
fi

echo ""
echo "Done! Created macOS distribution artifacts:"
echo "  • Native App Bundle:  src-tauri/target/release/bundle/macos/Haven.app"
echo "  • Installer Disk DMG: src-tauri/target/release/bundle/dmg/Haven_${VERSION}_macOS.dmg"
echo ""
echo "Open the DMG or drag Haven.app to /Applications to use."
read -p "Press Enter to finish..."
