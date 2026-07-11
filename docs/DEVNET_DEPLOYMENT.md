# Arena Lock V2 Devnet Deployment

Review date: 2026-07-11

This deployment is for Bullring UI/devnet testing only. It is intentionally
upgradeable while testing continues.

## Program

- Cluster: devnet
- Program id: `At5K4wSgzNawzGGYMzMHNXUxtJ3yjU6gbgbj8MpSBMUz`
- ProgramData: `D2ZVopHud1bz6GGERZf6yuknuYffxkQqnHWJzPLRgqFG`
- Upgrade authority: `HxfS8TXM9bprXe1jD2RgcAU1c8nd9np42jm3KqsxfojU`
- Last deployed slot: `475607976`
- Local SBF: `target/deploy/arena_lock_v2.so`
- SBF SHA-256: `cc17e92750e23c7bc919765acda5c99d949ac678d39f9a176e9ea4e1df520313`
- Deploy signature: `3TRQ7x613H4JebkZPcpfjW2uX9Z5JKsjFpfBDGBYDHhT4t329pcnKnhzywzDcNddk7fF1L2rYXaPtoQnK9R8SzMg`

## Bullring Test Config

- Config: `D85tMW6KxkcdCQNJREQfrAWqTW1YyWaiUoC4eTYvz8k6`
- Config id: `1783807594183`
- Authority: `242rt4KEVksuFEFuqegseVqiJ64DPPpJtMvRv87dF9nJ`
- Mint: `3sKYgehrgQVGkTGzUArebiDCeDp7UY3UmxMRjNvWxNyd`
- Vault token account: `VPHRdL2KoHcUFtuKBk7EokMHk1FLdHGv9gu9Pidu2DY`
- Reward pool token account: `ERdiVTQbpk6Yic7B4uyvTMTzbZvNus1K2ysg18XXHuMh`
- Reward funder token account: `3a6jkn5VuX9phn9Qk6cNjcJRT7E2affwNHAc6ed3ECXx`

Test policy values:

- Minimum lock: `30` seconds
- Activation delay: `1` second
- Epoch length: `2` seconds
- Early-exit penalty: `1000` bps
- Burn bps: `100` bps

Production policy should use the product cadence instead of this short test
cadence.

## Verification

Commands run after deployment:

```bash
RUST_LOG=error cargo test -p arena-lock-v2 -- --test-threads=1
NO_DNA=1 scripts/readiness-check.sh
```

Bullring UI/devnet checks run from `Sites/bullring`:

```bash
bun run devnet:test
bun run check
bun run build
BULLRING_DEVNET_PREFUND=0 bun run devnet:verify
```

`devnet:verify` confirmed the deployed program binary hash matches the local
SBF. Its only blocker is the still-present upgrade authority, which is expected
for this test deployment.
