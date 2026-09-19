#!/usr/bin/env bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

export PATH="/opt/homebrew/bin:$HOME/.cargo/bin:$PATH"

# If dist directory is empty or missing, compile the frontend
if [ ! -f "$SCRIPT_DIR/dist/index.html" ] || [ ! -f "$SCRIPT_DIR/dist/bundle.js" ]; then
  echo "Frontend bundle not found in dist/. Compiling Vanilla TypeScript frontend..."
  cd "$SCRIPT_DIR/frontend"
  if [ ! -d "node_modules" ]; then
    npm install --silent
  fi
  node build.js
  cd "$SCRIPT_DIR"
fi

# If release binary exists, execute it directly; otherwise compile and run via cargo
if [ -f "$SCRIPT_DIR/target/release/mac-sysmon" ]; then
  exec "$SCRIPT_DIR/target/release/mac-sysmon" "$@"
else
  echo "Release binary not found. Building and running via Cargo..."
  exec cargo run --release -- "$@"
fi
