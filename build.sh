#!/usr/bin/env bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

export PATH="/opt/homebrew/bin:$HOME/.cargo/bin:$PATH"

echo "========================================================"
echo "🔨 Building mac-sysmon (Full Stack Application)"
echo "========================================================"

# Step 1: Build TypeScript Frontend
echo ""
echo "▶ Phase 1: Compiling Vanilla TypeScript Frontend..."
if [ -d "frontend" ]; then
  if [ ! -f "dist/index.html" ] || [ ! -f "dist/bundle.js" ]; then
    echo "  Frontend assets missing from dist/, building from source..."
    cd frontend
    if [ ! -d "node_modules" ]; then
      echo "  Installing frontend devDependencies..."
      npm install --silent
    fi
    node build.js
    cd "$SCRIPT_DIR"
  elif [ -d "frontend/node_modules" ]; then
    cd frontend
    node build.js
    cd "$SCRIPT_DIR"
  else
    echo "  Pre-built frontend assets present in dist/ (skipping npm install)"
  fi
fi

# Verify dist directory
if [ ! -f "dist/index.html" ] || [ ! -f "dist/bundle.js" ]; then
  echo "❌ Error: Frontend distribution assets missing in dist/"
  exit 1
fi
echo "✓ Frontend assets compiled and staged into dist/"

# Step 2: Build Rust Backend
echo ""
echo "▶ Phase 2: Compiling Rust Backend..."
cargo build --release

echo ""
echo "========================================================"
echo "✅ Build Successful!"
echo "Binary executable: $SCRIPT_DIR/target/release/mac-sysmon"
echo "Run with: ./run.sh --open"
echo "========================================================"
