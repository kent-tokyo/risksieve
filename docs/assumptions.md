# Assumptions

Every [`RiskCertificate`](../src/certificate.rs) carries an
[`Assumptions`](../src/guarantee.rs) value. This file explains each field
and, critically, whether the library can check it, whether it is
guaranteed by construction, or whether it is necessarily taken on the
caller's word. Conflating these categories is the single biggest way a
statistical library can overstate what it proves — see AGENTS.md section 4.

| Field | Meaning | Category |
|---|---|---|
| `exchangeability` | Calibration and test data are i.i.d., only exchangeable, or i.i.d. within each of two different (covariate-shifted) distributions | caller-declared, not checkable from observed data — the *scope* this covers differs by controller: one test point plus calibration (`n+1`) for `crc`, `anytime`, `nonmonotone`, and `selective::mdr`, but the *entire batch* plus calibration (`n+m`) for `selective::sdr`, a strictly stronger requirement (see Status below). `CovariateShiftIid` is a distinct claim from `Iid`, not a special case of it: calibration and test are each i.i.d. *separately*, but from two different distributions, so the combined sample is not identically distributed the way plain `Iid` asserts |
| `bounded_loss` | The interval the loss is contractually confined to | checked at runtime by [`BoundedLoss::evaluate_checked`](../src/loss.rs) |
| `monotonicity` | Whether, and in which direction, the loss is monotone in the parameter | caller-declared for now; a future milestone may add a runtime monotonicity check over the observed calibration grid |
| `right_continuity` | Whether the relevant statistic is right-continuous, as some threshold-search arguments require | caller-declared, not checkable from finitely many observations |
| `symmetry` | Whether the optimization procedure is permutation-invariant | proven by construction for symmetric primitives the library ships; caller-declared (`CallerAsserted`) for optimizers supplied by the caller |
| `stability` | Evidence behind a beta-stability constant | `Analytic` is proven by an external reference; `UserSupplied` is caller-declared; `Estimated` is computed by the library from data but is itself only an estimate, not a proof |
| `shift` | Whether, and how, covariate shift is corrected for | `NoShift` is caller-declared; `CovariateShift` further distinguishes `KnownDensityRatio` (caller-declared as known) from `Estimated` (caller-declared, now further broken into `method`, `training_data_separate_from_calibration`, `consistency`, and `threshold_regularity` — see Status below for how each controller uses these fields differently) |

## The four categories

1. **Caller-declared assumptions.** The library takes the caller's word and
   cannot verify them from the data it sees (`exchangeability`,
   `right_continuity`, `bounded_loss`'s stated bounds as opposed to the
   observed values, `UserSupplied` stability, `KnownDensityRatio`).
2. **Properties checked by the library.** Verified at runtime against
   actual data and rejected with a [`RiskSieveError`](../src/error.rs) if
   violated (each observed loss against `bounded_loss`; each importance
   weight's finiteness and sign, and the weight sequence's non-degeneracy,
   via [`WeightAccumulator`](../src/shift/importance.rs)).
3. **Properties proven by construction.** True because of how the code is
   written, not because of anything checked at runtime (`ProvenSymmetric`
   for library-supplied optimizers; `Analytic` stability when the cited
   proof actually matches the algorithm used).
4. **Properties not checkable from observed data.** A strict subset of
   caller-declared assumptions where no amount of additional data could
   settle the question from inside the library (exchangeability is the
   canonical example: no finite-sample test distinguishes it from
   arbitrary dependence in general).

## Rule

Do not encode an unverified mathematical assumption as `true` or as the
"library-approved" variant merely because the caller selected an option.
If a field's value came from the caller asserting something the library
did not verify, the enum variant must say so explicitly (for example
`SymmetryAssumption::CallerAsserted`, not `ProvenSymmetric`).

## Status

Milestone 0 defines this vocabulary; Milestones 1-6 populate it.

