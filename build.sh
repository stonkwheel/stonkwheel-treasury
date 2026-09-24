#!/usr/bin/env bash
set -euo pipefail

# Point this at the extracted official Anza platform-tools v1.57 directory.
# This uses its compiler directly, without changing the user's global toolchain.
: "${STONK_PLATFORM_TOOLS:?Set STONK_PLATFORM_TOOLS to the extracted platform-tools v1.57 directory}"
cd -- "$(dirname -- "$0")"
export PATH="$STONK_PLATFORM_TOOLS/rust/bin:$STONK_PLATFORM_TOOLS/llvm/bin:$PATH"
# Stable paths make the binary reproducible across local checkout directories.
STONK_CARGO_ROOT="${CARGO_HOME:-$HOME/.cargo}"
export CARGO_ENCODED_RUSTFLAGS="--remap-path-prefix=$PWD=/stonkwheel"$'\x1f'"--remap-path-prefix=$STONK_CARGO_ROOT=/cargo"$'\x1f'"--remap-path-prefix=$STONK_PLATFORM_TOOLS=/platform-tools"
cargo build --locked --target sbpf-solana-solana --release
mkdir -p artifacts
cp "${CARGO_TARGET_DIR:-target}/sbpf-solana-solana/release/stonk_vault.so" artifacts/stonk_vault.so
python3 - <<'PY'
import hashlib, pathlib
p = pathlib.Path('artifacts/stonk_vault.so')
print('Built', p, 'bytes', p.stat().st_size, 'sha256', hashlib.sha256(p.read_bytes()).hexdigest())
PY
