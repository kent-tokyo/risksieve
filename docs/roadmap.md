# Roadmap

Tracked, publishable record of what each milestone in `AGENTS.md` section 7
still has open, and why. This is the file README, rustdoc, and
`docs/validation.md` point to for "not yet implemented" details; it
replaces the local-only `tasks/todo.md` as the public-facing backlog (see
`AGENTS.md` and `CLAUDE.md` for this repository's local, untracked
development notes).

## Remaining within Milestone 2 (anytime-valid CRC)

- Proposition 4.5 (asymptotic tightness diagnostics) not implemented.
- Re-verify the `f_{B,m,delta}` constants `1.44` / `2.42` against a
  canonical secondary source (peer-reviewed version, author
  implementation, etc.) — currently only extracted from arXiv's HTML
  rendering.
- Optimize `m_star`'s linear search (currently `O(m*)`) to, for example,
  binary search if it ever matters — left simple on purpose per "reference
  implementation first".

## Remaining within Milestone 3 (non-monotonic CRC)

Theorem 1 (the general symmetry + beta-stability reduction) is done in
`src/nonmonotone/stability.rs`. Not yet implemented, in the paper's own
"increasing order of generality":

- Proposition 2: bounded-loss discretization (`Theta_m = {0, 1/m, ..., 1}`).
  Its guarantee is `E[R] <= alpha + O~(1/sqrt(n))` involving the `-1`
  branch of the Lambert W function (`W_{-1}(-1/(4n(m+1)^2))`), not a clean
  closed-form beta — verify this formula against a secondary source before
  implementing.
- Proposition 3 / Corollary 2: continuous Lipschitz losses,
  `beta = L/(m(n+1))`.
- Proposition 4 / 5 / Corollary 3: selective classification,
  `beta = 2*max{alpha,1-alpha}*E[K]/(n+1)`.
- Proposition 6 / Corollary 4: regularized ERM,
  `beta <= 2*E[rho(Z_{n+1})^2]/(lambda(n+1))` — need to pin down what `rho`
  is exactly before implementing.
- Theorem 2, Propositions 7/8: multivariate/gradient extension and
  multigroup debiasing. `certify`'s `Parameter` type already accepts any
  type with no bounds, so multidimensional support for Theorem 1 itself
  needs no further work; this item is about the paper's ERM-gradient
  machinery specifically, not generic dimensionality.
- Stability estimation as diagnostics (`StabilityEvidence::Estimated`
  producing more than `EmpiricalOnly`) — only makes sense once at least
  one analytic proposition above exists to compare an estimator against.

## Remaining within Milestone 4 (SCoRE-MDR)

Definition 3.1, Equation 4.1/4.2, Algorithm 1, and Theorem 3.2 are done in
`src/selective/evalue.rs` and `src/selective/mdr.rs`. Not yet implemented:

- Theorem 4.6 (the "extra thresholding condition" Remark 4.5 mentions for
  `gamma > alpha`, needed to recover power in that regime) — the exact
  condition was not extracted with confidence; `gamma > alpha` is
  currently accepted (Theorem 4.2 says it stays valid) but with
  undocumented power characteristics beyond "worse than `gamma = alpha`".
- Wire Proposition 4.4's efficient shortcut in as an actual fast path for
  `gamma <= alpha`, if profiling ever shows `risk_adjusted_evalue`'s
  `O(n^2)` reference scan is a bottleneck — it is verified equivalent by a
  property test today but not used at runtime, left deliberately
  unoptimized per "reference implementation first". `selective::sdr::certify_independent`
  calls it once per batch item (`O(m * n^2)`); `selective::sdr::certify`
  (the default, coupled construction) does not use this per-point scan at
  all, so this item now only affects the independent path.

## Remaining within Milestone 5 (SCoRE-SDR)

