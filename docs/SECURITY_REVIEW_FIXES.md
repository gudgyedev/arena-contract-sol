# Security review fixes (2026-07-11)

Follow-up to the independent AI code review of `arena-lock-v2`.

## Fixed in program

| ID | Fix |
|----|-----|
| **H1** | Deposit no longer resets the whole lock clock. First stake sets timers; top-ups only **extend** `unlock_ts` via `max(old, now+min_lock)` and only re-delay activation for pending. `lock_start_ts` stays on first entry. |
| **M1** | Early-exit penalty uses **floor** division (removed ceiling). Tiny amounts can early-exit without `returned == 0` soft-lock. |
| **M2** | `ActivatePosition` and `RollEpoch` take reward-pool / vault-authority / mint / token-program accounts for custody validation. In v2, reward distribution moved to funding time; `RollEpoch` no longer burns or distributes pending batches. |
| **L2** | Explicit `is_writable` checks on state/token accounts that the program mutates. |

## Operational (C1 / C2 / M3) — not pure bytecode

| ID | Status |
|----|--------|
| **C1** | Documented launch gate: mainnet upgrade authority must be multisig/timelock or program made immutable after final deploy. Devnet may keep a single upgrade key. See `AUDIT_PACKAGE.md`. |
| **C2** | Bullring client `assertLiveBinding()` refuses txs in `coming-soon` mode and checks mint / vault / pool / token program / authority match on-chain config. |
| **M3** | Intentional: no in-program config mutate. Parameter changes = new config or audited upgrade. Documented. |
| **L1** | Reworked: `reward_dust` is deprecated; final surplus is burned only by `FinalizeRewards` using the actual reward-pool token balance after all stake/debt is gone. |
| **L3** | Tracked via `deny.toml` / readiness script; no code change. |

## Client / ABI note

`ActivatePosition` and `RollEpoch` account layouts changed. `FinalizeRewards`,
`FundRewardsChecked`, and shifted `ClaimRewards`/`Withdraw` enum tags require all
manual builders (Rust helpers + Bullring `arena.ts` / devnet scripts) to be
updated together with the program upgrade.

## Tests added

- `deposit_extends_unlock_without_shortening_existing_lock`
- `tiny_early_exit_with_floor_penalty_returns_principal`
- `prearmed_warming_stake_cannot_sync_then_snipe_target_roll`
- `maturing_topup_preserves_existing_eligible_rewards`
- `fractional_batches_do_not_block_full_exit_and_finalize_surplus`
- `checked_reward_funding_rejects_stale_eligible_snapshot`
