#!/usr/bin/env bash
# Make arena-lock-v2 non-upgradeable (upgrade authority = None).
# IRREVERSIBLE. Only run after the FINAL binary is deployed.
#
# Usage:
#   AUTHORITY_KEYPAIR=... PROGRAM_ID=AV4FTAite... RPC_URL=https://api.devnet.solana.com \
#     scripts/make-program-immutable.sh
#
# Or with Helius:
#   RPC_URL="https://devnet.helius-rpc.com/?api-key=$HELIUS_API_KEY" scripts/make-program-immutable.sh

set -euo pipefail

PROGRAM_ID="${PROGRAM_ID:-AV4FTAiteCN75iq6QbuPTuh2PVL4FKwyiWJiowhhzAsQ}"
AUTHORITY_KEYPAIR="${AUTHORITY_KEYPAIR:-${KEYPAIR:-/path/to/deploy-authority.json}}"
RPC_URL="${RPC_URL:-https://api.devnet.solana.com}"

if [[ ! -f "$AUTHORITY_KEYPAIR" ]]; then
  echo "Missing authority keypair: $AUTHORITY_KEYPAIR" >&2
  exit 1
fi

echo "Program:   $PROGRAM_ID"
echo "RPC:       $RPC_URL"
echo "Authority: $(solana-keygen pubkey "$AUTHORITY_KEYPAIR")"
echo
solana program show "$PROGRAM_ID" --url "$RPC_URL" --keypair "$AUTHORITY_KEYPAIR"
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
solana program show "$PROGRAM_ID" --url "$RPC_URL" --keypair "$AUTHORITY_KEYPAIR"
echo "Immutable if Authority line shows none."
