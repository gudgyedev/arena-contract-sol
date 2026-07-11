#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

NO_DNA=1 cargo fmt --check
NO_DNA=1 cargo test -p arena-lock-v2
NO_DNA=1 cargo clippy -p arena-lock-v2 --all-targets --all-features -- -D warnings
NO_DNA=1 cargo build-sbf --manifest-path programs/arena-lock-v2/Cargo.toml
NO_DNA=1 cargo deny --workspace check --exclude-dev --config deny.toml advisories sources
NO_DNA=1 cargo audit \
  --ignore RUSTSEC-2024-0344 \
  --ignore RUSTSEC-2022-0093 \
  --ignore RUSTSEC-2026-0098 \
  --ignore RUSTSEC-2026-0099 \
  --ignore RUSTSEC-2026-0104

if rg -n "\bunsafe\b|unchecked_|unwrap_unchecked|from_raw|transmute|set_len|MaybeUninit" programs/arena-lock-v2/src programs/arena-lock-v2/tests; then
  echo "unsafe or raw-memory pattern found in arena-lock-v2 source" >&2
  exit 1
fi
