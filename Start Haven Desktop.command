#!/bin/bash
cd "$(dirname "$0")"

echo "Starting Haven Desktop for macOS..."

if ! command -v node >/dev/null 2>&1; then
  echo "[ERROR] Node.js not found. Please install Node.js 18+ from https://nodejs.org or via Homebrew."
  read -p "Press Enter to exit..."
  exit 1
fi

if ! command -v rustc >/dev/null 2>&1; then
  echo "[ERROR] Rust compiler (rustc) not found. Install from https://rustup.rs then reopen."
  read -p "Press Enter to exit..."
  exit 1
fi

if [ ! -d "node_modules" ]; then
  echo "Installing dependencies first..."
  npm install || { echo "[ERROR] npm install failed"; read -p "Press Enter to exit..."; exit 1; }
fi

npm run tauri:dev
