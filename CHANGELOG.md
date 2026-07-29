# Changelog

All notable changes to `risksieve` are documented here. Format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added

- Project skeleton: `Cargo.toml`, dual `MIT OR Apache-2.0` licensing,
  CI (`cargo fmt`, `clippy -D warnings`, `test`, `doc`, `cargo-deny`).
- Validated numeric types `OpenUnitInterval`, `ClosedUnitInterval`,
  `NonNegative`, and `ClosedInterval`, rejecting NaN, infinity,
  out-of-range values, and negative zero; deterministic total order via
  `f64::total_cmp`.
- `BoundedLoss` trait with a checked-evaluation default method, plus
  `ZeroOneLoss` and `AbsoluteErrorLoss` built-ins.
- `GuaranteeKind`, `Assumptions`, and the constituent assumption enums
  (`ExchangeabilityAssumption`, `MonotonicityAssumption`,
  `SymmetryAssumption`, `ShiftAssumption`, `StabilityEvidence`,
  `StabilityEstimationMethod`, `ImportanceWeightSource`).
- `RiskCertificate<Parameter>` and `Diagnostics` output types.
- `RiskSieveError` taxonomy (adds `NegativeValue` beyond the variants
  listed in AGENTS.md section 13, to distinguish a negative `NonNegative`
  input from an out-of-unit-interval probability).
- Optional `serde` feature with hand-written `Serialize`/`Deserialize` for
  every validated type, so deserialization re-runs the same validation as
  construction rather than bypassing it.
- Reference documentation: `docs/guarantees.md`, `docs/assumptions.md`,
  `docs/references.md`, `docs/validation.md`.

- `risksieve::crc::monotone::certify`: the classical monotone CRC
  baseline from Angelopoulos, Bates, Fisch, Lei, and Schuster (2024),
  Theorem 1. Scans a caller-supplied ascending candidate grid for the
  smallest parameter whose finite-sample-corrected empirical risk is at
  most `alpha`; returns `RiskSieveError::NoFeasibleParameter` if none
  qualifies, and flags a `Diagnostics::uninformative_result` when the
  certified parameter is the most conservative candidate in the grid.
  Requires a loss bounded on `[0, B]` (rejects a nonzero lower bound,
  matching the theorem's stated assumption exactly rather than silently
  generalizing it).
- `risksieve::numerics::summation::{kahan_sum, kahan_mean}`: compensated
  summation for accumulated losses (AGENTS.md section 8), used by the
  monotone CRC controller and available for later controllers.
- `tests/paper_crc.rs`: paper-traceable tests against a hand-computed
  exceedance-loss example.

