#!/usr/bin/env bash
# Guarded fresh mainnet deployment. Dry-run summary unless CONFIRM_MAINNET_DEPLOY=YES.

set -euo pipefail

cd "$(dirname "$0")/.."

: "${RPC_URL:?Set the explicit mainnet RPC URL}"
: "${AUTHORITY_KEYPAIR:?Set the deploy/upgrade-authority keypair path}"
: "${PROGRAM_KEYPAIR:?Set a fresh mainnet program keypair path}"
: "${EXPECTED_GIT_COMMIT:?Set the reviewed release commit}"

SOLANA_BIN="${SOLANA_BIN:-solana}"
SOLANA_KEYGEN_BIN="${SOLANA_KEYGEN_BIN:-solana-keygen}"
SO_PATH="${SO_PATH:-target/deploy/arena_lock_v2.so}"
MAINNET_GENESIS_HASH="5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d"

for path in "$AUTHORITY_KEYPAIR" "$PROGRAM_KEYPAIR" "$SO_PATH"; do
  [[ -f "$path" ]] || { echo "Missing required file: $path" >&2; exit 1; }
done

HEAD_COMMIT="$(git rev-parse HEAD)"
[[ "$HEAD_COMMIT" == "$EXPECTED_GIT_COMMIT" ]] || {
  echo "HEAD $HEAD_COMMIT does not match EXPECTED_GIT_COMMIT $EXPECTED_GIT_COMMIT" >&2
  exit 1
}
[[ -z "$(git status --porcelain)" ]] || {
  echo "Refusing to deploy from a dirty or untracked worktree." >&2
  exit 1
}

GENESIS_HASH="$($SOLANA_BIN genesis-hash --url "$RPC_URL")"
[[ "$GENESIS_HASH" == "$MAINNET_GENESIS_HASH" ]] || {
  echo "RPC is not Solana mainnet-beta: genesis=$GENESIS_HASH" >&2
  exit 1
}

AUTHORITY="$($SOLANA_KEYGEN_BIN pubkey "$AUTHORITY_KEYPAIR")"
PROGRAM_ID="$($SOLANA_KEYGEN_BIN pubkey "$PROGRAM_KEYPAIR")"
BALANCE="$($SOLANA_BIN balance "$AUTHORITY" --url "$RPC_URL")"
ARTIFACT_BYTES="$(wc -c <"$SO_PATH" | tr -d ' ')"
ARTIFACT_HASH="$(sha256sum "$SO_PATH" | awk '{print $1}')"
PROGRAMDATA_RENT="$($SOLANA_BIN rent "$((ARTIFACT_BYTES + 45))" --url "$RPC_URL")"
BUFFER_RENT="$($SOLANA_BIN rent "$((ARTIFACT_BYTES + 37))" --url "$RPC_URL")"
PROGRAM_ACCOUNT_RENT="$($SOLANA_BIN rent 36 --url "$RPC_URL")"

if $SOLANA_BIN program show "$PROGRAM_ID" --url "$RPC_URL" >/dev/null 2>&1; then
  echo "Program address already exists on mainnet: $PROGRAM_ID" >&2
  exit 1
fi

echo "=== Bullring fresh mainnet program deploy ==="
echo "cluster-genesis=$GENESIS_HASH"
echo "rpc=$RPC_URL"
echo "commit=$HEAD_COMMIT"
echo "artifact=$SO_PATH"
echo "artifact-bytes=$ARTIFACT_BYTES"
echo "artifact-sha256=$ARTIFACT_HASH"
echo "authority=$AUTHORITY"
echo "authority-balance=$BALANCE"
echo "fresh-program-id=$PROGRAM_ID"
echo "programdata-$PROGRAMDATA_RENT"
echo "temporary-buffer-$BUFFER_RENT (normally reclaimed after successful deploy)"
echo "program-account-$PROGRAM_ACCOUNT_RENT"
echo "transaction fees are additional"
echo "upgradeable=yes (immutability is a separate explicit transaction)"
echo

if [[ "${CONFIRM_MAINNET_DEPLOY:-}" != "YES" ]]; then
  echo "Inspection only. Re-run with CONFIRM_MAINNET_DEPLOY=YES to send." >&2
  exit 2
fi

$SOLANA_BIN program deploy "$SO_PATH" \
  --program-id "$PROGRAM_KEYPAIR" \
  --upgrade-authority "$AUTHORITY_KEYPAIR" \
  --keypair "$AUTHORITY_KEYPAIR" \
  --url "$RPC_URL" \
  --commitment finalized \
  --output json

$SOLANA_BIN program show "$PROGRAM_ID" --url "$RPC_URL"
DEPLOYED_SO="$(mktemp)"
trap 'rm -f "$DEPLOYED_SO"' EXIT
$SOLANA_BIN program dump "$PROGRAM_ID" "$DEPLOYED_SO" --url "$RPC_URL"
DEPLOYED_BYTES="$(wc -c <"$DEPLOYED_SO" | tr -d ' ')"
if (( DEPLOYED_BYTES < ARTIFACT_BYTES )) || ! cmp -n "$ARTIFACT_BYTES" "$SO_PATH" "$DEPLOYED_SO"; then
  echo "Post-deploy byte verification failed." >&2
  exit 1
fi
if (( DEPLOYED_BYTES > ARTIFACT_BYTES )) && ! tail -c "+$((ARTIFACT_BYTES + 1))" "$DEPLOYED_SO" | od -An -tu1 | awk '{ for (i=1; i<=NF; i++) if ($i != 0) exit 1 }'; then
  echo "Post-deploy bytes contain non-zero trailing data." >&2
  exit 1
fi
echo "Post-deploy byte verification passed."
