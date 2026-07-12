# Bullring Mainnet Launch Readiness

Review date: 2026-07-12

## Current verdict

**ENGINEERING SIGN-OFF: unaudited self-launch release candidate.** The owner has
explicitly decided not to obtain an external audit and accepts that residual
smart-contract, economic, operational, market, key-custody, and legal risk
remains. This is not an audit opinion, a security guarantee, or a statement that
public funds cannot be lost.

The reviewed contract is ready to advance through the controlled mainnet launch
sequence below. It is not yet live: the final Pump.fun mint, fresh mainnet
program/config, production site binding, mainnet smoke transaction, and final
upgrade-authority decision necessarily happen at launch time.

The hardened executable source is frozen at commit
`c5cbc0bba2568bc8820f5d3ffa3ac57321d19638` on
`codex/mainnet-readiness-hardening`. Publication, CI, tagging, and a rebuild of
the exact final release commit remain mechanical release gates.

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
- The current candidate is deployed on devnet at slot `475812420`; its on-chain
  executable hash exactly matches the reviewed artifact. Loader padding was
  independently verified as zero-only.

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

## Release gates

- [x] Deterministic Docker build succeeds and its hash is recorded.
- [ ] Final hardening is pushed to the public repository (local source commit
      exists; publication remains pending).
- [ ] CI passes on the exact release commit.
- [x] External audit is not a launch gate by explicit owner decision. The
      release must always be described publicly as unaudited.
- [ ] Final token mint exists and mint/freeze authorities are revoked.
- [ ] Fresh mainnet program is deployed from the verified release artifact.
- [ ] Mainnet config uses the approved policy and fresh PDA/custody accounts.
- [ ] Vault/reward accounts are independently verified for mint, owner,
      delegate, close authority, frozen/native state, and Token-2022 extensions.
- [ ] After deploy/config/site verification, explicitly choose either immediate
      immutability or a documented temporary upgrade authority. Immutability is
      irreversible; keeping a hot authority preserves upgrade/key-compromise
      risk.
- [ ] Production site is pinned to the exact mainnet program/config/mint and
      passes manual Phantom deposit, activate, roll, fund, claim, early-exit,
      mature-exit, and failure-path testing.
- [ ] Monitoring, incident communications, RPC failover, and treasury funding
      procedures are documented and exercised.
- [x] Legal review is owner-waived as a technical launch gate. No legal opinion
      is provided; jurisdiction, disclosures, and launch compliance remain the
      owner's responsibility.

## Required release order

1. Finish engineering hardening and deterministic verification.
2. Commit/publish one frozen candidate; require green CI on that exact commit.
3. Rebuild the exact release commit and confirm the recorded artifact hashes.
4. Deploy the fresh mainnet program only after the explicit transaction review.
5. Launch the final mint; inspect its program, extensions, decimals, supply, and
   mint/freeze authorities before binding it to the arena.
6. Initialize one fresh Bullring config and verify every bound account.
7. Pin/deploy the production site and perform a low-value manual wallet smoke.
8. Explicitly make the program immutable or document the temporary upgrade
   authority and its removal deadline.
9. Open unrestricted use only after the exact production binding is verified.

See `docs/SELF_LAUNCH_SIGNOFF.md` for the owner decision and the final click
sequence.
