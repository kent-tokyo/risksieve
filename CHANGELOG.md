# Changelog

All notable changes to `risksieve` are documented here. Format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

## [0.2.1] - 2026-08-07

### Added

- `categories` and `keywords` in `Cargo.toml`, for crates.io discovery
  (`science`, `algorithms`, `mathematics`; `conformal-prediction`,
  `risk-control`, `calibration`, `uncertainty`, `statistics`).
- `risksieve::selective::evalue::risk_adjusted_evalue`'s doctest now
  contrasts a test score above every value in `M` (e-value `0`, the case
  already shown) against one at or below every value in `M` (e-value
  `2.0`), so the score-orientation convention documented in `0.2.0`'s
  module docs has a runnable, checked example rather than only prose.

## [0.2.0] - 2026-08-07

### Changed

- `risksieve::selective::sdr::certify`'s default e-value construction
  changed from the independent, per-test-point construction (Equation 4.1)
  used in `0.1.0` to the paper's own cross-test-point-coupled construction
  described below (Equation 5.1, Theorem 5.1). **The function signature is
  unchanged, but selected sets can differ from `0.1.0`'s for the same
  inputs** — a semver gray zone that a mechanical compatibility check will
  not flag. Callers who need `0.1.0`'s exact selection behavior should
  call the new `certify_independent` (below) explicitly instead of
  `certify`. `certify` now: SCoRE-SDR (Algorithm 2) over a batch of test
  points, using the coupled construction below by default and composing
  it with `ebh::select`. Returns a `GuaranteeKind::SelectiveDeploymentRisk`
  certificate whose `parameter` is the sorted-ascending selected indices;
  an empty selection is a valid certificate (`SDR = 0 <= alpha` via the
  `1 v |R|` denominator), not an error. Records `Diagnostics::ebh_tau_hat`
  for auditability.

### Added

- `risksieve::selective::coupled::coupled_risk_adjusted_evalues`: the
  paper's own cross-test-point-coupled e-value (Bai and Jin (2026),
  Equation 5.1, Theorem 5.1, and Algorithm 3's efficient computation),
  independently derived from the equation rather than translated from
  the official `SCoRE_SDR` Python implementation, in
  `O((n+m) log(n+m) + m(n+m))`. Groups pooled calibration-and-test scores
  into distinct sorted values before computing any prefix sum (rather
  than sorting a tuple list with duplicates and correcting for ties
  afterward, as the official implementation does); sums a tied score
  group's calibration losses in a canonical `(score, loss)` order (via
  `f64::total_cmp`), not input order, so the result is bit-exact under
  any permutation of the calibration input; computes the `FR_0`/`FR_1`
  feasibility checks via cross-multiplied comparisons to avoid a
  division; and clamps the `ell_bar` breakpoint value to `[0, 1]` before
  evaluating the objective, matching Equation 5.1's stated domain exactly
  — a deliberate divergence from `SCoRE_SDR`, which does not clamp it
  (confirmed by a 50,000-trial comparison, reproducible via
  `scripts/audits/compare_score_reference.py`, to change no output).
  Accepts `gamma: OpenUnitInterval`, narrower than the paper's stated
  `gamma > 0` for this construction, for reasons recorded in the module
  docs and `docs/references.md` (oracle parity, API consistency with
  Equation 4.1's `gamma`, and a proof that `gamma = 0` is the only point
  where the e-value's true infimum can be `+infinity`).
- `risksieve::selective::sdr::certify_independent`: the same batch entry
  point using `evalue::risk_adjusted_evalue` (Equation 4.1) applied
  independently to each test point instead, ignoring the rest of the
  batch — this crate's only SDR construction before the coupled one
  existed, kept for comparison and backward compatibility, sharing
  certificate assembly with `certify` through a private helper. Neither
  construction is claimed to dominate the other in selection power;
  `docs/references.md` records a fixture where they disagree and one
  where they coincide by symmetry.
- `scripts/oracles/generate_score_sdr.py` and
  `tests/fixtures/score_sdr_v0_1_1.json`: a cross-language oracle fixture
  generated from `Tian-Bai/SCoRE`'s `SCoRE_SDR` (commit
  `401b7caf6d030825ff67e8f08e44ba15ee8c94af`, package version `0.1.1`),
  covering hand-computable, tied, shared-score, all-zero/all-one-loss,
  empty-batch, zero/all-selection, and coupled-vs-independent-disagreement
  cases plus 20 fixed-seed randomized cases (30 total). The generator
  verifies the referenced checkout's git HEAD, `SCoRE/SCoRE.py`'s blob
  SHA, and `SCoRE.__version__` against these pinned values before running
  (`scripts/score_provenance.py`), failing immediately on any mismatch
  with no override flag, and writes the actually-measured (not merely
  assumed) values into the fixture's provenance. No independent
  (Equation 4.1) oracle column: the official `SCoRE_MDR_bf` brute force
  was found to diverge from the true infimum in a nontrivial fraction of
  randomized trials (it only evaluates its objective at `l in {0,1}`,
  missing interior breakpoints), so it is not used as an oracle —
  reproducible via `scripts/audits/compare_score_reference.py`; see
  `docs/references.md` and `THIRD_PARTY_NOTICES.md`.
