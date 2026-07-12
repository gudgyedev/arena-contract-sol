# Bullring Mainnet Launch Readiness

Review date: 2026-07-12

## Current verdict

**NO-GO for public mainnet funds.** The engineering candidate is materially
stronger, but the release evidence is not yet complete enough to call it
mainnet-ready.

The active review worktree is based on commit `2c3eb4c` and contains additional
uncommitted hardening. A new release commit and hash must be created after the
full gate passes. Do not deploy or attest `2c3eb4c` as the final candidate.

## Evidence currently passing

- Native program suite: 6 unit tests and 24 ProgramTest integration tests.
- Deterministic Solana 2.3.0 Docker build, with verifier executable hash
  `8ee664f6d81020599cc941056745e61fca6aa576218aac04888fb971cda10424`
  and artifact SHA-256
  `67718f595b6558f4f598132911afc839c9f4023e43d3c21d2f9cd75103ae0629`.
- Multi-user reward conservation across entry, funding, claim, and exit churn.
- Fail-closed Token-2022 extension allowlist tests.
- Formatting, strict clippy, SBF build, cargo-deny production dependency gate,
  cargo-audit review, and raw-memory source scan.
- Bullring site type-check and production build.
- Website live binding checks for program-derived config, mint, token program,
  authority, vault, and reward pool.
- Website now shows an explicit action summary before opening Phantom.
- Prior devnet manual transactions reconciled deposit, activation, early-exit
  return, burn, and empty final custody state.

## New hardening in this review

1. `InitializeConfig` rejects a minimum lock shorter than activation delay plus
   two epochs. This prevents stake from finishing warming after its penalty
   window has already expired.
2. The proposed Bullring policy is a 7-day minimum lock, 2-second activation
   delay, and 3-day epoch.
3. Multi-user fractional funding is regression-tested for conservation.
4. Token-2022 metadata/immutable-owner support and dangerous extension
   rejection are explicitly regression-tested.
5. Deployment/finalization scripts require explicit program, RPC, reviewed
   commit, clean worktree, matching upgrade signer, matching on-chain bytes,
   and separate upgrade/finalization confirmations.
6. Transitive dependencies are pinned to the Solana verifiable-build toolchain
   rather than accepting a build that only succeeds on Honey.

## Operational custody caveat

Tokens sent directly to a custody address do not run the program's accounting
instructions. An unsolicited direct transfer to the reward pool becomes
finalizable surplus; an unsolicited direct transfer to the principal vault is
not credited to any position and is intentionally not recoverable by an admin.
The site must only use the program instructions, and published documentation
must warn users not to send tokens directly to either custody address.

## Blocking gates

- [x] Deterministic Docker build succeeds and its hash is recorded.
- [ ] Final hardening is committed and pushed to the public repository.
- [ ] CI passes on the exact release commit.
- [ ] Independent Solana-focused reviewer/audit firm reviews the final commit;
      no unresolved critical/high findings remain.
- [ ] Final token mint exists and mint/freeze authorities are revoked.
- [ ] Fresh mainnet program is deployed from the verified release artifact.
- [ ] Mainnet config uses the approved policy and fresh PDA/custody accounts.
- [ ] Vault/reward accounts are independently verified for mint, owner,
      delegate, close authority, frozen/native state, and Token-2022 extensions.
- [ ] Upgrade authority is transferred to the approved multisig/timelock, with
      signers and recovery policy documented. Immutability is considered only
      after audit, deploy verification, and a public soak window.
- [ ] Production site is pinned to the exact mainnet program/config/mint and
      passes manual Phantom deposit, activate, roll, fund, claim, early-exit,
      mature-exit, and failure-path testing.
- [ ] Monitoring, incident communications, RPC failover, and treasury funding
      procedures are documented and exercised.
- [ ] Legal review covers the token launch, staking language, penalty
      redistribution, and jurisdiction-specific risks.

## Required release order

1. Finish engineering hardening and deterministic verification.
2. Commit/publish one frozen candidate and obtain external review.
3. Fix findings, repeat verification, and freeze a new commit if necessary.
4. Create the final mint and revoke mint/freeze authorities.
5. Deploy a fresh mainnet program from the verified SBF.
6. Initialize one fresh Bullring config and verify every bound account.
7. Transfer upgrade authority to the approved governance setup.
8. Pin and deploy the production site, then perform manual wallet testing.
9. Run a public low-value soak before accepting unrestricted public value.
10. Consider immutability only when all prior evidence is final.
