#!/bin/bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")" && pwd)"; cd "$ROOT"
for t in cargo node npm python3; do command -v "$t" >/dev/null || { echo "FAIL: $t missing"; exit 1; }; done
python3 scripts/verify_source.py
cargo test --workspace --lib
cargo build --release -p language-server-1o
ONEO_VERIFY_SERVER_PATH="$ROOT/target/release/language-server-1o" node scripts/verify_1o_installed.mjs "$ROOT"
npm ci --no-audit --no-fund
npm run compile
python3 scripts/stage_crates.py
cargo package --manifest-path release-stage/Cargo.toml -p core-1o --allow-dirty
echo '1o LOCAL RELEASE GATE: PASS'
