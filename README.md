# risksieve

*[日本語](README_ja.md)*

A Rust library that sieves predictions and decisions through finite-sample
and anytime-valid conformal risk guarantees.

## Status

**Milestone 0 (vocabulary), Milestone 1 (classical monotone CRC),
Milestone 2 (anytime-valid monotone CRC), Milestone 3 (non-monotonic CRC,
partial), Milestone 4 (SCoRE-MDR, partial), Milestone 5 (SCoRE-SDR), and
Milestone 6 (distribution shift, partial) are done.** Milestone 5 does not
yet include the paper's optional randomized-pruning boost (`prune='hete'`
/ `'homo'` in the official implementation) or weighted SDR; Milestone 6
covers importance-weighted anytime-valid CRC and weighted SCoRE-MDR, but
not yet weighted SDR (`SCoRE_SDR_w`) — see the Milestone 5 and 6
paragraphs below and `docs/roadmap.md`. Milestone 7 (downstream examples)
is not implemented yet.

Milestone 0 provides:

- validated numeric types for probability-like values (`OpenUnitInterval`,
  `ClosedUnitInterval`, `NonNegative`, `ClosedInterval`);
- the `BoundedLoss` contract, checked at evaluation time, with
  `ZeroOneLoss` and `AbsoluteErrorLoss` built-ins;
- the `GuaranteeKind` / `Assumptions` taxonomy that every future
  certificate will use;
- the `RiskCertificate` / `Diagnostics` output type;
- the `RiskSieveError` taxonomy.

Milestone 1 adds `risksieve::crc::monotone::certify`, the fixed-sample
expected-risk controller for bounded monotone losses (Angelopoulos,
Bates, Fisch, Lei, and Schuster (2024), Theorem 1), plus the compensated
summation it relies on (`risksieve::numerics::summation`).

Milestone 2 adds `risksieve::anytime::AnytimeController`, which folds in
calibration observations one at a time and returns an updated certificate
after each one (Hultberg, Zachariah, and Ribeiro (2026), Theorem 4.1 and
Definition 2.7). Below the minimum eligible calibration size it returns
the paper's designated uninformative result rather than an error, and the
deployed parameter across updates never increases, via a running minimum.

Milestone 3 adds `risksieve::nonmonotone::stability::certify`, the general
symmetry + beta-stability reduction for non-monotonic, multidimensional
losses (Angelopoulos (2026), Theorem 1). Unlike the other two controllers
this one does not search for a parameter itself: the caller supplies a
parameter their own algorithm already produced, plus a symmetry
declaration and stability evidence, and the function checks Theorem 1's
hypothesis and certifies it. Only Theorem 1 is implemented; the paper's
concrete stability instances (discretized losses, Lipschitz losses,
selective classification, regularized ERM) are tracked in `docs/roadmap.md`.

Milestone 4 adds `risksieve::selective::evalue::risk_adjusted_evalue` and
`risksieve::selective::mdr::certify`, the SCoRE-MDR direct deployment
decision (Bai and Jin (2026), Definition 3.1, Equation 4.1, Algorithm 1,
Theorem 3.2). Unlike the search-based controllers, this makes a single
deploy/abstain decision from a risk-adjusted e-value; the resulting
`E[loss * deploy] <= alpha` bound is marginal over the joint draw, not a
property of any one realized decision.

