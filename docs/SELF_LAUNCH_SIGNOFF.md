# Bullring Unaudited Self-Launch Sign-Off

Decision date: 2026-07-12

## Owner decision

The project owner has explicitly decided to launch without an external audit
and accepts the residual smart-contract, economic, operational, market,
key-custody, and legal risk. The release must be described as **unaudited**.
Nothing in this repository is an audit opinion, warranty, guarantee, or promise
that funds cannot be lost.

## Engineering evidence accepted for the release candidate

- 6 native unit tests and 24 ProgramTest integration tests pass.
- Deterministic Solana 2.3.0 build is stable across clean rebuilds.
- Reviewed artifact SHA-256:
  `67718f595b6558f4f598132911afc839c9f4023e43d3c21d2f9cd75103ae0629`.
- Verifier executable hash:
  `8ee664f6d81020599cc941056745e61fca6aa576218aac04888fb971cda10424`.
- The devnet deployment at `At5K4wSgzNawzGGYMzMHNXUxtJ3yjU6gbgbj8MpSBMUz`
  exactly matches the reviewed executable bytes (zero-only loader padding).
- Real devnet transactions verified deposit, activation, eligibility, claim,
  early withdrawal, 1% burn, and pro-rata 9% redistribution across four
  wallets. Conservation reconciled to the token.
- Token-2022 dangerous extensions fail closed; only the reviewed metadata and
  immutable-owner cases are accepted.
- Formatting, strict clippy, SBF build, dependency policy/audit review, source
  scan, website type-check, and website production build pass.

## Not implied by this sign-off

- No independent auditor reviewed or certified this release.
- Testing cannot prove the absence of every vulnerability or economic edge case.
- Pump.fun, Phantom, Solana RPC providers, Jupiter, Cloudflare, and wallet/user
  behavior remain outside the program's security boundary.
- Legal or jurisdiction-specific compliance has not been determined here.

## Final click sequence

Every item below must be completed in order. A checked engineering candidate is
not permission to skip launch-time binding checks.

1. Publish the exact contract release commit and require green CI.
2. Rebuild it deterministically and confirm both recorded hashes.
3. Generate a fresh mainnet program keypair and record only its public address.
4. Review the exact deploy summary; explicitly confirm before sending mainnet.
5. Verify the deployed bytes and upgrade authority on mainnet.
6. Launch the Pump.fun token and provide its mint address (CA).
7. Inspect mint owner/extensions, decimals, supply, and mint/freeze authorities.
8. Put the CA and mainnet program link on the site in launch mode while arena and
   swap actions remain gated.
9. Review the exact config initialization summary; explicitly confirm before
   creating the vault, reward pool, and config.
10. Verify every on-chain binding and publish the final production environment.
11. Perform a low-value manual Phantom deposit/activate/claim/withdraw smoke.
12. Make the program immutable, or document the temporary authority, its
    custody, and a concrete removal deadline.

## Launch policy frozen for initialization

- Minimum lock: 604,800 seconds (7 days)
- Activation delay: 2 seconds
- Epoch: 259,200 seconds (3 days)
- Early-exit penalty: 1,000 bps (10%)
- Direct burn share: 100 bps (1% of the withdrawn amount)
- Remaining early-exit penalty: pro-rata redistribution to eligible stake when
  eligible stake remains; otherwise the entire penalty is burned

Changing these values requires a new economic review before initialization.
