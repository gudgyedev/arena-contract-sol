# Adversarial security findings — arena-lock-v2

In-repo tracker after the adversarial re-review. Use this when re-running review.

## Status summary (engineering)

| ID | Severity | Status |
|----|----------|--------|
| **H-01** | High — reward remainder re-index / pool insolvency | **FIXED** |
| **H-02** | High — JIT reward sniping | **FIXED** |
| **H-03** | Medium–High — early-exit penalty floor bypass | **FIXED** |
| **M-04** | Medium — permanent reward dust | **MITIGATED** |
| **M-05** | Medium — lifetime counter DoS | **FIXED** (saturating telemetry) |

**Still not “firm-audited mainnet public funds.”** Do not freeze production for TVL without external review + verified builds.  
**Code is ready for another adversarial pass** on this worktree.

---

## H-01 — Reward remainder re-index — FIXED

**Root cause:** `RollEpoch` left `pending -= accounted` and re-indexed the remainder forever.

**Fix:** Once-only batch consumption when `accounted > 0`; dust → `reward_dust` (never re-indexed); if `accounted == 0`, leave pending unindexed.

**Test:** `roll_epoch_does_not_recredit_reward_remainder`

---

## H-02 — JIT reward sniping — FIXED

**Root cause:** Activate immediately increased `eligible_locked` used by the next `RollEpoch`, so a late large deposit could capture already-funded rewards in the same atomic flow.

**Fix:** Activation places stake in **warming** (`warming_amount` / `warming_locked`).  
- Distribution and `FundRewards` use **mature** `eligible_locked` only.  
- Warming matures when `current_epoch > warming_epoch` on the next position touch (after ≥1 epoch roll).  
- Activate with zero pending is allowed as a maturity sync.

**Test:** `warming_stake_cannot_snipe_funded_rewards_on_same_epoch_roll`

---

## H-03 — Penalty floor bypass — FIXED

**Root cause:** Per-withdraw `floor(amount * bps / 10_000)` let split withdraws pay zero while bulk paid non-zero.

**Fix:** Cumulative remainder on the position (`penalty_remainder`).  
`penalty = (amount * bps + remainder) / 10_000`, store new remainder.  
Full confiscation allowed when cumulative penalty equals the withdrawn amount.

**Tests:** `split_early_exit_penalty_matches_bulk_exit`, `processor::unit_tests::cumulative_penalty_matches_bulk_for_fifty_percent`

---

## M-04 — Permanent dust — MITIGATED

- Global floor dust tracked in `reward_dust`, burned when mature arena is empty.  
- Per-position `reward_accrual_remainder` reduces multi-user accrual dust over time.  
- Residual: if all stakers leave without claiming while index dust remains, empty-arena burn path recovers unallocated pool tokens (pending + dust). Claimable vs index mismatch across many users can still leave sub-unit residuals by design of floor math.

---

## M-05 — Lifetime counter DoS — FIXED

Telemetry counters (`total_rewards_funded/claimed/distributed`, penalties, burns, expired) use **saturating** adds so they cannot brick fund/claim/exit. Critical balances still use checked arithmetic.

---

## Deployment / process (still required for mainnet)

1. No mainnet program yet — correct until re-review + audit decision  
2. Freeze **only** the program id returned by deploy on that cluster  
3. Verified builds / bytecode attestation  
4. Multisig/timelock before or instead of immediate immutability  
5. Client/site must teach warming → mature epoch UX  

## Custody strengths (unchanged)

Canonical PDAs, vault authority seeds, token program + mint pinning, revoked mint/freeze, Token-2022 extension allowlist — no known “steal someone else’s principal” path.