- `risksieve::anytime::AnytimeController`: anytime-valid monotone CRC
  from Hultberg, Zachariah, and Ribeiro (2026), Theorem 4.1 and
  Definition 2.7. Accumulates a bounded monotone loss incrementally over
  a growing calibration stream via `update`; below the minimum eligible
  calibration size `m*` it returns the paper's designated uninformative
  fallback rather than an error, and the deployed parameter across
  updates is kept non-increasing via a running minimum ("The threshold
  sequence will be non-increasing and, when necessary, we use a running
  minimum."), reported through `Diagnostics::running_minimum_applied`.
  This paper postdates this project's training-data cutoff, so the
  theorem statement was extracted from three independent fetches of the
  paper's own text (not recalled from training) before implementing; see
  `docs/references.md` for the provenance note.
- `risksieve::anytime::boundary`: the `m*` search and correction-term
  functions from Theorem 4.1, unit-tested against an independently
  computed Python fixture at the paper's own Section 6.1 simulation
  parameters (`alpha = 5%`, `delta = 10%`, giving `m* = 325`).
- `tests/paper_anytime.rs` and a `proptest` property test covering the
  non-increasing threshold-sequence invariant named in AGENTS.md section
  9.3, exercised against randomized alpha, delta, and observation
  streams rather than only the hand-picked fixture.

- `risksieve::nonmonotone::stability::certify`: the general symmetry +
  beta-stability risk-control reduction from Angelopoulos (2026), Theorem
  1. Unlike the other two controllers, this does not search for a
  parameter itself — the caller supplies a parameter already produced by
  their own algorithm, together with a symmetry declaration and
  `StabilityEvidence`, and this function checks Theorem 1's hypothesis
  (`reference_risk_bound <= target_risk - beta`) and assembles the
  certificate. `StabilityEvidence::Unknown` is rejected outright;
  `StabilityEvidence::Estimated` downgrades the guarantee to
  `GuaranteeKind::EmpiricalOnly` rather than claiming an exact bound.
  Because `Parameter` carries no trait bounds, multidimensional
  parameters are already supported. This paper also postdates the
  training-data cutoff; Theorem 1's statement was cross-checked across
  independent fetches and is an exact (non-asymptotic) result, unlike the
  paper's Proposition 2 (bounded-loss discretization), which was deferred
  because its stated guarantee is an `Õ(1/√n)` Lambert-W-function bound
  extracted with lower confidence — see `docs/roadmap.md`.
- `Diagnostics::asserted_reference_bound`: records the caller-asserted
  reference-algorithm risk bound that `nonmonotone::stability::certify`'s
  Theorem 1 hypothesis depends on, so it remains auditable from the
  certificate rather than being consumed internally and discarded
  (AGENTS.md section 16).
- `tests/paper_nonmonotone.rs`: checks Theorem 1's exact hypothesis
  boundary. An earlier draft also tried to check Proposition 1 (monotone
  CRC is the `beta = 0` special case) by feeding
  `crc::monotone::certify`'s output back through `certify`, but that only
  asserted `certify`'s passthrough fields against themselves — a
  tautology, not a check of the theorem — so it was removed before
  release rather than merged as a misleadingly named test. Proposition 1
  is now validated for real: against a reference algorithm `A*` this
  crate chose itself (the uncorrected oracle threshold on the full
  `(n+1)`-point dataset, since the paper never names `A*` precisely
  enough to transcribe), `certify`'s leave-one-out threshold on any
  held-out point provably never falls below `A*`'s, an exact per-dataset
  domination rather than a Monte Carlo estimate. A first pass pinned a
  non-vacuity count at the threshold level but not the risk level; with
  the 0/1 `ExceedsThreshold` loss, the risk-level statement turns out to
  collapse to an exact equality on *every* held-out point (a genuine
  structural fact of that loss, proved in the test's doc comment — no
  choice of sample size or grid changes it), so a second,
  continuously-varying `RampLoss` was added to exercise the risk-level
  inequality as strict (pinned non-zero, independently recomputed in
  Python). Both losses are checked on a fixed sequence and fuzzed via
  `proptest`.

- `risksieve::selective::evalue::risk_adjusted_evalue`: the risk-adjusted
  e-value construction from Bai and Jin (2026), Definition 3.1 and
  Equation 4.1. Computes the exact infimum over `l in [0,1]` via a
  breakpoint-enumeration algorithm derived from the construction's
  monotonicity structure (documented in the module docs), rather than a
  numerical search; verified against two independently hand-derived
  fixtures and a permutation-invariance property test. This paper
  postdates this project's training-data cutoff; see the module's
  provenance note for how Definition 3.1, Equation 4.1/4.2, Theorem 4.2,
  and Remark 4.5 were cross-checked.
- `risksieve::selective::mdr::certify`: SCoRE-MDR (Algorithm 1), the
  direct deploy/abstain decision from thresholding a risk-adjusted
  e-value at `1/alpha` (Theorem 3.2), with an explicit `gamma` parameter
  (never silently defaulted to `alpha`). Returns a
  `GuaranteeKind::MarginalDeploymentRisk` certificate; its module docs
  are explicit that the certified `E[loss * deploy] <= alpha` bound is
  marginal over the joint draw, not a property of any single returned
  decision. Records `Diagnostics::risk_adjusted_evalue` and
  `Diagnostics::gamma` for auditability, and flags
  `Diagnostics::uninformative_result` when no threshold is feasible at
  the requested `gamma` (distinct from a genuinely small e-value).
  Proposition 4.4's efficient shortcut (valid for `gamma <= alpha`) is
  verified against the general computation by a property test but not
  wired in as a separate code path (AGENTS.md's Milestone 4 requirement
  to build the transparent reference first); see `docs/roadmap.md`.
- `tests/paper_score_mdr.rs`: two hand-derived `n=1` fixtures worked
  through step by step in comments, plus a test distinguishing "no
  feasible threshold" from a genuinely informative zero e-value.

- `risksieve::selective::ebh::select`: generic eBH selection (Bai and Jin
  2026, Theorem 3.3), deliberately decoupled from any specific e-value
  construction — its only requirement is that each input e-value
  individually satisfies Definition 3.1. Sorts descending via
  `f64::total_cmp` (stable, so ties keep ascending-index order — a
  documented implementation choice, since the paper does not specify
  tie-breaking), scans every candidate `tau` rather than stopping at the
  first failure (the qualifying condition is not guaranteed monotone in
  `tau` for an arbitrary e-value multiset), and selects by comparing each
  e-value directly against the resulting threshold rather than taking a
  "top-`tau_hat`" slice of the sorted list (those differ exactly at
  ties). An empty input returns an empty, valid selection rather than an
  error.
- `risksieve::selective::sdr::certify`: SCoRE-SDR (Algorithm 2) over a
  batch of test points, composing `ebh::select` with Milestone 4's
  `evalue::risk_adjusted_evalue` applied independently to each test
  point. This is a deliberate scope reduction from the paper's own
  per-test-point e-value (Equation 5.1), which couples each test point's
  threshold search to every other test point in the batch via a
  normalizing function that is a ratio of two non-decreasing-in-`t`
  quantities — unlike Equation 4.1's fixed-denominator sum, this ratio is
  not obviously monotone in `t`, so the breakpoint-enumeration approach
  `evalue.rs` uses does not straightforwardly extend, and the paper's own
  "Algorithm 3" (efficient computation for Equation 5.1) was not
  extractable in enough detail across several independent, targeted
  fetches to implement with confidence. Reusing Milestone 4's
  construction per test point remains a fully valid instantiation of
  Theorem 3.3 (whose hypothesis only requires each e-value to
  individually satisfy Definition 3.1) — what is given up is selection
  power, not correctness; see `docs/roadmap.md`. Returns a
  `GuaranteeKind::SelectiveDeploymentRisk` certificate whose `parameter`
  is the sorted-ascending selected indices; an empty selection is a valid
  certificate (`SDR = 0 <= alpha` via the `1 v |R|` denominator), not an
  error. Records `Diagnostics::ebh_tau_hat` for auditability.
- `risksieve::selective::sdr::realized_selective_risk`: the post-hoc,
  label-requiring realized selective risk, returning a plain `f64` (never
  a certificate) so it cannot be mistaken for validating the guarantee
  that produced the selected set. Uses the `max(1, selected_count)`
  denominator exactly (AGENTS.md: "Never replace the denominator
  `max(1, selected_count)` with a different convention"), verified at
  `|R| = 0` to be `0.0`, never `NaN`.
- `Diagnostics::ebh_tau_hat`: records the eBH critical value the
  selection hinged on, for the same auditability reason as
  `asserted_reference_bound` and `gamma`.
- `tests/paper_score_sdr.rs`: a hand-derived `m=3` eBH selection trace, an
  end-to-end `sdr::certify` trace reusing `mdr.rs`'s own hand-computed
  e-value fixture across a batch, and the zero-selection/denominator
  checks named directly in AGENTS.md's Milestone 5 requirements.

- `risksieve::shift::importance::WeightAccumulator`: non-negative finite
  importance-weight validation (delegated to `NonNegative`) plus
  Kahan-compensated running sum, sum of squares, min, max, and Kish
  effective sample size, folded in incrementally one weight at a time.
  Rejects an all-zero or empty weight sequence via
  `RiskSieveError::DegenerateWeights`, checked against the *current*
  accumulated state rather than latched permanently, so a zero-weight
  observation followed by a positive one is not degenerate once the
  positive weight arrives.
- `risksieve::anytime::boundary::weighted_term`: the boundary function
  Theorem 4.7's correction term actually uses. Every independent fetch of
  the paper's text read this as `h_{B,m,delta}` (the plain log-log term);
  a dimensional argument (that reading would make the shift-corrected
  bound decay *faster* than the unweighted `O(1/sqrt(n))` rate it
  generalizes, which is impossible) shows this is a shared misreading,
  confirmed numerically by comparing against the unweighted correction at
  constant weights. `weighted_term` is `f`'s square-root term without the
  linear `2.42 * B * h` term, matching what every fetch consistently read
  for the (separate) `m*`-defining condition — restoring the same
  function being used in both places, as in the unweighted Theorem 4.1.
- `risksieve::anytime::AnytimeShiftedController`: importance-weighted
  anytime-valid CRC (Theorem 4.7). Unlike the unweighted controller, `m*`
  depends on the realized weights and so cannot be precomputed at
  `build()` time; it is instead discovered at runtime as a stopping time
  (frozen the first time its defining condition holds on the weights seen
  so far) and reused as a fixed reference for every later step, exactly
  as Theorem 4.1's `m*` is. The same running-minimum argument from
  `anytime::calibration` still applies. `weight_source` is a required
  builder field (never defaulted): `ImportanceWeightSource::KnownDensityRatio`
  yields `GuaranteeKind::AnytimeHighProbability` (Theorem 4.7's actual
  hypothesis); `Estimated` yields `GuaranteeKind::Asymptotic` instead,
  since the paper establishes no finite-sample guarantee for estimated
  weights.
- `Diagnostics::weight_sum` and `Diagnostics::weight_sum_of_squares`:
  complete the five weight diagnostics AGENTS.md's Milestone 6 requires
  (`effective_sample_size` and `weight_range` already existed).
- `tests/paper_anytime_shifted.rs`: constant-weight exactness checks
  (bias term is exactly `0.0`, `W_n` and Kish ESS are exactly `n`), a
  permanent regression guard asserting the weighted correction always
  exceeds the unweighted one at matching `n` (the same argument that
  ruled out the fetched `h`-reading, kept in the suite rather than living
  only in a one-time derivation), an `Estimated`-weights-yield-`Asymptotic`
  check, and a property test for the non-increasing threshold sequence
  under randomized weights.

- `risksieve::selective::coupled::coupled_risk_adjusted_evalues`: the
  paper's own cross-test-point-coupled e-value (Bai and Jin (2026),
  Equation 5.1, Theorem 5.1, and Algorithm 3's efficient computation),
  independently derived from the equation rather than translated from
  the official `SCoRE_SDR` Python implementation, in
  `O((n+m) log(n+m) + m(n+m))`. Groups pooled calibration-and-test scores
  into distinct sorted values before computing any prefix sum (rather
  than sorting a tuple list with duplicates and correcting for ties
  afterward, as the official implementation does); computes the
  `FR_0`/`FR_1` feasibility checks via cross-multiplied comparisons to
  avoid a division; and clamps the `ell_bar` breakpoint value to `[0, 1]`
  before evaluating the objective, matching Equation 5.1's stated domain
  exactly — a deliberate divergence from `SCoRE_SDR`, which does not
  clamp it (confirmed by a 50,000-trial comparison to change no output).
  Accepts `gamma: OpenUnitInterval`, narrower than the paper's stated
  `gamma > 0` for this construction, for reasons recorded in the module
  docs and `docs/references.md` (oracle parity, API consistency with
  Equation 4.1's `gamma`, and a proof that `gamma = 0` is the only point
  where the e-value's true infimum can be `+infinity`).
- `risksieve::selective::sdr::certify` now uses this coupled construction
  by default; the earlier per-test-point-independent composition (Equation
  4.1 applied to each test point on its own, ignoring the rest of the
  batch) is preserved as `risksieve::selective::sdr::certify_independent`,
  sharing certificate assembly with `certify` through a private helper.
  Neither construction is claimed to dominate the other in selection
  power; `docs/references.md` records a fixture where they disagree and
  one where they coincide by symmetry.
- `scripts/oracles/generate_score_sdr.py` and
  `tests/fixtures/score_sdr_v0_1_1.json`: a cross-language oracle fixture
  generated from `Tian-Bai/SCoRE`'s `SCoRE_SDR` (commit
  `401b7caf6d030825ff67e8f08e44ba15ee8c94af`, package version `0.1.1`),
  covering hand-computable, tied, shared-score, all-zero/all-one-loss,
  empty-batch, zero/all-selection, and coupled-vs-independent-disagreement
  cases plus 20 fixed-seed randomized cases (30 total). No independent
  (Equation 4.1) oracle column: the official `SCoRE_MDR_bf` brute force
  was found to diverge from the true infimum in ~28% of a 5,000-trial
  comparison (it only evaluates its objective at `l in {0,1}`, missing
  interior breakpoints), so it is not used as an oracle — see
  `docs/references.md` and `THIRD_PARTY_NOTICES.md`.
- `tests/score_sdr_oracle.rs`: opens this crate's tier 5 (cross-language
  oracle tests). Reads the fixture above (Python is never invoked by
  `cargo test`) and checks e-values with a combined absolute/relative
  tolerance and selected indices / `tau_hat` exactly.
- `tests/statistical_validity.rs`: opens this crate's tier 4 (statistical
  validity tests). A simple, auditable exchangeable data-generating
  process (i.i.d. `(U_i, L_i)` with `U_i ~ Uniform(0,1)` as the score and
  `L_i | U_i ~ Bernoulli(U_i)` as the loss, so any calibration/test split
  is exchangeable by construction), a hand-rolled SplitMix64 RNG (no new
  dependency, per AGENTS.md section 12 and this feature's explicit
  scope), and a Hoeffding-bound acceptance check
  (`observed_mean_sdr <= alpha + half_width`) rather than a naive
  `observed <= alpha` assertion. A fast 500-repetition smoke test runs in
  normal CI; a 20,000-repetition version is `#[ignore]`d. Checks both
  `sdr::certify` and `sdr::certify_independent`.
