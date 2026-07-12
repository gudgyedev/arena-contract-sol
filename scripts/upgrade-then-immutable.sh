#!/usr/bin/env bash
# 1) Deploy/upgrade current target/deploy/arena_lock_v2.so
# 2) Optionally make immutable (CONFIRM_FINAL=YES)
#
# Gentle single-path deploy for rate-limited RPCs.
#
# Usage:
#   PROGRAM_ID=... AUTHORITY_KEYPAIR=... RPC_URL=... EXPECTED_GIT_COMMIT=... \
#     CONFIRM_UPGRADE=YES [CONFIRM_FINAL=YES] scripts/upgrade-then-immutable.sh

set -euo pipefail

cd "$(dirname "$0")/.."

: "${PROGRAM_ID:?Set PROGRAM_ID to the exact existing program being upgraded}"
: "${RPC_URL:?Set RPC_URL explicitly for the intended cluster}"
: "${EXPECTED_GIT_COMMIT:?Set EXPECTED_GIT_COMMIT to the reviewed release commit}"

AUTHORITY_KEYPAIR="${AUTHORITY_KEYPAIR:-${KEYPAIR:-/path/to/deploy-authority.json}}"
SO_PATH="${SO_PATH:-target/deploy/arena_lock_v2.so}"

if [[ ! -f "$SO_PATH" ]]; then
  echo "Missing $SO_PATH — run: NO_DNA=1 cargo build-sbf --manifest-path programs/arena-lock-v2/Cargo.toml" >&2
  exit 1
fi
if [[ ! -f "$AUTHORITY_KEYPAIR" ]]; then
  echo "Missing authority keypair: $AUTHORITY_KEYPAIR" >&2
  exit 1
fi
HEAD_COMMIT="$(git rev-parse HEAD)"
if [[ "$HEAD_COMMIT" != "$EXPECTED_GIT_COMMIT" ]]; then
  echo "HEAD $HEAD_COMMIT does not match EXPECTED_GIT_COMMIT $EXPECTED_GIT_COMMIT" >&2
  exit 1
fi
if ! git diff --quiet || ! git diff --cached --quiet; then
  echo "Refusing to deploy from a dirty worktree." >&2
  exit 1
fi

AUTHORITY_PUBKEY="$(solana-keygen pubkey "$AUTHORITY_KEYPAIR")"
PROGRAM_SHOW="$(solana program show "$PROGRAM_ID" --url "$RPC_URL" --keypair "$AUTHORITY_KEYPAIR")"
ONCHAIN_AUTHORITY="$(printf '%s\n' "$PROGRAM_SHOW" | awk -F': ' '/^Authority:/ {print $2}')"
if [[ "$ONCHAIN_AUTHORITY" != "$AUTHORITY_PUBKEY" ]]; then
  echo "Authority mismatch: on-chain=$ONCHAIN_AUTHORITY signer=$AUTHORITY_PUBKEY" >&2
  exit 1
fi
if [[ "${CONFIRM_UPGRADE:-}" != "YES" ]]; then
  echo "Preflight passed. Re-run with CONFIRM_UPGRADE=YES to deploy." >&2
  exit 2
fi

echo "== upgrade =="
echo "program=$PROGRAM_ID"
echo "so=$SO_PATH ($(wc -c <"$SO_PATH") bytes)"
echo "rpc=$RPC_URL"
echo "commit=$HEAD_COMMIT"
echo "sha256=$(sha256sum "$SO_PATH" | awk '{print $1}')"
echo "authority=$AUTHORITY_PUBKEY"

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

DEPLOYED_SO="$(mktemp)"
trap 'rm -f "$DEPLOYED_SO"' EXIT
solana program dump "$PROGRAM_ID" "$DEPLOYED_SO" --url "$RPC_URL"
LOCAL_BYTES="$(wc -c <"$SO_PATH")"
DEPLOYED_BYTES="$(wc -c <"$DEPLOYED_SO")"
if (( DEPLOYED_BYTES < LOCAL_BYTES )) || ! cmp -n "$LOCAL_BYTES" "$SO_PATH" "$DEPLOYED_SO"; then
  echo "Post-upgrade byte verification failed." >&2
  exit 1
fi
if (( DEPLOYED_BYTES > LOCAL_BYTES )) && ! tail -c "+$((LOCAL_BYTES + 1))" "$DEPLOYED_SO" | od -An -tu1 | awk '{ for (i=1; i<=NF; i++) if ($i != 0) exit 1 }'; then
  echo "Post-upgrade program has non-zero trailing bytes." >&2
  exit 1
fi
echo "Post-upgrade bytes match reviewed SBF."

if [[ "${CONFIRM_FINAL:-}" == "YES" ]]; then
  echo
  echo "== make immutable =="
  CONFIRM_FINAL=YES \
  PROGRAM_ID="$PROGRAM_ID" \
  AUTHORITY_KEYPAIR="$AUTHORITY_KEYPAIR" \
  RPC_URL="$RPC_URL" \
  SO_PATH="$SO_PATH" \
  EXPECTED_GIT_COMMIT="$EXPECTED_GIT_COMMIT" \
    scripts/make-program-immutable.sh
else
  echo
  echo "Upgrade done. To freeze forever:"
  echo "  CONFIRM_FINAL=YES AUTHORITY_KEYPAIR=$AUTHORITY_KEYPAIR RPC_URL=$RPC_URL scripts/make-program-immutable.sh"
fi