Theorem 3.3 (generic eBH selection), Algorithm 2, and Equation 5.1 /
Theorem 5.1 (the paper's own cross-test-point-coupled e-value and its
efficient computation) are done in `src/selective/ebh.rs`,
`src/selective/sdr.rs`, and `src/selective/coupled.rs`. Not yet
implemented:

- Randomized pruning (`prune='hete'` / `prune='homo'` in the official
  `SCoRE_SDR`), an optional power-boosting randomization on top of the
  coupled e-value. Deliberately out of scope for this milestone (it needs
  its own RNG-dependency decision, per `AGENTS.md` section 12, and its own
  validity argument) — see `src/selective/coupled.rs`'s module docs for
  where it would plug in.
- Weighted SDR (`SCoRE_SDR_w`) — see "Remaining within Milestone 6" below;
  ordered after weighted MDR in the backlog.
- `ebh::select`'s `tau_hat` search is `O(m)` per candidate `tau` scanned
  naively rather than exploiting any structure beyond "check every
  candidate" — fine at the sizes tested, revisit only if profiling says
  so.
- A more exhaustive empirical study of when the coupled construction's
  extra selection power (over the independent composition) is largest;
  the numerical comparison so far (`docs/references.md`) covers the
  crate's own test fixtures and one constructed large-`m` scenario, not a
  systematic sweep.

## Remaining within Milestone 6 (distribution shift)

Importance-weight validation/diagnostics
(`shift::importance::WeightAccumulator`), importance-weighted
anytime-valid CRC (Theorem 4.7, `anytime::shifted::AnytimeShiftedController`),
and weighted MDR (Equation 6.1, Theorem 6.2/6.4, `SCoRE_MDR_w`) — done in
`src/selective/evalue_weighted.rs` and `selective::mdr::certify_weighted`
— are done. Not yet implemented:

- Weighted SDR (`SCoRE_SDR_w`) — the batch/eBH-selection counterpart to
  weighted MDR, composing `selective::evalue_weighted` with
  `selective::coupled`'s grouped-threshold representation the way
  `selective::sdr` composes the unweighted e-value. This is the
  recommended next PR — see this file's closing note.
- Independent verification that Theorem 4.7's `m*`, as a data-dependent
  stopping time rather than the deterministic build-time constant Theorem
  4.1's `m*` is, doesn't break any measurability assumption the paper's
  proof relies on. The scan-and-freeze logic itself is correct (a `min`
  over an increasing scan is fixed permanently the first time its
  condition holds, regardless of when it's evaluated), but the underlying
  probabilistic argument has not been independently verified the way the
  running-minimum argument in `anytime::calibration` was.
- Re-verify `weighted_term`'s constant (`1.44`, shared with `f`) and the
  exact form of the additive bias term `B*(1 - mean(omega))` against a
  canonical secondary source — the dimensional/asymptotic argument in
  `src/anytime/boundary.rs` gives high confidence in the *shape* (which
  function is used where) but the exact constants rest on the same arXiv
  HTML extraction as Theorem 4.1's.

## Milestone 7 onward (not started)

- Milestone 7: downstream examples (`masstrust`, `quietset`, `lineprior`,
  `veridict`, `renkin`).

## Built-in losses deferred from Milestone 0

- `SelectiveLoss` (design alongside Milestone 4/5).
- `WeightedLoss` (design alongside Milestone 6).
- `MultigroupLoss` (design once a consuming milestone needs it).

## Recommended next PR

Weighted SDR (`SCoRE_SDR_w`, Milestone 6/backlog item 19's remaining half)
— see `src/selective/evalue_weighted.rs` (the weighted e-value engine,
already done), `src/selective/coupled.rs` (grouped threshold
representation and prefix accumulators for the cross-test-point
construction), and `src/shift/importance.rs` (weight validation and
diagnostics) for the building blocks it would compose. Randomized
pruning, Python/WASM bindings, and ML model training remain out of scope
until a milestone explicitly calls for them (`AGENTS.md` section 3).
