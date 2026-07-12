# Pump.fun → Arena → Site (self-launch order)

## Critical compatibility note

Pump.fun mints are **Token-2022** with **metadata** extensions.

- Old immutable program `AV4FTAiteCN75iq6QbuPTuh2PVL4FKwyiWJiowhhzAsQ` is not the final target for Pump.fun mainnet.
- New **pump-compatible v2** build allowlists only:
  - Mint: `MetadataPointer`, `TokenMetadata`
  - Account: `ImmutableOwner`
  - Still rejects transfer hooks, transfer fees, permanent delegate, etc.

**Current upgradeable devnet test program (never reuse as the mainnet id):**

```text
At5K4wSgzNawzGGYMzMHNXUxtJ3yjU6gbgbj8MpSBMUz
```

## Order of operations

```
1) Launch coin on Pump.fun  → get mint address
2) Deploy/confirm v2 program → fresh devnet/mainnet program id
3) Initialize arena config  → vault + reward pool for that mint
4) Pin site .env.production → live mode
5) Cloudflare deploy
6) Run a low-value smoke and verify exact production bindings
7) Make immutable, or document a temporary upgrade authority and removal date
```

## Mainnet deploy (needs SOL)

Fund the deploy authority with ~3 SOL on mainnet, then:

```bash
export KEYPAIR=/path/to/deploy-authority.json
export HELIUS_API_KEY=...
export RPC_URL="https://mainnet.helius-rpc.com/?api-key=$HELIUS_API_KEY"

cd arena-contract-sol
solana-verify build --library-name arena_lock_v2

# Generate a fresh ignored mainnet program keypair once. Back it up securely;
# publish only its public address.
mkdir -p target/mainnet
solana-keygen new --no-bip39-passphrase --silent \
  --outfile target/mainnet/arena-lock-v2-program-keypair.json

# Inspection-only first. This refuses a dirty/wrong commit, non-mainnet RPC,
# reused program address, missing artifact, or mismatched key files.
RPC_URL="$RPC_URL" \
AUTHORITY_KEYPAIR="$KEYPAIR" \
PROGRAM_KEYPAIR=target/mainnet/arena-lock-v2-program-keypair.json \
EXPECTED_GIT_COMMIT=$(git rev-parse HEAD) \
  scripts/deploy-mainnet-program.sh

# Only after reviewing the exact printed summary and explicit approval:
CONFIRM_MAINNET_DEPLOY=YES \
RPC_URL="$RPC_URL" \
AUTHORITY_KEYPAIR="$KEYPAIR" \
PROGRAM_KEYPAIR=target/mainnet/arena-lock-v2-program-keypair.json \
EXPECTED_GIT_COMMIT=$(git rev-parse HEAD) \
  scripts/deploy-mainnet-program.sh
```

After exact-byte verification and a successful production smoke, optionally
freeze **the program id you actually deployed**. This is irreversible; never
use a hardcoded devnet default:

```bash
# PROGRAM_ID must be the pubkey returned by `solana program deploy` on THIS cluster
CONFIRM_FINAL=YES \
PROGRAM_ID=<mainnet-or-target-program-id> \
AUTHORITY_KEYPAIR=$KEYPAIR \
RPC_URL=$RPC_URL \
EXPECTED_GIT_COMMIT=$(git rev-parse HEAD) \
  scripts/make-program-immutable.sh
```

**Unaudited self-launch:** the owner explicitly waived an external audit. Re-run
the automated/adversarial gate on the exact release commit and verify deployed
bytes. See `docs/SECURITY_ADVERSARIAL_FINDINGS.md`. Engineering Highs
H-01/H-02/H-03 are fixed in source; that is not firm certification or a safety
guarantee.

## After you have the pump mint

```bash
cd Sites/bullring
export PUBLIC_TOKEN_MINT=<pump mint>
export PUBLIC_ARENA_PROGRAM_ID=<fresh-mainnet-program-id>
export SOLANA_RPC_URL="https://mainnet.helius-rpc.com/?api-key=$HELIUS_API_KEY"
export PUBLIC_SOLANA_CLUSTER=mainnet-beta
export PUBLIC_ARENA_MIN_DEPOSIT_AMOUNT=1  # token units, converted to raw units by the script
export ARENA_AUTHORITY_KEYPAIR=...

bun scripts/post-pump-launch.mjs # inspection only
CONFIRM_MAINNET_INITIALIZE=YES bun scripts/post-pump-launch.mjs
# → after confirmation, writes production-pin.env
# merge into .env.production, then:
bun run deploy:cloudflare
```

## Website until then

Keep `PUBLIC_SITE_MODE=coming-soon` on thebullring.app (already production default).
