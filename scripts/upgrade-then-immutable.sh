#!/usr/bin/env bash
# 1) Deploy/upgrade current target/deploy/arena_lock_v2.so
# 2) Optionally make immutable (CONFIRM_FINAL=YES)
#
# Gentle single-path deploy for rate-limited RPCs.
#
# Usage:
#   AUTHORITY_KEYPAIR=... RPC_URL=... [CONFIRM_FINAL=YES] scripts/upgrade-then-immutable.sh

set -euo pipefail

cd "$(dirname "$0")/.."

PROGRAM_ID="${PROGRAM_ID:-AV4FTAiteCN75iq6QbuPTuh2PVL4FKwyiWJiowhhzAsQ}"
AUTHORITY_KEYPAIR="${AUTHORITY_KEYPAIR:-${KEYPAIR:-/path/to/deploy-authority.json}}"
RPC_URL="${RPC_URL:-https://api.devnet.solana.com}"
SO_PATH="${SO_PATH:-target/deploy/arena_lock_v2.so}"

if [[ ! -f "$SO_PATH" ]]; then
  echo "Missing $SO_PATH — run: NO_DNA=1 cargo build-sbf --manifest-path programs/arena-lock-v2/Cargo.toml" >&2
  exit 1
fi
if [[ ! -f "$AUTHORITY_KEYPAIR" ]]; then
  echo "Missing authority keypair: $AUTHORITY_KEYPAIR" >&2
  exit 1
fi

echo "== upgrade =="
echo "program=$PROGRAM_ID"
echo "so=$SO_PATH ($(wc -c <"$SO_PATH") bytes)"
echo "rpc=$RPC_URL"
echo "authority=$(solana-keygen pubkey "$AUTHORITY_KEYPAIR")"

# Prefer pre-sized buffer matching on-chain program data when possible
MAX_LEN="${MAX_LEN:-242608}"

solana program deploy "$SO_PATH" \
  --program-id "$PROGRAM_ID" \
  --url "$RPC_URL" \
  --keypair "$AUTHORITY_KEYPAIR" \
  --max-len "$MAX_LEN" \
  --commitment confirmed \
  --with-compute-unit-price "${CU_PRICE:-2000}"

echo
echo "== post-upgrade show =="
solana program show "$PROGRAM_ID" --url "$RPC_URL" --keypair "$AUTHORITY_KEYPAIR"

if [[ "${CONFIRM_FINAL:-}" == "YES" ]]; then
  echo
  echo "== make immutable =="
  solana program set-upgrade-authority "$PROGRAM_ID" \
    --final \
    --url "$RPC_URL" \
    --keypair "$AUTHORITY_KEYPAIR"
  solana program show "$PROGRAM_ID" --url "$RPC_URL" --keypair "$AUTHORITY_KEYPAIR"
else
  echo
  echo "Upgrade done. To freeze forever:"
  echo "  CONFIRM_FINAL=YES AUTHORITY_KEYPAIR=$AUTHORITY_KEYPAIR RPC_URL=$RPC_URL scripts/make-program-immutable.sh"
fi