- New property tests for the coupled construction: calibration and
  test-batch order invariance (including tied score groups, since
  grouping sorts before any arithmetic), e-value non-negativity, the
  `m=1` reduction to Equation 4.1 (proved algebraically in the module
  docs: with one test point, Equation 5.1's `1 + sum_{k != j}` term has an
  empty sum and collapses to the constant `1`), and
  `sdr::certify`'s selected-set / `tau_hat` invariance under permuting the
  test batch's submission order. No monotonicity/dominance property is
  asserted between the coupled and independent constructions, since
  neither the paper nor this crate proves one.
- `docs/roadmap.md`: the tracked, publishable backlog that README,
  rustdoc, and `docs/validation.md` now point to, replacing the local-only
  `tasks/todo.md` (which is real and still exists locally, but was never
  committed — `AGENTS.md` is now tracked and un-ignored for the same
  reason: a contributor cloning this repository from GitHub had no way to
  read either file before this change).

### Not yet implemented

Milestone 7 in AGENTS.md section 7 (downstream examples) is still open.
Within Milestone 2, Proposition 4.5 (asymptotic tightness diagnostics) is
not implemented. Within Milestone 3, everything beyond Theorem 1 itself
(Propositions 2-8, Theorem 2) is not implemented. Within Milestone 4,
Theorem 4.6 (the extra thresholding condition for `gamma > alpha`) is not
implemented, and Proposition 4.4's shortcut is verified but not wired in
as a separate code path. Within Milestone 5, randomized pruning (the
official implementation's optional `prune='hete'` / `'homo'` power boost)
and weighted SDR are not implemented. Within Milestone 6, weighted SCoRE
(weighted MDR/SDR) is not implemented — deferred in favor of the shifted
anytime controller and the paper-exact coupled SDR construction, per
AGENTS.md's backlog ordering. See `docs/roadmap.md` for the complete,
maintained backlog.
