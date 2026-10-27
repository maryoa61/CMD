#!/usr/bin/env bash
set -euo pipefail

echo "→ Building WASM..."
wasm-pack build --target web --out-dir pkg --release

echo "→ Preparing dist/..."
rm -rf dist
mkdir -p dist
cp -r pkg/* dist/
cp -r www/* dist/

echo ""
echo "✓ Build successful!"
echo "  Serve with:  npx serve dist"
echo "  or:          python3 -m http.server 8080 --directory dist"
