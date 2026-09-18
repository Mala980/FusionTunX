#!/bin/bash
set -e

echo "=== Building FusionTunX (Rust) ==="

cd "$(dirname "$0")"

echo "[1/3] Building frontend..."
cd dash
if [ -f package-lock.json ]; then
    npm install
fi
npm run build
cd ..

echo "[2/3] Building Rust binary (Multi-Arch)..."
mkdir -p bin

# Determine host architecture
HOST_ARCH=$(uname -m)
case "$HOST_ARCH" in
    x86_64) HOST_TARGET="x86_64-unknown-linux-gnu" ;;
    aarch64|arm64) HOST_TARGET="aarch64-unknown-linux-gnu" ;;
    armv7*|armhf) HOST_TARGET="armv7-unknown-linux-gnueabihf" ;;
    *) HOST_TARGET="" ;;
esac

# Function to build for a target
build_target() {
    local target="$1"
    local output="$2"
    echo " -> Building for $target..."
    if cargo build --release --target "$target"; then
        if [ -f "target/$target/release/fusiontunx" ]; then
            cp "target/$target/release/fusiontunx" "$output"
            echo "    Created $output"
            return 0
        elif [ -f "../target/$target/release/fusiontunx" ]; then
            cp "../target/$target/release/fusiontunx" "$output"
            echo "    Created $output"
            return 0
        fi
    fi
    echo "    Warning: Target $target failed to build"
    return 1
}

# 1. Build amd64
if rustup target list 2>/dev/null | grep -q "x86_64-unknown-linux-gnu (installed)"; then
    build_target "x86_64-unknown-linux-gnu" "bin/fusiontunx-linux-amd64" || true
fi

# 2. Build arm64
if rustup target list 2>/dev/null | grep -q "aarch64-unknown-linux-gnu (installed)"; then
    build_target "aarch64-unknown-linux-gnu" "bin/fusiontunx-linux-arm64" || true
fi

# 3. Build armv7
if rustup target list 2>/dev/null | grep -q "armv7-unknown-linux-gnueabihf (installed)"; then
    build_target "armv7-unknown-linux-gnueabihf" "bin/fusiontunx-linux-armv7" || true
fi

# Default host build
echo " -> Building for host default..."
cargo build --release
if [ -f "target/release/fusiontunx" ]; then
    cp "target/release/fusiontunx" "bin/fusiontunx"
elif [ -f "../target/release/fusiontunx" ]; then
    cp "../target/release/fusiontunx" "bin/fusiontunx"
fi

if [ "$HOST_ARCH" == "x86_64" ] && [ ! -f "bin/fusiontunx-linux-amd64" ]; then
    cp "bin/fusiontunx" "bin/fusiontunx-linux-amd64"
elif { [ "$HOST_ARCH" == "aarch64" ] || [ "$HOST_ARCH" == "arm64" ]; } && [ ! -f "bin/fusiontunx-linux-arm64" ]; then
    cp "bin/fusiontunx" "bin/fusiontunx-linux-arm64"
fi

echo "[3/3] Build complete!"
echo ""
echo "Binaries available in bin/:"
ls -lh bin/
