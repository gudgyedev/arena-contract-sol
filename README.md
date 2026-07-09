# Arena Lock V2

Bullring arena staking contract for Solana.

This repository intentionally contains only the arena contract we have been working on:

- `programs/arena-lock-v2`

## Program

`arena-lock-v2` is a native Solana program for funded arena rewards:

- SPL Token and plain Token-2022 staking
- Per-user position PDA with pending activation and eligible stake accounting
- Configurable minimum lock time, activation delay, epoch length, minimum deposit, early-exit penalty, and burn share
- Explicitly funded reward pools, rolled by epoch and claimed by users
- Early exits can split penalty between reward pool and burn
- Full position exit settles pending rewards so users do not remain unstaked with dangling claimable rewards

Current devnet program id:

```text
AV4FTAiteCN75iq6QbuPTuh2PVL4FKwyiWJiowhhzAsQ
```

## Build And Test

```bash
NO_DNA=1 cargo test -p arena-lock-v2
NO_DNA=1 cargo build-sbf --manifest-path programs/arena-lock-v2/Cargo.toml
```

## Status

This is devnet-tested contract code for Bullring. It is not mainnet-public-funds ready without production authority controls, independent review, and an external audit.
