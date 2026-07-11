# Adversarial Security Findings - arena-lock-v2

Tracker for the 2026-07-11 adversarial re-audit fixes. This is engineering
evidence for another review pass, not an audit-firm opinion.

## Status Summary

| ID | Severity | Status |
|----|----------|--------|
| H-01 | High - pre-armed warming/JIT reward sniping | Fixed |
| H-02 | High - top-up maturity erases existing rewards | Fixed |
| H-03 | Medium-High - unsafe reward dust / final exit block | Fixed |
| M-01 | Medium-High - split penalty/burn bypass | Fixed for one position lifecycle; Sybil economics documented |
| M-02 | High operational - no safe migration for old state | Fixed by v2 version rejection; use fresh configs |
| M-03 | Medium - permanent real-token surplus | Fixed by explicit finalization from actual pool balance |
| M-04 | Medium - finite telemetry/index blockers | Mitigated |
| Low | Funder slippage, empty-roll cadence, claim sync rollback | Mitigated |

The correct release status remains: ready for re-review, not ready for
mainnet public funds or immutability without external review, verified build
evidence, and final deployment governance.

## H-01 - Pre-Armed Warming Sniping - Fixed

Old risk: a large warming position could be left ready to mature, then synced
immediately before a funded roll and claim most of a reward batch.

Fix:

- `FundRewards` indexes rewards immediately against the current mature
  `eligible_locked` snapshot.
- `RollEpoch` only advances epoch cadence and maturity eligibility. It no
  longer distributes a mutable pending reward batch.
- Warming stake still matures only when `current_epoch > warming_epoch` during a
  position touch, so it cannot alter already-indexed funding.
- `FundRewardsChecked` lets funders bind the transfer to
  `expected_eligible_locked` and `max_current_epoch`.

Evidence:

- `prearmed_warming_stake_cannot_sync_then_snipe_target_roll`
- `checked_reward_funding_rejects_stale_eligible_snapshot`

## H-02 - Maturing Top-Up Erases Rewards - Fixed

Old risk: maturing a top-up reset the whole position checkpoint before accruing
the already-eligible tranche, erasing rewards it had earned.

Fix:

- `mature_warming_if_ready` now accrues the existing eligible amount before
  promoting warming stake.
- The combined position checkpoint is then set to the current reward index and
  generation.

Evidence:

- `maturing_topup_preserves_existing_eligible_rewards`

## H-03 - Unsafe Reward Dust / Final Exit Block - Fixed

Old risk: `reward_dust` counted floor remainders as burnable even though position
fractional carry could later crystallize them into claims. Final withdraw could
try to burn more than the real pool balance and roll back principal exit.

Fix:

- `reward_dust` is deprecated in v2 and must remain zero.
- Funded batches are committed exactly once by increasing the reward index and
  keeping the actual pool balance as backing.
- Withdraw no longer performs final unallocated reward burns.
- `FinalizeRewards` burns the actual reward-pool token balance only when
  `total_locked`, `eligible_locked`, `warming_locked`,
  `pending_activation_locked`, and config `pending_rewards` are all zero.

Evidence:

- `fractional_batches_do_not_block_full_exit_and_finalize_surplus`
- `roll_epoch_does_not_recredit_reward_remainder`

## M-01 - Penalty / Burn Partitioning - Fixed With Limits

Fix:

- Early-exit penalty uses cumulative per-position `penalty_remainder`.
- Direct burn uses independent cumulative `burn_remainder`.
- Initialization rejects configs where `min_deposit_amount * penalty_bps` or
  `min_deposit_amount * burn_bps` cannot charge at least one raw unit when the
  respective bps is non-zero.
- When no mature denominator remains after an early withdrawal, the reward-share
  penalty is burned instead of being left for the next entrant.

Evidence:

- `split_early_exit_penalty_matches_bulk_exit`
- `processor::unit_tests::cumulative_penalty_matches_bulk_for_fifty_percent`
- `processor::unit_tests::cumulative_burn_matches_bulk_for_small_splits`

Residual economic truth:

- Redistributed penalties are never Sybil-resistant in the same way as direct
  burn or treasury capture. A controlled remaining position can still receive
  some redistributed penalties. This is a tokenomics choice, not a custody break.

## M-02 - Old State Migration - Fixed By Rejection

Fix:

- Config and position state now carry explicit version `2`.
- Load rejects unsupported versions.
- Old configs that processed rewards under the previous algorithm must not be
  upgraded in place. Use fresh config/program state unless a bespoke migration
  proof is written and audited.

Evidence:

- `ArenaConfig::load`
- `ArenaPosition::load`

## M-03 - Real Token Surplus - Fixed

Fix:

- Direct donations or sub-unit surplus are not guessed from counters.
- `FinalizeRewards` burns actual reward-pool token balance after all stake and
  reward accounting is empty.

Evidence:

- `fractional_batches_do_not_block_full_exit_and_finalize_surplus`

## M-04 - Finite Arithmetic / Availability - Mitigated

Fixes:

- `total_deposited` and other telemetry use saturating addition where custody
  does not depend on the counter.
- `reward_index_generation` tracks one u128 index wrap for accrual.
- Burn math uses u128 intermediates.
- `reward_dust` no longer grows into a global blocker.

Residual:

- More than one reward-index generation between a position's touches is rejected.
  Creating that condition requires enormous backed reward funding, but reviewers
  should still inspect the accumulator design.

## Low Findings - Mitigated

- Claim maturity sync now persists and returns success when there are no rewards
  but maturity happened.
- `RollEpoch` advances by the configured epoch boundary instead of resetting
  cadence to `now`.
- `FundRewardsChecked` provides denominator/epoch protection for treasury
  funding flows.
- Position rent close remains intentionally out of scope for this pass.

## Verification Run

Passing local evidence from this worktree:

```bash
cargo fmt
cargo test -p arena-lock-v2
cargo test -p arena-lock-v2 -- --test-threads=1
```

## Launch Gate

Do not accept public mainnet value until:

1. A fresh config/program state is deployed from this exact reviewed commit.
2. Source, site, deployment manifest, and on-chain bytes are synchronized.
3. Pinned CI, SBF build, clippy, audit tooling, and bytecode attestation pass.
4. A third-party Solana audit has reviewed this replacement accounting model.
5. Upgrade authority is multisig/timelock controlled, then immutable only after
   audit and soak.
