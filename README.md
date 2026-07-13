# Bullring Arena Program

[![arena-lock-v2 CI](https://github.com/gudgyedev/arena-contract-sol/actions/workflows/arena-lock-v2-ci.yml/badge.svg)](https://github.com/gudgyedev/arena-contract-sol/actions/workflows/arena-lock-v2-ci.yml)

The Bullring Arena is a native Solana token-locking and funded-reward program.
Users lock tokens, activate their position, mature into the eligible stake base,
and receive a pro-rata share of rewards that are explicitly added to the arena.

There is no promised APY and no reward minting. Rewards come from treasury or
creator funding and the redistributable share of early-exit penalties.

> **Release status:** the mainnet program is deployed and byte-verified. The
> official token/config binding will be created after the final token mint is
> verified. The program remains upgradeable during the controlled launch phase.
> This software is unaudited and provided without warranty.

## Mainnet Release

| Field | Value |
|---|---|
| Cluster | `mainnet-beta` |
| Program | [`DACFfLpaVw2Q7dz4mEUVBFzR7VjjTxMJT71AFBDLJmwU`](https://solscan.io/account/DACFfLpaVw2Q7dz4mEUVBFzR7VjjTxMJT71AFBDLJmwU) |
| Deployed source | [`55eb89f2757962bf5b91f619676dbf46c811069e`](https://github.com/gudgyedev/arena-contract-sol/commit/55eb89f2757962bf5b91f619676dbf46c811069e) |
| Release | [`mainnet-program-55eb89f`](https://github.com/gudgyedev/arena-contract-sol/releases/tag/mainnet-program-55eb89f) |
| Artifact SHA-256 | `67718f595b6558f4f598132911afc839c9f4023e43d3c21d2f9cd75103ae0629` |
| Deployment slot | `432549714` |
| Deployment transaction | [`5bNt9w...6Z5q3`](https://solscan.io/tx/5bNt9wimPwMfmQzbkviZSqc7rNvsUPFkJNT5RXqGnmFdhcEKf6z4CK6TJ9B4NBKYud29zxH8JEGddX7apPb6Z5q3) |

The executable was independently dumped after deployment. Its 181,824 bytes
and SHA-256 hash exactly matched the reviewed release artifact. The full record
is in [`docs/MAINNET_DEPLOYMENT.md`](docs/MAINNET_DEPLOYMENT.md).

## Official Launch Policy

The official arena config will be initialized with these reviewed values:

| Rule | Value |
|---|---:|
| Minimum lock | 7 days |
| Activation delay | 2 seconds |
| Epoch length | 3 days |
| Minimum deposit | 1 token |
| Early-exit penalty | 10% |
| Direct burn on early exit | 1% of the withdrawn amount |
| Redistributable early-exit share | 9% when mature eligible stake remains |

If an early exit leaves no mature eligible stake, the full penalty is burned
instead of becoming an unclaimable reward. Withdrawals after the minimum lock
are penalty-free.

## Position Lifecycle

1. **Deposit** — tokens move into the program-controlled principal vault and
   are recorded to the user's position PDA.
2. **Activate** — after the activation delay, deposited stake enters warming.
3. **Mature** — after the next epoch roll and a position sync, warming stake
   joins the eligible stake base.
4. **Earn** — funded rewards are indexed pro rata across mature eligible stake.
5. **Claim or withdraw** — rewards can be claimed while staked. An early
   withdrawal applies the configured penalty; a mature withdrawal does not.

Epochs control stake admission. They are not mutable reward batches, and users
cannot gain a larger reward share by repeatedly claiming.

## Reward Funding

Anyone may fund rewards from a compatible token account. Production funding
should use `FundRewardsChecked`, which binds the transfer to the expected mature
eligible denominator and current epoch. This prevents a stale client from
funding against an unexpected stake snapshot.

Rewards are denominated in the same token as the arena config. The program does
not swap assets, collect creator fees, or mint rewards; those operations happen
outside the program before a funder submits tokens through the funding
instruction.

## Custody and Trust Model

- Each arena config binds one mint, token program, principal vault, reward pool,
  authority, and immutable economic parameters.
- User accounting lives in program-derived position accounts.
- Principal and rewards are held in separate token accounts controlled by the
  program's vault-authority PDA.
- The public instruction surface has no administrator withdrawal or rescue
  instruction.
- Tokens sent directly to custody addresses do not execute program accounting
  and are not credited to a user position. Always use the program instructions.
- The program currently has an upgrade authority. Until that authority is
  removed, a valid program upgrade can change future behavior.

The program supports standard SPL Token mints and a fail-closed subset of
Token-2022. Unsupported or dangerous Token-2022 extensions are rejected during
config initialization.

## Instructions

The public instruction interface is defined in
[`programs/arena-lock-v2/src/instruction.rs`](programs/arena-lock-v2/src/instruction.rs):

- `InitializeConfig`
- `Deposit`
- `ActivatePosition`
- `RollEpoch`
- `FundRewardsChecked`
- `ClaimRewards`
- `Withdraw`
- `FinalizeRewards`

`FundRewards` remains available at the program layer, but checked funding is
the recommended production path.

## Build and Verify

Requirements: Rust `1.95.0` and the Solana SBF toolchain.

```bash
NO_DNA=1 cargo fmt --check
NO_DNA=1 cargo test --locked -p arena-lock-v2 -- --test-threads=1
NO_DNA=1 cargo clippy --locked -p arena-lock-v2 --all-targets --all-features -- -D warnings
NO_DNA=1 cargo build-sbf --manifest-path programs/arena-lock-v2/Cargo.toml -- --locked
```

Run the complete release gate with:

```bash
NO_DNA=1 scripts/readiness-check.sh
```

The test suite contains 6 native unit tests and 24 ProgramTest integration
tests covering multi-user accounting, reward conservation, warming and epoch
boundaries, early exits, burns, custody validation, and token compatibility.

## Repository Guide

- [`programs/arena-lock-v2`](programs/arena-lock-v2) — program source and tests
- [`docs/MAINNET_DEPLOYMENT.md`](docs/MAINNET_DEPLOYMENT.md) — deployment and byte-verification record
- [`docs/MAINNET_LAUNCH_READINESS.md`](docs/MAINNET_LAUNCH_READINESS.md) — launch gates and engineering evidence
- [`docs/SELF_LAUNCH_SIGNOFF.md`](docs/SELF_LAUNCH_SIGNOFF.md) — unaudited self-launch decision and final sequence
- [`docs/SECURITY_ADVERSARIAL_FINDINGS.md`](docs/SECURITY_ADVERSARIAL_FINDINGS.md) — detailed historical security review
- [`scripts`](scripts) — guarded deployment and immutability tooling

## Safety Notice

This repository does not claim an independent audit, certification, warranty,
or guarantee that funds cannot be lost. Testing and byte verification reduce
risk but cannot prove the absence of every smart-contract, economic,
operational, key-custody, or integration failure. Users interact at their own
risk.
