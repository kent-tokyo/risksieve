# risksieve

*[日本語](README_ja.md)*

A Rust library that sieves predictions and decisions through finite-sample
and anytime-valid conformal risk guarantees.

## Status

**Milestone 0 (vocabulary), Milestone 1 (classical monotone CRC),
Milestone 2 (anytime-valid monotone CRC), Milestone 3 (non-monotonic CRC,
partial), Milestone 4 (SCoRE-MDR, partial), Milestone 5 (SCoRE-SDR,
partial), and Milestone 6 (distribution shift, partial) are done.**
Milestone 7 (downstream examples) is not implemented yet.

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
selective classification, regularized ERM) are tracked in `tasks/todo.md`.

Milestone 4 adds `risksieve::selective::evalue::risk_adjusted_evalue` and
`risksieve::selective::mdr::certify`, the SCoRE-MDR direct deployment
decision (Bai and Jin (2026), Definition 3.1, Equation 4.1, Algorithm 1,
Theorem 3.2). Unlike the search-based controllers, this makes a single
deploy/abstain decision from a risk-adjusted e-value; the resulting
`E[loss * deploy] <= alpha` bound is marginal over the joint draw, not a
property of any one realized decision.

Milestone 5 adds `risksieve::selective::sdr::certify`, batch SCoRE-SDR
(Bai and Jin (2026), Algorithm 2, Theorem 3.3), built from a generic eBH
selection engine (`risksieve::selective::ebh::select`) composed with
Milestone 4's per-test-point e-value construction, applied independently
to each item in the batch rather than the paper's own cross-test-point
construction (Equation 5.1) — deferred because its efficient-computation
algorithm wasn't extractable with confidence and its normalizing
function's monotonicity in the threshold isn't obvious; see
`tasks/todo.md`. This remains statistically valid, just presumably less
powerful than the paper's own construction. An empty selected set is a
valid certificate, not an error; `risksieve::selective::sdr::realized_selective_risk`
computes the post-hoc realized risk once labels arrive, returning a plain
number rather than a certificate so it can't be mistaken for the
guarantee itself.

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
downgrades to an asymptotic one, since the paper does not establish
finite-sample validity for estimated weights. Weighted SCoRE is deferred;
see `tasks/todo.md`.

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
