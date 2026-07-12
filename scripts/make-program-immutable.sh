#!/usr/bin/env bash
# Make arena-lock-v2 non-upgradeable (upgrade authority = None).
# IRREVERSIBLE. Only run after the FINAL binary is deployed.
#
# Usage:
#   AUTHORITY_KEYPAIR=... PROGRAM_ID=... RPC_URL=... \
#     EXPECTED_GIT_COMMIT=... CONFIRM_FINAL=YES \
#     scripts/make-program-immutable.sh
#
# Or with Helius:
#   RPC_URL="https://devnet.helius-rpc.com/?api-key=$HELIUS_API_KEY" scripts/make-program-immutable.sh

set -euo pipefail

cd "$(dirname "$0")/.."

: "${PROGRAM_ID:?Set PROGRAM_ID to the exact program being finalized}"
: "${RPC_URL:?Set RPC_URL explicitly for the intended cluster}"
: "${EXPECTED_GIT_COMMIT:?Set EXPECTED_GIT_COMMIT to the reviewed release commit}"

AUTHORITY_KEYPAIR="${AUTHORITY_KEYPAIR:-${KEYPAIR:-/path/to/deploy-authority.json}}"
SO_PATH="${SO_PATH:-target/deploy/arena_lock_v2.so}"

if [[ ! -f "$AUTHORITY_KEYPAIR" ]]; then
  echo "Missing authority keypair: $AUTHORITY_KEYPAIR" >&2
  exit 1
fi
if [[ ! -f "$SO_PATH" ]]; then
  echo "Missing reviewed binary: $SO_PATH" >&2
  exit 1
fi

HEAD_COMMIT="$(git rev-parse HEAD)"
if [[ "$HEAD_COMMIT" != "$EXPECTED_GIT_COMMIT" ]]; then
  echo "HEAD $HEAD_COMMIT does not match EXPECTED_GIT_COMMIT $EXPECTED_GIT_COMMIT" >&2
  exit 1
fi
if ! git diff --quiet || ! git diff --cached --quiet; then
  echo "Refusing to finalize from a dirty worktree." >&2
  exit 1
fi

AUTHORITY_PUBKEY="$(solana-keygen pubkey "$AUTHORITY_KEYPAIR")"
PROGRAM_SHOW="$(solana program show "$PROGRAM_ID" --url "$RPC_URL" --keypair "$AUTHORITY_KEYPAIR")"
ONCHAIN_AUTHORITY="$(printf '%s\n' "$PROGRAM_SHOW" | awk -F': ' '/^Authority:/ {print $2}')"
if [[ "$ONCHAIN_AUTHORITY" != "$AUTHORITY_PUBKEY" ]]; then
  echo "Authority mismatch: on-chain=$ONCHAIN_AUTHORITY signer=$AUTHORITY_PUBKEY" >&2
  exit 1
fi

DEPLOYED_SO="$(mktemp)"
trap 'rm -f "$DEPLOYED_SO"' EXIT
solana program dump "$PROGRAM_ID" "$DEPLOYED_SO" --url "$RPC_URL"
LOCAL_BYTES="$(wc -c <"$SO_PATH")"
DEPLOYED_BYTES="$(wc -c <"$DEPLOYED_SO")"
if (( DEPLOYED_BYTES < LOCAL_BYTES )) || ! cmp -n "$LOCAL_BYTES" "$SO_PATH" "$DEPLOYED_SO"; then
  echo "On-chain bytes do not begin with the reviewed local SBF." >&2
  exit 1
fi
if (( DEPLOYED_BYTES > LOCAL_BYTES )) && tail -c "+$((LOCAL_BYTES + 1))" "$DEPLOYED_SO" | od -An -tu1 | awk '{ for (i=1; i<=NF; i++) if ($i != 0) exit 1 }'; then
  :
elif (( DEPLOYED_BYTES > LOCAL_BYTES )); then
  echo "On-chain program has non-zero trailing bytes after the reviewed SBF." >&2
  exit 1
fi

echo "Program:   $PROGRAM_ID"
echo "RPC:       $RPC_URL"
echo "Commit:    $HEAD_COMMIT"
echo "SBF hash:  $(sha256sum "$SO_PATH" | awk '{print $1}')"
echo "Authority: $AUTHORITY_PUBKEY"
echo
printf '%s\n' "$PROGRAM_SHOW"
echo
echo "This sets upgrade authority to NONE. Cannot be undone."
if [[ "${CONFIRM_FINAL:-}" != "YES" ]]; then
  echo "Re-run with CONFIRM_FINAL=YES to execute." >&2
  exit 2
fi

solana program set-upgrade-authority "$PROGRAM_ID" \
  --final \
  --url "$RPC_URL" \
  --keypair "$AUTHORITY_KEYPAIR"

echo
echo "Result:"
FINAL_SHOW="$(solana program show "$PROGRAM_ID" --url "$RPC_URL" --keypair "$AUTHORITY_KEYPAIR")"
printf '%s\n' "$FINAL_SHOW"
FINAL_AUTHORITY="$(printf '%s\n' "$FINAL_SHOW" | awk -F': ' '/^Authority:/ {print $2}')"
if [[ -n "$FINAL_AUTHORITY" && "$FINAL_AUTHORITY" != "none" && "$FINAL_AUTHORITY" != "None" ]]; then
  echo "Finalization did not remove upgrade authority: $FINAL_AUTHORITY" >&2
  exit 1
fi
echo "Program is immutable."
