# Arena Lock V2

Bullring arena staking contract for Solana.

This repository intentionally contains only the arena contract we have been working on:

- `programs/arena-lock-v2`

## Program

`arena-lock-v2` is a native Solana program for funded arena rewards:

- SPL Token and plain Token-2022 staking
- Per-user position PDA with pending activation and eligible stake accounting
- Configurable minimum lock time, activation delay, epoch length, minimum deposit, early-exit penalty, and burn share
- Explicitly funded rewards indexed immediately against mature eligible stake
- Epochs gate warming stake maturation; they do not hold a mutable reward batch
- Early exits split penalty between direct burn and redistribution when mature stake remains
- Full position exit settles pending rewards so users do not remain unstaked with dangling claimable rewards
- Final reward-pool surplus is burned from the actual pool balance only after all stake/reward debt is gone

Current devnet program id:

```text
At5K4wSgzNawzGGYMzMHNXUxtJ3yjU6gbgbj8MpSBMUz
```

## Build And Test

```bash
NO_DNA=1 cargo fmt --check
NO_DNA=1 cargo test --locked -p arena-lock-v2
NO_DNA=1 cargo clippy --locked -p arena-lock-v2 --all-targets --all-features -- -D warnings
NO_DNA=1 cargo build-sbf --manifest-path programs/arena-lock-v2/Cargo.toml -- --locked
```

For the full pre-audit gate:

```bash
scripts/readiness-check.sh
```

See `docs/AUDIT_PACKAGE.md` for the required external-review package, launch preconditions, and acceptance bar.

## Status

**Not mainnet-public-funds ready** (verified build passes; independent review,
governance, final mint/config, production pinning, and launch operations remain).

Adversarial Highs from the last pass are **fixed in source**:

| ID | Topic | Status |
|----|--------|--------|
| H-01 | Reward remainder re-index | Fixed |
| H-02 | JIT reward sniping (warming stake) | Fixed |
| H-03 | Split early-exit penalty bypass | Fixed |
| M-04 / M-05 | Dust / counter DoS | Fixed / mitigated |

See `docs/SECURITY_ADVERSARIAL_FINDINGS.md`. Prior “10/10 RC” claim remains **retracted**.

**Product note:** after `ActivatePosition`, stake is **warming** until the next
epoch roll plus a position touch; only then is it mature for funding/rewards.
Treasury funding should use `FundRewardsChecked` so the funder binds the transfer
to the expected mature denominator and epoch.