Milestone 5 adds `risksieve::selective::sdr::certify`, batch SCoRE-SDR
(Bai and Jin (2026), Algorithm 2, Theorem 3.3) using the paper's own
cross-test-point-coupled e-value (Equation 5.1, Theorem 5.1, via the new
`risksieve::selective::coupled` module), built from a generic eBH
selection engine (`risksieve::selective::ebh::select`). The earlier
composition — Milestone 4's e-value construction applied independently to
each batch item, ignoring every other test point — remains available as
`risksieve::selective::sdr::certify_independent`: still a fully valid
instantiation of Theorem 3.3 (whose hypothesis only requires each e-value
to individually satisfy Definition 3.1), kept for comparison and backward
compatibility. The two constructions do not always select the same set —
`docs/references.md` records a fixture where they differ and one where
they coincide by symmetry — and neither the paper nor this crate proves
one dominates the other in general. The coupled construction is
cross-checked against `Tian-Bai/SCoRE`'s own `SCoRE_SDR` (30 fixture
cases, `tests/score_sdr_oracle.rs`) and against a Monte Carlo simulation
of the SDR guarantee itself (`tests/statistical_validity.rs`, opening this
crate's tier 4). An empty selected set is a valid certificate, not an
error; `risksieve::selective::sdr::realized_selective_risk` computes the
post-hoc realized risk once labels arrive, returning a plain number rather
than a certificate so it can't be mistaken for the guarantee itself.
Randomized pruning (an optional power boost in the official
implementation) and weighted SDR remain unimplemented; see
`docs/roadmap.md`.

Milestone 6 adds `risksieve::anytime::AnytimeShiftedController`,
importance-weighted anytime-valid CRC under covariate shift (Hultberg,
Zachariah, and Ribeiro (2026), Theorem 4.7), plus
`risksieve::shift::importance::WeightAccumulator` for non-negative
finite weight validation and diagnostics (sum, sum of squares, effective
sample size, min, max) shared by any future weighted controller. `m*`
here cannot be precomputed the way Milestone 2's is, since Theorem 4.7's
`m*`-defining condition depends on the realized weights: it is instead
discovered at runtime as a stopping time, frozen the first time its
condition holds. `weight_source` is a required, never-defaulted field —
`KnownDensityRatio` gives the full finite-sample guarantee, `Estimated`
downgrades to an empirical-only diagnostic unconditionally, since the
paper never discusses estimated weights at all (it takes the importance
weight as a standing known hypothesis, not something the theorem
relaxes) — there is no asymptotic argument here to fall back on, unlike
weighted MDR's `Estimated` case below.

Milestone 6 also adds `risksieve::selective::evalue_weighted::weighted_risk_adjusted_evalue`
and `risksieve::selective::mdr::certify_weighted`, weighted SCoRE-MDR
under covariate shift (Bai and Jin (2026), Equation 6.1, Theorem 6.2 and
6.4). `KnownDensityRatio` yields `MarginalDeploymentRisk`, the same
finite-sample guarantee kind as unweighted MDR. `Estimated` yields
`Asymptotic` only when *every one* of Theorem 6.4's four hypotheses is
declared true — training data independent of calibration,
`L2(P_X)`-consistency of the weight estimator (`WeightConsistencyEvidence`),
regularity of the paper's threshold function (`ThresholdRegularityEvidence`),
and `gamma == alpha` exactly — downgrading to `EmpiricalOnly` otherwise.
This is a stricter, theorem-by-theorem check, not a blanket
`Estimated -> Asymptotic` rule: `risksieve::anytime::AnytimeShiftedController`
downgrades every `Estimated` case to `EmpiricalOnly` unconditionally
instead, because the anytime-valid paper it implements (Hultberg,
Zachariah, and Ribeiro (2026), Theorem 4.7) has no asymptotic argument for
estimated weights at all. Both controllers record
`ExchangeabilityAssumption::CovariateShiftIid` (calibration i.i.d. from
`P`, test i.i.d. from a different `Q`), distinct from the plain `Iid`
(same distribution) claim neither setting actually satisfies. Calibration
points are weighted individually (`w(X_i)`), not just the test point, and
weights carry no normalization requirement — the construction is
invariant to rescaling every weight (calibration and test) by the same
positive constant, but not to a non-uniform reweighting, and every weight
is normalized by their shared maximum before computation so that
finite-but-huge weights (for example near `f64::MAX`) cannot spuriously
overflow it. The e-value is `f64::INFINITY` in a narrow, non-degenerate
case (a concrete instance found while building the oracle fixture, not a
hypothetical one — see `docs/references.md`'s "Equation 6.1 audit"),
represented by a dedicated `EValue` type (`Finite(NonNegative)` /
`PositiveInfinity`) rather than clamped to a large finite value; this
type lives in `certificate.rs` so `Diagnostics::risk_adjusted_evalue` can
use it directly, round-tripping `Finite`/`PositiveInfinity`/`None`
distinctly under the `serde` feature. Cross-checked against
`Tian-Bai/SCoRE`'s own `SCoRE_MDR_w` (38 fixture cases, 109 test points,
`tests/score_mdr_w_oracle.rs` — two independent comparisons per case,
since the official package has no weighted e-value function of its own to
check the e-value against; the official decision is compared exactly for
every case regardless of `gamma` vs `alpha`, confirmed by a 300,000-trial
randomized search rather than assumed) and against a Monte Carlo
simulation of the weighted MDR guarantee
(`tests/statistical_validity_weighted_mdr.rs`). Weighted SDR is deferred;
see `docs/roadmap.md`.

A follow-up review pass hardened this milestone's numerics further.
`WeightAccumulator::update` (used by `AnytimeShiftedController`) now
returns a `Result`, rejecting an update whose weight-squared, running
sum, running sum of squares, or effective sample size would overflow to
non-finite, and is fully transactional: every candidate value is computed
into a local first, so a rejected update never partially mutates the
accumulator. `AnytimeShiftedController::update` itself is now
transactional end to end (weight accumulation, loss evaluation, and the
derived `gamma_n` correction term all compute against local candidates
before anything is committed to `self`), and rejects the update outright
with `RiskSieveError::NumericalOverflow` rather than ever returning a
certificate whose `gamma_n` is `inf`/`NaN`. `certify_weighted`'s
calibration weight diagnostics, by contrast, use a separate,
never-failing `shift::importance::WeightSummary` helper: since
`weighted_risk_adjusted_evalue` already normalizes by the shared maximum
weight and stays exact regardless, a diagnostic-only overflow in
`weight_sum`/`weight_sum_of_squares` must not block the call the way an
overflow in the anytime controller's guarantee-bearing accumulator does.
New `Diagnostics::weight_sum_overflowed`/`weight_sum_of_squares_overflowed`
fields make that distinction explicit (`Some(true)` when the
corresponding field is `None` *because it overflowed*, not because it
was never computed) — the same class of serde-safety fix `EValue` exists
for `risk_adjusted_evalue`. The 300,000-trial `gamma > alpha` audit cited
above is reproducible by a third party via
`scripts/audits/compare_score_mdr_w.py --repo /path/to/Tian-Bai/SCoRE`;
see `docs/references.md`'s "Equation 6.1 audit" for the exact command and
reproduced counts.

See `AGENTS.md` section 7 for the full implementation sequence and
`docs/validation.md` for what is and is not tested yet.

## Scientific basis

`risksieve` unifies four layers, each traceable to a specific paper:

1. **Classical conformal risk control** for bounded monotone losses —
   Angelopoulos, Bates, Fisch, Lei, and Schuster (2024), *Conformal Risk
   Control*, ICLR 2024, [arXiv:2208.02814](https://arxiv.org/abs/2208.02814).
2. **Anytime-valid risk control** for growing calibration data — Hultberg,
   Zachariah, and Ribeiro (2026), *Anytime-Valid Conformal Risk Control*,
   [arXiv:2602.04364](https://arxiv.org/abs/2602.04364).
3. **Risk control for non-monotonic losses** and multidimensional
   parameters via algorithmic stability — Angelopoulos (2026), *Conformal
   Risk Control for Non-Monotonic Losses*,
   [arXiv:2602.20151](https://arxiv.org/abs/2602.20151).
4. **Selective deployment** through risk-adjusted e-values, MDR, and SDR —
   Bai and Jin (2026), *Conformal Selective Prediction with General Risk
   Control*, [arXiv:2603.24704](https://arxiv.org/abs/2603.24704).

Distribution-shift support is an explicit extension of each layer, not an
implicit default. See `docs/references.md` for the complete bibliography
and the mapping from each theorem to its implementation.

## Scope

In scope: bounded scalar losses, fixed-sample and anytime-valid risk
control, non-monotonic losses, multidimensional parameters, selective
deployment (MDR/SDR), risk-adjusted e-values, known and estimated
importance weights (clearly labeled), deterministic auditable
certificates.

Explicitly out of scope for the first stable release: training ML models,
automatic density-ratio estimation from raw features, generic nonlinear
optimization, automatic proofs of optimizer stability, arbitrary
concept-drift guarantees, causal claims, regulatory certification, and
bindings to other languages before the Rust API stabilizes. See
`AGENTS.md` section 3 for the complete list.

## Every certificate answers

- What risk quantity is controlled?
- Is the guarantee in expectation or with high probability?
- Is it fixed-sample or anytime-valid?
- Does it concern all predictions or only selected/deployed predictions?
- Which assumptions were required?
- Were importance weights known or estimated?
- Was optimizer stability proven, supplied, or merely estimated?
- Is the result a theorem-backed certificate, an asymptotic statement, or
  an empirical diagnostic?

## Usage

```bash
cargo build
cargo test --all-features
```

```rust
use risksieve::{BoundedLoss, OpenUnitInterval, ZeroOneLoss};

let alpha = OpenUnitInterval::new("alpha", 0.1)?;
let observed = ZeroOneLoss.evaluate_checked(&"cat", &"dog")?;
assert_eq!(observed, 1.0);
```

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at
your option. See `THIRD_PARTY_NOTICES.md` for the licensing status of code
and fixtures adapted from external repositories, and AGENTS.md section 11
for the clean-room implementation policy.

## Contributing

Read `AGENTS.md` first — it is the governing engineering policy for this
crate: scope, API principles, milestone sequence, numerical requirements,
testing strategy, citation policy, and pull-request requirements.
