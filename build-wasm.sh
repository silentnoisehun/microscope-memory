#!/bin/bash

echo "Building Microscope Memory for WebAssembly..."

# Install wasm-pack if not present
if ! command -v wasm-pack &> /dev/null; then
    echo "Installing wasm-pack..."
    curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh
fi

# Add wasm32 target
rustup target add wasm32-unknown-unknown

# Build WASM module
wasm-pack build --target web --out-dir pkg --release

echo "WASM build complete! Check the pkg/ directory"
echo ""
echo "To use in browser:"
echo "1. Serve the pkg/ directory with a web server"
echo "2. Import in JavaScript:"
echo "   import init, { MicroscopeWasm } from './pkg/microscope_memory.js';"
echo "   await init();"
echo "   const microscope = new MicroscopeWasm();"