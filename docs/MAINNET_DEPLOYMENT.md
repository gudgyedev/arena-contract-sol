# Bullring Mainnet Program Deployment

Deployment date: 2026-07-12

## Identity

| Field | Value |
|---|---|
| Cluster | `mainnet-beta` |
| Genesis hash | `5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d` |
| Program id | `DACFfLpaVw2Q7dz4mEUVBFzR7VjjTxMJT71AFBDLJmwU` |
| ProgramData | `4SDKR8Qb4fo4fFsktWjS3xKyNX9mPuEc11xmYCuuGb5R` |
| Upgrade authority | `HxfS8TXM9bprXe1jD2RgcAU1c8nd9np42jm3KqsxfojU` |
| Loader | `BPFLoaderUpgradeab1e11111111111111111111111` |
| Deployment slot | `432549714` |
| Transaction | `5bNt9wimPwMfmQzbkviZSqc7rNvsUPFkJNT5RXqGnmFdhcEKf6z4CK6TJ9B4NBKYud29zxH8JEGddX7apPb6Z5q3` |

## Reviewed release

| Field | Value |
|---|---|
| Release commit | `55eb89f2757962bf5b91f619676dbf46c811069e` |
| Artifact | `target/deploy/arena_lock_v2.so` |
| Artifact length | `181824` bytes |
| Artifact SHA-256 | `67718f595b6558f4f598132911afc839c9f4023e43d3c21d2f9cd75103ae0629` |
| Verifier executable hash | `8ee664f6d81020599cc941056745e61fca6aa576218aac04888fb971cda10424` |

## Read-back verification

The finalized mainnet executable was dumped independently after deployment:

```text
dump-bytes=181824
dump-sha256=67718f595b6558f4f598132911afc839c9f4023e43d3c21d2f9cd75103ae0629
exact-byte-match=yes
```

The deployment transaction finalized successfully. The temporary upload buffer
was closed and no mainnet buffer remained under the authority. The authority
balance after deployment was `1.73124044 SOL`.

## Current state

- Program is upgradeable during the controlled mainnet test phase.
- No mainnet arena config, principal vault, reward pool, position, or official
  token binding was created by this deployment.
- Immutability is a separate irreversible transaction after test-token and
  official-mint verification.