Worth noting: `selective::mdr::certify` (Milestone 4) has no parameter
search at all — Algorithm 1 makes a single deploy/abstain decision from a
risk-adjusted e-value, so `monotonicity` (`NonMonotone`) and
`right_continuity` (`false`) and `stability` (`Unknown`) do not describe
anything the method actually reasons about. Per the rule above, each is
still set to the variant that claims the least rather than left out: `NonMonotone`
and `false` because there is no monotone parameter to claim continuity or
direction for, and `Unknown` because no stability constant is used or
computed. `symmetry` is `ProvenSymmetric` for a real reason, though: Equation
4.1's construction only depends on the calibration set as a multiset of
`(score, loss)` pairs, never on input order (checked by a permutation
property test in `src/selective/evalue.rs`).

`selective::sdr::certify` (Milestone 5) sets the same
`monotonicity`/`right_continuity`/`stability` defaults as `mdr::certify`
and for the same reason (no parameter search), and `symmetry` is
`ProvenSymmetric` for the same reason too (checked by a permutation
property test in `src/selective/ebh.rs`). Its `exchangeability`, however,
is a *strictly stronger* claim than every other controller's: Theorem 3.3
and Algorithm 2 require `{(X_i,Y_i)}_{i=1}^{n+m}` — calibration plus
*every* test point in the batch — to be jointly exchangeable, not just
each test point individually exchangeable with calibration. A caller
who assembles a batch by, say, filtering or sorting test points by some
property of their own is not entitled to assume this holds. See
`src/selective/sdr.rs`'s module docs.

`anytime::AnytimeShiftedController` (Milestone 6) is the first controller
to populate `shift` with anything other than `NoShift`. Its
`exchangeability` is `CovariateShiftIid`, *not* the unshifted
`AnytimeController`'s plain `Iid`: the shift is between calibration and
test *distributions* (`P` vs. the paper's `P*`), so asserting `Iid`
(same distribution) would misstate exactly the thing `shift` is there to
correct for. `shift` is `CovariateShift { weight_source }`, taken
directly from the caller's `weight_source(...)` builder call — a
caller-declared choice that directly determines the certificate's
`GuaranteeKind`: `AnytimeHighProbability` for `KnownDensityRatio`, and
`EmpiricalOnly` for `Estimated` *unconditionally*, regardless of what its
`consistency`/`threshold_regularity` fields declare — Hultberg,
Zachariah, and Ribeiro (2026), Theorem 4.7 has no asymptotic argument for
estimated weights at all (it takes `omega` as known, a standing
hypothesis, not something the theorem relaxes), so there is nothing for
those fields to unlock here. They exist to support
`selective::mdr::certify_weighted`'s own hypothesis check below, not this
controller's.

`selective::mdr::certify_weighted` (Milestone 6, Equation 6.1) sets the
same `monotonicity`/`right_continuity`/`stability` defaults as
`mdr::certify`, for the same reason (still a single deploy/abstain
decision, no parameter search), and `symmetry` is `ProvenSymmetric` for
the same reason (Equation 6.1 depends on calibration only as a multiset
of `(score, weight, loss)` triples — checked by a permutation property
test in `src/selective/evalue_weighted.rs`). Unlike `mdr::certify`,
`exchangeability` is `CovariateShiftIid`, not `Exchangeable`: Assumption
6.1 states i.i.d. draws within each of the calibration (`P`) and test
(`Q`) distributions, not mere exchangeability, and not plain `Iid` either
since `P != Q`. `shift` is `CovariateShift { weight_source }`, taken
directly from the caller's `weight_source` argument. Unlike
`AnytimeShiftedController`, this controller's `Estimated` case *can*
reach `Asymptotic` — but only when every one of Bai and Jin (2026)
Theorem 6.4's four hypotheses is declared true:
`training_data_separate_from_calibration`, `consistency`
(`WeightConsistencyEvidence::Asserted`), `threshold_regularity`
(`ThresholdRegularityEvidence::Asserted`), and the caller passing
`gamma == alpha` exactly (checked as exact `f64` equality, the
conservative direction: a near-but-not-identical value does not count).
Any one missing downgrades to `EmpiricalOnly` instead of erroring — a
real deploy/abstain decision still comes out of the same e-value
computation, just without a theorem attached to it, the same choice
`nonmonotone::stability::certify` makes for `StabilityEvidence::Estimated`
without its full analytic hypothesis. Declaring any of these fields true
is the caller's assertion; none of them is verified by this crate from
data.