- `tests/paper_score_sdr.rs`: a hand-derived `m=3` eBH selection trace, a
  hand-derived `m=2` coupled e-value trace, an end-to-end
  `sdr::certify_independent` trace reusing `mdr.rs`'s own hand-computed
  e-value fixture across a batch, a fixture where the coupled and
  independent constructions select different sets, and the
  zero-selection/denominator checks named directly in AGENTS.md's
  Milestone 5 requirements.
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
  test-batch order invariance (including tied score groups, exercised
  with loss values not exactly representable in binary so a
  summation-order dependency would actually surface), e-value
  non-negativity, the `m=1` reduction to Equation 4.1 (proved
  algebraically in the module docs: with one test point, Equation 5.1's
  `1 + sum_{k != j}` term has an empty sum and collapses to the constant
  `1`), and `sdr::certify`'s selected-set / `tau_hat` invariance under
  permuting the test batch's submission order. No monotonicity/dominance
  property is asserted between the coupled and independent
  constructions, since neither the paper nor this crate proves one.

- `risksieve::selective::evalue_weighted::weighted_risk_adjusted_evalue`:
  the weighted risk-adjusted e-value construction from Bai and Jin (2026),
  Equation 6.1 (Section 6's covariate-shift extension), computed via the
  same breakpoint-enumeration approach as the unweighted construction but
  built as a fully independent implementation rather than a thin wrapper
  around it (weight `1` is not special-cased into the unweighted code
  path, to avoid silently coupling their rounding behavior). Calibration
  points are weighted individually (`w(X_i)`), not just the test point.
  Introduces `EValue` (`Finite(NonNegative)` / `PositiveInfinity`, defined
  in `certificate.rs` and re-exported at this module's path for backward
  compatibility, so `Diagnostics::risk_adjusted_evalue` can use it without
  a dependency from the foundational `certificate` module onto this
  Milestone-6-specific one): unlike Equation 4.1, whose infimum is
  provably always finite, Equation 6.1's can be `+infinity` when the test
  point's own contribution to the weighted denominator is zero while its
  numerator can still clear the deployment threshold — found as a
  concrete reproducer while generating the oracle fixture below, not a
  hypothetical case, and represented faithfully rather than clamped to a
  large finite value. Invariant to rescaling every weight (calibration
  and test) by the same positive constant; not invariant to non-uniform
  reweighting (verified by a proptest that also confirms the invariance
  property isn't vacuous). Every weight is normalized by their shared
  maximum before any other computation, so this invariance is not just a
  proven property but an actively-relied-upon numerical safeguard: finite
  but huge weights (for example both near `f64::MAX`) can otherwise
  overflow `total_weight` to `+infinity` and spuriously trigger the
  `EValue::PositiveInfinity` case for what is, once the shared scale
  cancels, a genuinely finite e-value.
- `risksieve::guarantee::WeightConsistencyEvidence` and
  `ThresholdRegularityEvidence`: typed, caller-declared evidence for
  Theorem 6.4's two population-level hypotheses (`L2(P_X)`-consistency of
  an estimated weight sequence; continuity and strict monotonicity of the
  paper's `F` at `t*`), added as fields of
  `ImportanceWeightSource::Estimated` alongside the existing `method` and
  `training_data_separate_from_calibration`. Neither is checkable by this
  crate from a single realized estimate — both follow the same
  caller-declared-evidence pattern as `SymmetryAssumption::CallerAsserted`
  and `StabilityEvidence::UserSupplied`.
- `risksieve::guarantee::ExchangeabilityAssumption::CovariateShiftIid`:
  calibration i.i.d. from `P`, test i.i.d. from a *different* distribution
  `Q`, related by a declared covariate-shift assumption (Bai and Jin 2026,
  Assumption 6.1; Hultberg, Zachariah, and Ribeiro 2026, Section 4.2) —
  distinct from the existing `Iid` variant, which asserts calibration and
  test are drawn from the *same* distribution and does not apply to
  either shifted setting.
- `risksieve::selective::mdr::certify_weighted`: the weighted extension of
  Algorithm 1 (SCoRE-MDR) for one test point under covariate shift, taking
  an explicit, never-defaulted `ImportanceWeightSource`. Produces
  `GuaranteeKind::MarginalDeploymentRisk` for `KnownDensityRatio` (Theorem
  6.2's finite-sample hypothesis). For `Estimated`, produces
  `GuaranteeKind::Asymptotic` **only** when *every one* of Theorem 6.4's
  four hypotheses is declared true —
  `training_data_separate_from_calibration`, `consistency`, and
  `threshold_regularity` all hold, *and* the caller passed `gamma == alpha`
  exactly (checked as exact `f64` equality, not an approximate comparison)
  — and downgrades to `GuaranteeKind::EmpiricalOnly` otherwise (a real
  deploy/abstain decision is still returned, just without a theorem
  attached to it — the same choice `nonmonotone::stability::certify` makes
  for `StabilityEvidence::Estimated`). `Assumptions::exchangeability` is
  `CovariateShiftIid`. Records the same calibration-only weight diagnostics
  (`weight_sum`, `weight_sum_of_squares`, `effective_sample_size`,
  `weight_range`) as `AnytimeShiftedController`, plus a new
  `Diagnostics::test_weight` recording the test point's own weight
  separately, since it is not folded into those calibration-only
  statistics but does enter Equation 6.1's shared normalizing constant.
  Single test point per call, matching Equation 6.1's own shape, rather
  than a batch API (the batch/eBH-selection extension is weighted SDR,
  out of scope here — see `docs/roadmap.md`).
- `scripts/oracles/generate_score_mdr_w.py` and
  `tests/fixtures/score_mdr_w_v0_1_1.json`: a cross-language oracle
  fixture generated against `Tian-Bai/SCoRE` (commit
  `401b7caf6d030825ff67e8f08e44ba15ee8c94af`, package version `0.1.1`),
  covering all-weights-1, calibration-only/test-only/both non-uniform,
  zero-weight, score/weight ties, all-zero/all-one-loss, empty-batch,
  zero/all-selection, extreme weight ratio, uniform weight rescale, an
  overflow-adversarial case (weights near `f64::MAX`), and three
  `gamma > alpha` cases (one ordinary, one where the official shortcut's
  overlap condition flips its naive decision, one where it doesn't), plus
  20 fixed-seed randomized cases split between `gamma <= alpha` and
  `gamma > alpha` (38 total). Unlike the SDR oracle, this one makes two
  independent comparisons per case rather than one, since the official
  package has no weighted e-value function of its own: the e-value itself
  is checked against the fixture generator's own from-scratch Python
  reference implementation of Equation 6.1, and the deploy/abstain
  decision is checked against the official `SCoRE_MDR_w` shortcut,
  exactly, for *every* case regardless of `gamma` vs `alpha` — this
  crate's own construction never takes the shortcut, so its decision does
  not need the shortcut's extra `gamma > alpha` overlap condition to
  already agree with it (confirmed by a 300,000-trial randomized search,
  not assumed). Reuses `scripts/score_provenance.py`'s fail-fast checkout
  verification. See `docs/references.md`'s "Equation 6.1 audit" for the
  full correspondence; no formula discrepancy was found against either
  comparison target.
- `scripts/audits/compare_score_mdr_w.py`: makes the 300,000-trial search
  above independently reproducible (fixed seed `20260730`, CLI-configurable
  trial count), rather than resting on an uncommitted one-off search.
  Tallies `gamma > alpha` trials, how many have the official shortcut's
  naive (pre-overlap-check) decision as "deploy", and how many of those
  the overlap condition flips to "abstain" -- descriptive counts only, not
  part of the pass/fail verdict, which is the official-vs-reference
  decision mismatch count (must be zero). Follows the same pattern as
  `scripts/audits/compare_score_reference.py`: reuses
  `scripts/score_provenance.py`'s fail-fast checkout verification, and
  exits non-zero on any mismatch, printing the first full reproducer
  (inputs, both decisions, the reference e-value) if one is found. Not
  part of `cargo test` or CI.
- `tests/score_mdr_w_oracle.rs`: reads the fixture above (Python is never
  invoked by `cargo test`); all 38 cases (109 test points) match the
  reference e-values, and every case's official decision matches exactly.
- `tests/statistical_validity_weighted_mdr.rs`: a Monte Carlo check for
  Theorem 6.2, with a deliberately non-vacuous data-generating process — a
  decoupled score coordinate (identical under calibration and test) and
  risk coordinate (`Uniform(0,1)` under calibration, density `2x` under
  test, known density ratio `w(x) = 2x`), plus a `naive` arm applying
  plain unweighted `certify` to the same shifted test point to confirm
  the DGP is genuinely discriminating (it violates `alpha`, by a wide
  margin, at both repetition counts) rather than passing vacuously. A
  fast 500-repetition smoke test runs in normal CI; a 20,000-repetition
  version is `#[ignore]`d. Deliberately does not exercise
  `ImportanceWeightSource::Estimated`, since a Monte Carlo pass there
  would only bear on Theorem 6.4's asymptotic conclusion, not a
  finite-sample one.
- `docs/roadmap.md`: the tracked, publishable backlog that README,
  rustdoc, and `docs/validation.md` now point to, replacing the local-only
  `tasks/todo.md` (which is real and still exists locally, but was never
  committed — `AGENTS.md` is now tracked and un-ignored for the same
  reason: a contributor cloning this repository from GitHub had no way to
  read either file before this change).

### Fixed

- `risksieve::selective::evalue::risk_adjusted_evalue`: tied calibration
  scores were grouped via a stable sort keyed on score alone, leaving a
  tied group's loss-summation order equal to caller input order; two
  calls with the same `(score, loss)` multiset but a different input
  order could therefore land on a different floating-point rounding for
  loss values not exactly representable in binary (confirmed to produce
  different bit patterns, not just different rounding within tolerance,
  for a constructed adversarial input). This contradicts the permutation
  invariance this function's own property test
  (`construction_is_permutation_invariant`) already covered with a
  powers-of-two-friendly loss alphabet that happened to mask the bug.
  Found while building the weighted e-value construction below, whose
  property tests reuse the same "weight = 1 matches unweighted" check
  across adversarial loss values and would otherwise have been comparing
  against a non-deterministic reference. Fixed by sorting calibration
  entries by `(score, loss)` before grouping (the same fix applied to
  `selective::coupled::coupled_risk_adjusted_evalues` previously), so the
  grouped sum depends only on the multiset of loss values, never on input
  order; added a regression test with the same adversarial loss alphabet
  used there. No public API changed; only the exact floating-point result
  for calibration sets with ties at an identical score and specific
  non-representable loss values can differ from before this fix (and only
  by ULPs).
- `risksieve::selective::mdr::certify_weighted` was returning
  `GuaranteeKind::Asymptotic` for *any* `ImportanceWeightSource::Estimated`,
  regardless of whether Theorem 6.4's actual hypotheses held (weight
  estimator trained independent of calibration, `L2(P_X)`-consistent,
  Theorem 6.4's threshold-regularity condition, and `gamma == alpha`) —
  overstating the guarantee for a caller who declared `Estimated` without
  meeting all four. Fixed by extending `Estimated` with typed
  `consistency`/`threshold_regularity` evidence fields and only returning
  `Asymptotic` when every hypothesis is declared true, downgrading to
  `EmpiricalOnly` otherwise. Verified against the paper's exact wording
  (re-fetched arXiv:2603.24704, Theorem 6.4) before implementing, not
  inferred from an earlier paraphrase.
- `risksieve::anytime::AnytimeShiftedController::update` was also
  returning `GuaranteeKind::Asymptotic` for any `Estimated` weight source,
  but Hultberg, Zachariah, and Ribeiro (2026), Theorem 4.7 (re-fetched and
  read in full to confirm) never discusses estimated weights at all — it
  takes the importance weight as a standing known hypothesis, not
  something the theorem relaxes — so there was no asymptotic argument
  backing that claim in the first place. Fixed to downgrade every
  `Estimated` case there to `EmpiricalOnly` unconditionally.
- Both `certify_weighted` and `AnytimeShiftedController::update` were
  recording `Assumptions::exchangeability` as `ExchangeabilityAssumption::Iid`,
  which asserts calibration and test are drawn from the *same*
  distribution — the wrong claim for a covariate-shift setting, where
  they are drawn from two different distributions `P` and `Q`. Fixed by
  adding `ExchangeabilityAssumption::CovariateShiftIid` and using it in
  both places instead.
- `weighted_risk_adjusted_evalue` computed `total_weight` and every
  weighted loss at the caller's raw weight scale, so finite-but-huge
  weights (for example both near `f64::MAX`) could overflow to
  `+infinity`, spuriously producing `EValue::PositiveInfinity` for what
  is, once the shared weight scale is factored out, a genuinely finite
  e-value (confirmed with an exact `f64::MAX`/`f64::MAX` regression case
  whose true e-value is `2.0`). Fixed by normalizing every weight
  (calibration and test alike) by their shared maximum before any other
  computation, exploiting the construction's own proven uniform-scale
  invariance. This also exposed a second, latent bug: the normalized
  weight sum was accumulated in caller-supplied order rather than a
  canonical sorted order, so it was never truly permutation-invariant (it
  only "got lucky" with small-integer test weights before; the extra
  rounding from normalization was enough to surface a 1-ULP mismatch).
  Fixed with the same canonical-order-before-summing pattern already used
  for tied score groups.
- `Diagnostics::risk_adjusted_evalue` was a plain `Option<f64>`, so a
  `+infinity` weighted e-value round-tripped through `serde_json` as
  `null` — indistinguishable from "not computed". Fixed by retyping it to
  `Option<EValue>`, which round-trips `Finite`, `PositiveInfinity`, and
  `None` distinctly, using ordinary tagged-enum JSON (no reliance on a
  non-standard bare `Infinity` token).
- The weighted-MDR oracle's own from-scratch Python reference
  implementation (`weighted_evalue_reference` in
  `scripts/oracles/generate_score_mdr_w.py`) had no epsilon tolerance on
  its feasibility comparison, so a breakpoint computed to satisfy
  `F(t;l) <= gamma` with exact equality could land a few ULPs past
  `gamma_scaled` after floating-point rounding and be incorrectly
  rejected, silently missing the true infimum. Found while building
  targeted `gamma > alpha` oracle fixtures (a 200,000-trial randomized
  search surfaced a disagreement between this reference and the official
  `SCoRE_MDR_w` decision); confirmed by hand-deriving the true e-value for
  the exact failing case and finding it matched the Rust implementation
  and the official decision, not the buggy reference — so the bug was in
  the reference script, not in `weighted_risk_adjusted_evalue` or in
  `SCoRE_MDR_w`. Fixed with the same epsilon-tolerance pattern
  (`feasibility_epsilon`) already used in `evalue.rs` and
  `evalue_weighted.rs`; a 300,000-trial re-check after the fix found zero
  further mismatches.

### Not yet implemented

Milestone 7 in AGENTS.md section 7 (downstream examples) is still open.
Within Milestone 2, Proposition 4.5 (asymptotic tightness diagnostics) is
not implemented. Within Milestone 3, everything beyond Theorem 1 itself
(Propositions 2-8, Theorem 2) is not implemented. Within Milestone 4,
Theorem 4.6 (the extra thresholding condition for `gamma > alpha`) is not
implemented, and Proposition 4.4's shortcut is verified but not wired in
as a separate code path. Within Milestone 5, randomized pruning (the
official implementation's optional `prune='hete'` / `'homo'` power boost)
and weighted SDR are not implemented. Within Milestone 6, weighted SDR
(`SCoRE_SDR_w`) is not implemented — the recommended next PR, composing
`selective::evalue_weighted` with `selective::coupled`'s grouped-threshold
representation the way `selective::sdr` composes the unweighted e-value.
Remark 6.6's doubly-robust refinement (weighted MDR with only one of the
weight or risk model consistent) is also not implemented, deferred to an
appendix the paper does not detail. See `docs/roadmap.md` for the
complete, maintained backlog.

## [0.1.0] - 2026-07-27

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

### Not yet implemented

Milestone 7 in AGENTS.md section 7 (downstream examples) is still open.
Within Milestone 2, Proposition 4.5 (asymptotic tightness diagnostics) is
not implemented. Within Milestone 3, everything beyond Theorem 1 itself
(Propositions 2-8, Theorem 2) is not implemented. Within Milestone 4,
Theorem 4.6 (the extra thresholding condition for `gamma > alpha`) is not
implemented, and Proposition 4.4's shortcut is verified but not wired in
as a separate code path. Within Milestone 5, Equation 5.1 and Algorithm 3
(the paper's own cross-test-point-coupled e-value and its efficient
computation) are not implemented. Within Milestone 6, weighted SCoRE
(weighted MDR/SDR) is not implemented — deferred in favor of the shifted
anytime controller, per AGENTS.md's backlog ordering (item 18 before
item 19).
