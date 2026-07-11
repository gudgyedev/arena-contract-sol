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
NO_DNA=1 cargo fmt --check
NO_DNA=1 cargo test -p arena-lock-v2
NO_DNA=1 cargo clippy -p arena-lock-v2 --all-targets --all-features -- -D warnings
NO_DNA=1 cargo build-sbf --manifest-path programs/arena-lock-v2/Cargo.toml
```

For the full pre-audit gate:

```bash
scripts/readiness-check.sh
```

See `docs/AUDIT_PACKAGE.md` for the required external-review package, launch preconditions, and acceptance bar.

## Status

**Not mainnet-public-funds ready** (needs re-review / audit / verified builds).

Adversarial Highs from the last pass are **fixed in source**:

| ID | Topic | Status |
|----|--------|--------|
| H-01 | Reward remainder re-index | Fixed |
| H-02 | JIT reward sniping (warming stake) | Fixed |
| H-03 | Split early-exit penalty bypass | Fixed |
| M-04 / M-05 | Dust / counter DoS | Mitigated / fixed |

See `docs/SECURITY_ADVERSARIAL_FINDINGS.md`. Prior “10/10 RC” claim remains **retracted**.

**Product note:** after `ActivatePosition`, stake is **warming** until the next epoch
roll + a position touch; only then is it mature for funding/rewards.
