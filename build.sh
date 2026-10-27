#!/usr/bin/env bash
set -euo pipefail

echo "Building mobile-linux-shell.wasm …"
wasm-pack build --target web --out-dir pkg --release

# Optional: copy to a static server directory
mkdir -p dist
cp -r pkg/* dist/
cp www/* dist/

echo "Done. Serve the 'dist' directory (or 'pkg' + 'www') over HTTPS."
echo "  npx serve dist   # or any static file server"
