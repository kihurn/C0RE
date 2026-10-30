#!/bin/bash
# Builds and tests everything, then packages:
#   dist/c0re-<version>-windows-x64.zip   c0re.exe, c0re-svc.exe, README, docs
#   dist/c0re-<version>-source.zip        the source at HEAD
#   dist/c0re-<version>.bundle            full git history (git clone c0re-<version>.bundle c0re)
set -euo pipefail
cd "$(dirname "$0")/.."
VER=$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)

echo "== dashboard"
(cd web && npm ci --silent && npm test --silent && npm run check --silent && npm run build --silent)

echo "== tests"
cargo test -q

echo "== windows build"
cargo build --release --target x86_64-pc-windows-gnu

echo "== package"
rm -rf dist/stage && mkdir -p dist/stage/C0RE/docs
cp target/x86_64-pc-windows-gnu/release/c0re.exe target/x86_64-pc-windows-gnu/release/c0re-svc.exe dist/stage/C0RE/
cp README.md dist/stage/C0RE/
cp docs/setup-windows.md docs/manual-test.md dist/stage/C0RE/docs/
rm -f "dist/c0re-$VER-windows-x64.zip"
(cd dist/stage && python3 -m zipfile -c "../c0re-$VER-windows-x64.zip" C0RE)
rm -rf dist/stage
git archive --format=zip --prefix="c0re/" -o "dist/c0re-$VER-source.zip" HEAD
git bundle create "dist/c0re-$VER.bundle" --all
ls -la dist/
