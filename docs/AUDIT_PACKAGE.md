# Arena Lock V2 Audit Package

This document defines what must be sent to an external reviewer before Bullring is considered mainnet-launch ready.

## Scope

- Repository: `arena-contract-sol`
- Program: `programs/arena-lock-v2`
- Program id in source: `AV4FTAiteCN75iq6QbuPTuh2PVL4FKwyiWJiowhhzAsQ` (do not assume this is the final deploy id)
- Token support: SPL Token and Token-2022 with only allowlisted metadata/owner extensions
- Out of scope unless explicitly included: Bullring website, deployment wallet custody, token launch mechanics, market making, Pump.fun launch flow, and off-chain keepers

## Product Rules

- Users deposit the configured mint into a PDA-owned vault token account.
- Deposits enter pending activation first, then warming, then mature eligible stake after an epoch roll plus a position touch.
- Rewards are explicitly funded or created by early-exit penalties. Do not describe them as APY or automatic yield.
- Treasury rewards index immediately at funding time against mature eligible stake.
- `RollEpoch` advances maturity cadence; it does not distribute a pending reward batch.
- Full exit settles pending rewards during withdraw so a user cannot be unstaked with a separate dangling claim.
- Claim requires the position to still have locked stake.
- Early-exit penalty applies only before `unlock_ts`.
- If no eligible stake remains after an early withdrawal, the reward-share penalty is burned rather than left for the next entrant to snipe.

## Launch Preconditions

- The staking mint must have mint authority revoked before `InitializeConfig`.
- The staking mint must have freeze authority revoked before `InitializeConfig`.
- Custody token accounts must be owned by the arena vault-authority PDA.
- Custody token accounts must not be frozen, native, delegated, or close-authority controlled.
- Token-2022 mint extensions are limited to `MetadataPointer` and `TokenMetadata`.
- Token-2022 account extensions are limited to `ImmutableOwner`.
- Production upgrade authority must be controlled by multisig or timelock before public value is accepted.
- After final audit fixes, either make the program immutable or publish the exact upgrade-authority policy.
- Do not submit `.env`, payer keypairs, deploy keypairs, or generated `target/` artifacts as source.

## Invariants Reviewers Should Check

- Account identity:
  - Config PDA derives from `arena-config`, authority, and `config_id`.
  - Position PDA derives from `arena-position`, config, and owner.
  - Vault authority PDA derives from `arena-vault-authority` and config.
  - Stored config/position data must match the passed account addresses.
- Token custody:
  - Deposits can only transfer from the signing owner token account into the configured vault account.
  - Withdrawals can only transfer from the configured vault account to the signing owner token account.
  - Rewards can only transfer from the configured reward-pool account to the signing owner token account.
  - Burns can only burn from PDA-owned custody accounts.
- Accounting:
  - `total_locked == eligible_locked + warming_locked + pending_activation_locked` after each successful stake movement.
  - Position locked amount equals eligible plus warming plus pending activation amount.
  - Funding commits exactly one indexed batch against the mature eligible snapshot.
  - `reward_dust` remains zero in v2; final surplus burns use actual token-account balance.
  - Full exit leaves `pending_rewards == 0` on the position.
  - Early-exit penalty plus returned principal equals withdrawn amount.
  - Unsupported state versions are rejected.
- Liveness:
  - Anyone may roll an epoch once the epoch time has elapsed.
- Anyone may fund rewards only when mature eligible stake exists. Treasury/keeper flows should use `FundRewardsChecked`.
  - No admin-only instruction is required for normal user exit.

## Required Evidence Before Launch

Run and save output for:

```bash
scripts/readiness-check.sh
```

That script runs formatting, arena-lock-v2 tests, clippy, SBF build, cargo-deny advisories/sources, cargo-audit, and an unsafe/raw-memory source scan.

Also capture:

- Git commit hash submitted to auditors.
- Deployed program id and upgrade authority.
- Config PDA, vault token account, reward-pool token account, mint, token program, and configured epoch/lock/penalty values.
- On-chain proof that mint authority and freeze authority are revoked.
- On-chain proof that vault and reward-pool token accounts have no delegate or close authority.
- Bullring site read-only verifier output: `cd ../Sites/bullring && bun run devnet:verify`.
- Devnet or localnet transaction set showing deposit, activate, checked fund, roll, claim, mature withdraw, early withdraw, final reward surplus handling, and failed attack substitutions.

## Known Design Choices

- This program has no in-place config-update instruction. Changing lock/epoch/penalty policy requires a fresh config or program upgrade.
- Reward funding is explicit. The contract does not create yield by itself.
- Token-2022: only MetadataPointer + TokenMetadata (mint) and ImmutableOwner (accounts) are allowed — for Pump.fun compatibility. Transfer hooks, transfer fees, permanent delegate, confidential transfer, etc. are still rejected.
- Reward surplus can remain as real tokens in the reward pool until `FinalizeRewards` burns the actual balance after all stake and reward debt are gone.
- Old vulnerable config/position state must not be upgraded in place. Version 2 rejects unsupported state; use fresh config state unless a bespoke migration is reviewed.
- `cargo audit` currently reports allowed warnings from transitive Solana/tooling dependencies. `deny.toml` documents the two unmaintained Solana-transitive advisories accepted for this release candidate; reviewers should still inspect the full readiness output.

## Acceptance Bar

Do not accept mainnet public funds until:

- The current commit passes all required evidence checks.
- External reviewers produce no unresolved critical or high severity findings.
- Medium findings are either fixed or explicitly accepted in writing with user-facing impact understood.
- Upgrade authority and mint/freeze authority status are independently verified on-chain.
- The Bullring website signs only transactions matching the audited program id, config, mint, token program, vault, and reward-pool accounts.

## Security review fixes (in-repo)

See `docs/SECURITY_REVIEW_FIXES.md` for H1/M1/M2/L2 code changes and C1/C2 operational gates.

### Upgrade authority (C1)

Before mainnet public funds:

1. Transfer upgrade authority to a multisig (e.g. Squads) with timelock, **or**
2. Set upgrade authority to `None` (immutable) after the final audited build is deployed.
3. Publish the authority pubkey / policy in the launch post.

### Website binding (C2)

Bullring must only sign when:

- `PUBLIC_SITE_MODE=live`
- Site mint, vault, reward pool, token program, and authority match the on-chain config
- Program id matches the audited deployment

### Config policy (M3)

There is no `UpdateConfig` instruction. Changing epoch/lock/penalty requires a new config id or a program upgrade under the C1 policy.
