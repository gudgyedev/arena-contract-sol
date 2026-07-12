# Arena Lock V2 Devnet Deployment

Review date: 2026-07-12

This deployment is for Bullring UI/devnet testing only. It is intentionally
upgradeable while testing continues.

## Program

- Cluster: devnet
- Program id: `At5K4wSgzNawzGGYMzMHNXUxtJ3yjU6gbgbj8MpSBMUz`
- ProgramData: `D2ZVopHud1bz6GGERZf6yuknuYffxkQqnHWJzPLRgqFG`
- Upgrade authority: `HxfS8TXM9bprXe1jD2RgcAU1c8nd9np42jm3KqsxfojU`
- Reviewed source commit: `c5cbc0bba2568bc8820f5d3ffa3ac57321d19638`
- Last deployed slot: `475812420`
- Local SBF: `target/deploy/arena_lock_v2.so`
- Solana verifier executable hash: `8ee664f6d81020599cc941056745e61fca6aa576218aac04888fb971cda10424`
- SBF SHA-256: `67718f595b6558f4f598132911afc839c9f4023e43d3c21d2f9cd75103ae0629`
- Corrected deploy signature: `pURQUjipH6uzM5PMWEz688JXXA53hNchGEubVwGtE5WBb2mF91frQQnfE7DJKUQCngKzST66bTJgHprDvp2aDoK`
- Padded program-data SHA-256: `fd83eef2862770254ed5107dbc92fd178c40bb90f003f1bfa079a463670906f5`

The loader account is 183,472 bytes while the reviewed SBF is 181,824 bytes.
The corrected deploy explicitly wrote 1,648 zero padding bytes. An initial
shorter upgrade retained a non-zero byte from the prior binary; the deployment
integrity gate rejected it before the zero-padded correction was accepted.

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
RUST_LOG=error cargo test --locked -p arena-lock-v2 -- --test-threads=1
NO_DNA=1 bash scripts/readiness-check.sh
```

Bullring UI/devnet checks run from `Sites/bullring`:

```bash
bun run devnet:test
bun run check
bun run build
bun run devnet:verify
```

`devnet:verify` passes with the deployed program prefix matching the reviewed
SBF and zero-only loader padding. It warns that the expected hot devnet upgrade
authority remains present for the active test cycle; that authority model is
not acceptable for mainnet.
