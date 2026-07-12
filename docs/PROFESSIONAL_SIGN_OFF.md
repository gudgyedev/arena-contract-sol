# Professional sign-off — arena-lock-v2

**Date:** 2026-07-11 (post adversarial fix pass)  
**Repository:** `arena-contract-sol`  
**Program:** `programs/arena-lock-v2`  
**Reviewer role:** Engineering security work (AI-assisted + human-directed), **not** a licensed audit firm  

---

## Honest scores

| Layer | Score |
|--------|--------|
| Account / PDA / custody | **~8.5–9 / 10** (still strong) |
| Economic / reward / penalty logic after this pass | **Ready for re-review** - re-audit Highs addressed in source |
| Mainnet public-funds readiness | **Still not 10/10** — needs external audit, verified builds, ops |
| Prior “10/10 engineering RC” claim | **Retracted** and replaced by this document |

---

## What changed since the adversarial findings

| Finding | Treatment |
|---------|-----------|
| H-01 pre-armed warming sniping | Fixed by funding-time indexing + checked funding |
| H-02 top-up maturity erases rewards | Fixed by accrue-before-promotion |
| H-03 unsafe reward dust/final exit block | Fixed by actual-balance finalization |
| M-01 penalty/burn partitioning | Fixed for one position lifecycle via independent remainders + min raw-unit guard |
| M-02 old state migration | Fixed by v2 state-version rejection; fresh configs required |
| M-03 real-token surplus | Fixed by `FinalizeRewards` |
| M-04 finite counters/index | Mitigated; reviewers should inspect accumulator limits |
| F-01 lock expires before maturity/funding exposure | Fixed by enforcing minimum lock >= activation delay + two epochs |

See `docs/SECURITY_ADVERSARIAL_FINDINGS.md`.

---

## Verification expected before re-review

```bash
cd arena-contract-sol
NO_DNA=1 cargo test --locked -p arena-lock-v2 -- --test-threads=1
NO_DNA=1 cargo clippy --locked -p arena-lock-v2 --all-targets --all-features -- -D warnings
NO_DNA=1 cargo build-sbf --manifest-path programs/arena-lock-v2/Cargo.toml -- --locked
solana-verify build --library-name arena_lock_v2
```

---

## What this sign-off is **not**

- Not a firm audit letter  
- Not permission to take public mainnet funds without further review  
- Not a guarantee of zero residual economic dust or market risk  

**Signed:** Highs from the adversarial package are addressed in source; program is appropriate to **re-run adversarial review**. Mainnet/freeze only after that review (and ideally formal audit) remains green.
