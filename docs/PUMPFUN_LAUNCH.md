# Pump.fun → Arena → Site (self-launch order)

## Critical compatibility note

Pump.fun mints are **Token-2022** with **metadata** extensions.

- Old immutable program `AV4FTAiteCN75iq6QbuPTuh2PVL4FKwyiWJiowhhzAsQ` is not the final target for Pump.fun mainnet.
- New **pump-compatible v2** build allowlists only:
  - Mint: `MetadataPointer`, `TokenMetadata`
  - Account: `ImmutableOwner`
  - Still rejects transfer hooks, transfer fees, permanent delegate, etc.

**Devnet program id previously used for pump-compatible testing (verify before reuse; deploy a fresh v2 build for final testing):**

```text
Ac9fhZ2ZC19p7KEXtebhRweaqSEsuguSAXXnFNr1ML75
```

## Order of operations

```
1) Launch coin on Pump.fun  → get mint address
2) Deploy/confirm v2 program → fresh devnet/mainnet program id
3) Initialize arena config  → vault + reward pool for that mint
4) Pin site .env.production → live mode
5) Cloudflare deploy
6) Make program immutable   → CONFIRM_FINAL=YES (mainnet when ready)
```

## Mainnet deploy (needs SOL)

Fund the deploy authority with ~3 SOL on mainnet, then:

```bash
export KEYPAIR=/path/to/deploy-authority.json
export HELIUS_API_KEY=...
export RPC_URL="https://mainnet.helius-rpc.com/?api-key=$HELIUS_API_KEY"

cd arena-contract-sol
NO_DNA=1 cargo build-sbf --manifest-path programs/arena-lock-v2/Cargo.toml

# NEW program id for mainnet (do not reuse immutable AV4FTA)
solana program deploy target/deploy/arena_lock_v2.so \
  --program-id target/deploy/arena_lock_v2-pump-keypair.json \
  --url "$RPC_URL" \
  --keypair "$KEYPAIR"
```

After soak, freeze **the program id you actually deployed** (never a hardcoded
devnet default — freezing the wrong id leaves mainnet upgradeable):

```bash
# PROGRAM_ID must be the pubkey returned by `solana program deploy` on THIS cluster
CONFIRM_FINAL=YES \
PROGRAM_ID=<mainnet-or-target-program-id> \
AUTHORITY_KEYPAIR=$KEYPAIR \
RPC_URL=$RPC_URL \
  scripts/make-program-immutable.sh
```

**Stop before mainnet:** re-run adversarial review on this commit, prefer formal
audit + verified builds, then deploy under multisig/timelock. See
`docs/SECURITY_ADVERSARIAL_FINDINGS.md`. Engineering Highs H-01/H-02/H-03 are
fixed in source; that is not the same as firm-certified mainnet.

## After you have the pump mint

```bash
cd Sites/bullring
export PUBLIC_TOKEN_MINT=<pump mint>
export PUBLIC_ARENA_PROGRAM_ID=Ac9fhZ2ZC19p7KEXtebhRweaqSEsuguSAXXnFNr1ML75  # or mainnet id
export SOLANA_RPC_URL="https://mainnet.helius-rpc.com/?api-key=$HELIUS_API_KEY"
export PUBLIC_SOLANA_CLUSTER=mainnet-beta
export PUBLIC_ARENA_MIN_DEPOSIT_AMOUNT=1  # token units, converted to raw units by the script
export DEVNET_PAYER_KEYPAIR=...

bun scripts/post-pump-launch.mjs
# → writes production-pin.env
# merge into .env.production, then:
bun run deploy:cloudflare
```

## Website until then

Keep `PUBLIC_SITE_MODE=coming-soon` on thebullring.app (already production default).
