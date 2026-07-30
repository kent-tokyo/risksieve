# Validation strategy

Summary of AGENTS.md section 9, kept close to the test suite so the
policy and the tests cannot silently diverge. Passing tests alone is not
sufficient if the API can misstate the guarantee (AGENTS.md section 16).

## Tiers

1. **Deterministic unit tests** — every public mathematical primitive is
   tested at its edge cases (alpha/delta near 0 and 1, empty calibration
   data, ties, invalid inputs, serialization round trips when `serde` is
   enabled). Currently in `src/*.rs` `#[cfg(test)]` modules, next to the
   code they test.
2. **Paper-traceable tests** — named after the source and theorem or
   algorithm, for example `anytime_theorem_4_1_boundary_matches_reference`.
   `tests/paper_crc.rs` covers Angelopoulos, Bates, Fisch, Lei, and
   Schuster (2024), Theorem 1; `tests/paper_anytime.rs` covers Hultberg,
   Zachariah, and Ribeiro (2026), Theorem 4.1 and Definition 2.7, both
   against hand- or Python-computed fixtures. `tests/paper_nonmonotone.rs`
   covers Angelopoulos (2026), Theorem 1 — this one has no numeric
   fixture to compute (the theorem doesn't itself search for a parameter
   or compute a correction term), so it instead checks the theorem's
   exact hypothesis boundary directly. An earlier version of this file
   tried to check Proposition 1 (monotone CRC as the `beta = 0` special
   case) by feeding `crc::monotone::certify`'s output back through
   `certify`, but that only asserted passthrough fields equal themselves
   — a tautology that would pass regardless of what `crc::monotone`
   returned — so it was removed rather than left as a misleading
   paper-traceable name. Proposition 1 is now validated for real, against
   a reference algorithm `A*` this crate chose itself (the uncorrected
   oracle threshold on the full dataset), proved to be dominated by
   `certify`'s leave-one-out threshold as an exact per-dataset inequality
   rather than a Monte Carlo estimate. Two losses are used on purpose: with
   the 0/1 `ExceedsThreshold` loss
   (`nonmonotone_proposition_1_leave_one_out_domination_is_non_vacuous`),
   the risk-level half of the claim collapses to an exact equality on
   every held-out point — a genuine structural fact of that loss, proved
   in the test's doc comment, not a coincidence of the fixture — so
   `nonmonotone_proposition_1_leave_one_out_can_strictly_reduce_risk` uses
   a continuously-varying `RampLoss` to exercise the risk-level inequality
   as strict instead. The `proptest`-fuzzed counterpart
   `nonmonotone_proposition_1_leave_one_out_never_beats_the_oracle` checks
   the threshold-level domination for both losses over randomized data.
   See `src/nonmonotone/stability.rs` and the tests' own doc comments for
   the full algebra. `tests/paper_score_mdr.rs`
   covers Bai and Jin (2026), Definition 3.1, Equation 4.1, Algorithm 1,
   and Theorem 3.2, with two hand-computed fixtures small enough (`n=1`)
   to derive by hand in the file's own comments. `tests/paper_score_sdr.rs`
   covers Theorem 3.3 (eBH selection), Algorithm 2, and Equation 5.1 /
   Theorem 5.1 (the paper's own cross-test-point-coupled e-value),
   including a hand-computed `m=3` eBH trace, a hand-computed coupled
   `m=2` e-value trace, the zero-selection convention, and a fixture where
   the coupled and independent (`certify_independent`, Equation 4.1
   applied per test point) constructions select different sets — see
   `src/selective/coupled.rs` and `src/selective/sdr.rs` for the
   constructions themselves and `docs/roadmap.md` for what remains open
   (randomized pruning, weighted SDR). `tests/paper_anytime_shifted.rs` covers Hultberg,
   Zachariah, and Ribeiro (2026), Theorem 4.7; its central test,
   `anytime_theorem_4_7_weighted_correction_never_tightens_the_unweighted_bound`,
   is a permanent regression guard for a case where independently derived
   dimensional reasoning overrode digit-for-digit-consistent fetched text
   (every fetch of the theorem's correction term misread one function
   name; see `src/anytime/boundary.rs`'s `weighted_term` doc). Weighted
   MDR (Equation 6.1, Theorem 6.2/6.4) has no dedicated `tests/paper_*.rs`
   file of its own: its hand-traceable fixtures (`n=1` cases at weight `1`
   matching `tests/paper_score_mdr.rs`'s own `n=1` fixtures exactly, and a
   hand-derived `+infinity` case) live as unit tests directly in
   `src/selective/evalue_weighted.rs` and `src/selective/mdr.rs`'s
   `#[cfg(test)]` modules instead — see tier 5 below for its
   cross-language coverage. The remaining `tests/paper_*.rs` files land as
   their theorems are implemented.
3. **Property tests** (`proptest`) — structural invariants such as
   permutation invariance, non-negativity of e-values, and serialization
   preserving semantics. `proptest` is already a dev-dependency; property
   tests land alongside the first controller that has invariants worth
   stating.
4. **Statistical validity tests** — Monte Carlo checks with fixed seeds
   for CI and slower `--ignored` tests with more repetitions for
   scheduled runs. `tests/statistical_validity.rs` opens this tier (SDR
   only so far); the fast/slow split and the recording requirements (RNG,
   seed, repetitions, data-generating process, target alpha/delta,
   acceptance criterion, software version) apply to every entry.
5. **Cross-language oracle tests** — the MIT-licensed SCoRE and
   foundational-CRC reference repositories may be used to generate
   fixtures; the non-monotonic repository must not be copied from until
   its license is clarified. Checked-in fixtures must record provenance
   and a generation script. `tests/score_sdr_oracle.rs` opens this tier,
   reading `tests/fixtures/score_sdr_v0_1_1.json`
   (`scripts/oracles/generate_score_sdr.py` generates it; Python is never
   invoked by `cargo test`).
6. **Regression tests** — every discovered correctness bug gets a minimal
   failing test, a fix, a note on which guarantee could have been
   misstated, and a `CHANGELOG.md` entry when public behavior changes.

## Current status

Tier 1 covers the validated numeric types, the bounded-loss contract, the
guarantee/certificate vocabulary, and every controller's error path except
`selective::ebh::select`, which cannot fail (an empty or degenerate input
is a valid selection, not an error) and so returns no `Result`.
`shift::importance::WeightAccumulator` is also tier 1: non-negative
finite validation is delegated to `NonNegative` at the call site, and
`WeightAccumulator` itself is tested directly for degenerate (all-zero,
empty) rejection, min/max/ESS tracking, and extreme-but-finite weights.
Tier 2 has `tests/paper_crc.rs`, `tests/paper_anytime.rs`,
`tests/paper_nonmonotone.rs`, `tests/paper_score_mdr.rs`,
`tests/paper_score_sdr.rs`, and `tests/paper_anytime_shifted.rs`. Tier 3
has twelve property tests so far:
`anytime::calibration::tests::anytime_threshold_sequence_is_non_increasing`
and `anytime_theorem_4_7_threshold_sequence_is_non_increasing` (in
`tests/paper_anytime_shifted.rs`), covering the invariant AGENTS.md
section 9.3 names explicitly ("the anytime threshold sequence follows the
monotonicity/running-minimum rule") with randomized alpha, delta, and
observation streams (and, for the shifted case, randomized weights too);
`selective::evalue::tests::construction_is_permutation_invariant` and
`selective::ebh::tests::selected_set_is_permutation_invariant`, covering
"permutation invariance of symmetric procedures" for the SCoRE e-value
construction and the eBH selection engine respectively;
`selective::mdr::tests::score_proposition_4_4_shortcut_matches_general_decision`,
which cross-checks Bai and Jin (2026)'s Proposition 4.4 closed-form
shortcut against the general Equation 4.1 computation over randomized
calibration sets — the "efficient computation shortcut only after a
transparent reference implementation passes tests" requirement in
AGENTS.md's Milestone 4 description;
`nonmonotone_proposition_1_leave_one_out_never_beats_the_oracle` (in
`tests/paper_nonmonotone.rs`), fuzzing Proposition 1's per-dataset
domination inequality over randomized observation vectors; and, for the
coupled SDR construction,
`selective::coupled::tests::calibration_and_test_batch_order_is_invariant`,
`evalues_are_non_negative_fuzzed`, and
`single_test_point_matches_equation_4_1_fuzzed` (in
`src/selective/coupled.rs`, the last one fuzzing the `m=1` reduction to
Equation 4.1 derived in that module's docs), plus
`selective::sdr::tests::coupled_certify_selected_set_is_invariant_to_test_batch_order`
(in `src/selective/sdr.rs`), checking that permuting the test batch's
submission order changes neither `tau_hat` nor the selected set of
identities; and, for weighted MDR,
`selective::evalue_weighted::tests::construction_is_permutation_invariant`
and `evalues_are_non_negative_fuzzed` (in
`src/selective/evalue_weighted.rs`), covering calibration permutation
invariance and e-value non-negativity for Equation 6.1. No
scale-invariance property test is stated for *unnormalized* weights in
general (only *uniform* rescaling of every weight together is invariant —
see that module's docs for why non-uniform rescaling can and does change
the e-value, checked by
`non_uniform_rescale_of_calibration_only_can_change_the_evalue` instead).
No monotonicity/dominance property between the coupled and independent
constructions is asserted, since neither the paper nor this crate proves
one. Run all of tiers 1-3 with:

```bash
cargo test --all-features
```

Tier 4 has `tests/statistical_validity.rs`: a fast, deterministic
`sdr_monte_carlo_smoke_test` (500 repetitions, run by default) and a
slower `sdr_monte_carlo_large_scale` (20,000 repetitions, `#[ignore]`d).
Both use a hand-rolled SplitMix64 RNG (no new dependency), a simple
exchangeable DGP (i.i.d. `(U_i, L_i)` with `U_i ~ Uniform(0,1)` as the
score and `L_i | U_i ~ Bernoulli(U_i)` as the loss, so any calibration/test
split is exchangeable by construction), and accept
`observed_mean_sdr <= alpha + hoeffding_half_width(repetitions, delta)`
rather than a naive `observed <= alpha` assertion — the half-width is
`~0.061` at the smoke test's repetition count (wide enough that it mainly
catches a badly broken implementation, not a tight bound) and `~0.0096`
at the large-scale test's. Run the slower version explicitly:

```bash
cargo test --test statistical_validity -- --ignored --nocapture
```

Tier 4 also has `tests/statistical_validity_weighted_mdr.rs` for weighted
MDR (Equation 6.1, Theorem 6.2): a fast `weighted_mdr_monte_carlo_smoke_test`
(500 repetitions) and a slower `weighted_mdr_monte_carlo_large_scale`
(20,000 repetitions, `#[ignore]`d). The DGP splits the covariate into an
independent score coordinate `X1 ~ Uniform(0,1)` (identical under `P` and
`Q`) and a risk coordinate `X2` (`~ Uniform(0,1)` under `P`, density `2*x2`
under `Q`), with loss `L | X ~ Bernoulli(X2)` and known density ratio
`w(x1,x2) = 2*x2` — deliberately decoupling the score from the shift (an
earlier, single-coordinate version of this DGP was vacuous: the
unweighted procedure's score threshold silently self-corrected for a
shift that lived on the same coordinate it scored by, so it passed the
check even at weight `1`; see the file's module docs for the full
account). Three arms share one replication loop: `weighted`
(`certify_weighted` with the true weights, `KnownDensityRatio`), `naive`
(the same shifted test point through plain unweighted `certify` — the
wrong thing to do, included specifically to prove the DGP is not
vacuous), and `control` (shares the unshifted score coordinate with the
other two arms, re-draws the risk coordinate from `P` instead of `Q`,
weight `1`). At `alpha = gamma = 0.3`, seed `20260730`: at 500 repetitions
(half-width `0.0607`), `weighted` mean `0.2120`, `control` mean `0.2900`,
`naive` mean `0.3880`; at 20,000 repetitions (half-width `0.0096`),
`weighted` mean `0.22485`, `control` mean `0.28780`, `naive` mean
`0.38010` — `naive` exceeds `alpha` by a wide margin at both repetition
counts, confirming the DGP genuinely exercises the weighting rather than
passing vacuously. This file deliberately does not exercise
`ImportanceWeightSource::Estimated`: a Monte Carlo pass there would only
bear on Theorem 6.4's asymptotic conclusion, not a finite-sample one, and
could be misread as validating a guarantee this crate does not make for
that case. Run the slower version explicitly:

```bash
cargo test --test statistical_validity_weighted_mdr -- --ignored --nocapture
```

Tier 5 has `tests/score_sdr_oracle.rs`, cross-checking
`coupled_risk_adjusted_evalues` (Equation 5.1) against 30 cases generated
from `Tian-Bai/SCoRE`'s `SCoRE_SDR` (commit
`401b7caf6d030825ff67e8f08e44ba15ee8c94af`, package version `0.1.1`) —
e-values with a combined absolute/relative tolerance (`1e-9`), selected
indices and `tau_hat` exactly. There is no independent-construction
(Equation 4.1) oracle column in this fixture; see `docs/references.md`
for why `SCoRE_MDR_bf` was found unsuitable as an oracle for it.

Tier 5 also has `tests/score_mdr_w_oracle.rs`, reading 38 cases (109 test
points) from `tests/fixtures/score_mdr_w_v0_1_1.json`
(`scripts/oracles/generate_score_mdr_w.py` generates it against the same
`Tian-Bai/SCoRE` commit and package version). Unlike the SDR oracle, this
one makes *two* independent comparisons per test point, because the
official package has no weighted e-value function at all (only
`SCoRE_MDR_w`, a decision-only shortcut valid unconditionally for
`gamma <= alpha`, with an extra overlap condition for `gamma > alpha`):
the e-value itself is checked against the fixture generator's own
from-scratch Python breakpoint-enumeration reference implementation of
Equation 6.1 (not derived from this crate's Rust code, and not from the
official package), with a combined absolute/relative tolerance (`1e-9`),
including exact agreement on a `+infinity` case; the deploy/abstain
decision is checked against the official `SCoRE_MDR_w` shortcut exactly,
for *every* case, `gamma <= alpha` or not — this crate's own construction
never takes the shortcut, so a 300,000-trial randomized search (not mere
assumption) confirmed its decision needs no help from the shortcut's own
`gamma > alpha` overlap condition to already agree with it, including on
two fixture cases specifically constructed to exercise both outcomes of
that condition. See `docs/references.md`'s "Equation 6.1 audit" for this
design and the floating-point boundary bug in the fixture generator's own
reference implementation that building those cases surfaced (fixed with
the same `feasibility_epsilon` tolerance pattern the Rust code already
uses).

Tier 6 (regressions) has six entries so far, all found while building
weighted MDR:

1. `risk_adjusted_evalue` (`src/selective/evalue.rs`, Equation 4.1)
   grouped tied calibration scores via a sort keyed on score alone,
   leaving its summed loss dependent on the caller's input order rather
   than only on the calibration multiset — found while building the
   weighted e-value construction, which needed `risk_adjusted_evalue` to
   be genuinely order-invariant for a "weight `1` matches unweighted"
   test to be meaningful. No public guarantee was misstated (the affected
   quantity is an internal summation step, not a part of any theorem
   statement), but the *specific returned e-value* could differ by a few
   ULPs depending on input order for adversarial tied inputs. Fixed by
   sorting by `(score, loss)` and Kahan-summing within canonical groups;
   regression test `ties_are_summed_in_a_canonical_order_not_input_order`
   in `src/selective/evalue.rs` checks forward, reversed, and permuted
   input now agree bit-exactly.
2. `selective::mdr::certify_weighted` returned `GuaranteeKind::Asymptotic`
   for *any* `ImportanceWeightSource::Estimated`, regardless of whether
   Theorem 6.4's four hypotheses actually held — a genuine guarantee
   overstatement, not merely an internal detail. Fixed by adding typed
   `consistency`/`threshold_regularity` evidence fields and downgrading to
   `EmpiricalOnly` unless every hypothesis (including `gamma == alpha`
   exactly) is declared true; see
   `estimated_weight_with_full_theorem_6_4_conditions_reaches_asymptotic`
   and its four sibling tests in `src/selective/mdr.rs`.
3. `anytime::AnytimeShiftedController::update` made the identical
   overstatement for its own `Estimated` case, but with no theorem at all
   to fall back on (Theorem 4.7 never discusses estimated weights). Fixed
   to downgrade unconditionally to `EmpiricalOnly`; see
   `estimated_weight_source_downgrades_to_empirical_only` in
   `src/anytime/shifted.rs` and
   `anytime_theorem_4_7_estimated_weights_yield_empirical_only_not_high_probability`
   in `tests/paper_anytime_shifted.rs`.
4. Both of the controllers above recorded
   `Assumptions::exchangeability` as `ExchangeabilityAssumption::Iid`
   under covariate shift, which asserts calibration and test are drawn
   from the *same* distribution — the wrong claim when they are drawn
   from two different ones. Fixed by adding
   `ExchangeabilityAssumption::CovariateShiftIid` and using it in both
   places; see `weighted_exchangeability_is_covariate_shift_iid_not_plain_iid`
   and `exchangeability_is_covariate_shift_iid_not_plain_iid`.
5. `weighted_risk_adjusted_evalue` computed at the caller's raw weight
   scale, so finite-but-huge weights (both near `f64::MAX`) could
   overflow `total_weight` to `+infinity`, spuriously producing
   `EValue::PositiveInfinity` for a genuinely finite e-value. Fixed by
   normalizing every weight by their shared maximum first, exploiting the
   construction's own proven uniform-scale invariance — which in turn
   exposed a second, latent bug (the normalized weight sum was summed in
   caller order, not canonical order, so it was never truly
   permutation-invariant; the extra rounding from normalization was
   enough to surface a 1-ULP mismatch a proptest already covered). Fixed
   with the same canonical-order-before-summing pattern used for tied
   score groups; see
   `huge_but_finite_weights_do_not_spuriously_overflow_to_infinity` and
   `construction_is_permutation_invariant` in
   `src/selective/evalue_weighted.rs`.
6. This fixture generator's own `weighted_evalue_reference` (a from-scratch
   Python reference, not the official package or this crate's Rust code)
   had no epsilon tolerance on its feasibility comparison, so a breakpoint
   computed to satisfy `F(t;l) <= gamma` with exact equality could be
   incorrectly rejected after floating-point rounding, silently missing
   the true infimum. A 200,000-trial search surfaced 69 disagreements
   against the official decision before the fix; hand-deriving the true
   e-value for the smallest failing case confirmed the bug was in the
   reference script, not in the Rust crate (which already carries this
   tolerance) or the official package. Fixed with the identical
   `feasibility_epsilon` pattern; a 300,000-trial re-check found zero
   further mismatches. See `docs/references.md`'s "Equation 6.1 audit".

See `CHANGELOG.md`'s `Fixed` section for all six. Each remaining tier or
per-milestone gap activates as the work in `docs/roadmap.md` that needs it
lands; this file will list the actual test names and fixture locations as
they are added, rather than describing them abstractly.

## Timing comparison: coupled vs. independent SDR (informal)

`tests/timing_comparison.rs` (`#[ignore]`d, not `criterion` — see its own
module docs for why a benchmark dependency wasn't added for a single
comparative reading) measures `sdr::certify` (coupled) against
`sdr::certify_independent` at three fixed input sizes, in release mode,
with a 5-iteration warm-up and the median of 20 measured iterations
reported. Run it with:

```bash
cargo test --release --test timing_comparison -- --ignored --nocapture
```

Measured on `aarch64-macos` (Darwin 25.5.0), `rustc 1.97.0 (2d8144b78
2026-07-07)`, `risksieve` at this PR's HEAD:

| size | `n` | `m` | coupled (median) | independent (median) |
|---|---|---|---|---|
| small | 20 | 5 | 4.542 µs | 10.5 µs |
| medium | 200 | 50 | 66.084 µs | 883.959 µs |
| large | 2000 | 500 | 6.772417 ms | 312.535459 ms |

The coupled construction was faster at all three sizes tested here, by a
growing margin as `n` increases. This tracks the two constructions'
different complexity classes, not a coincidence of these particular
sizes: `certify_independent` calls `risk_adjusted_evalue` once per test
point, and that reference implementation is `O(n^2)` per call (its own
breakpoint-enumeration reference scan, deliberately left unoptimized per
AGENTS.md's "reference implementation first" policy — Proposition 4.4's
closed-form shortcut is verified equivalent but not wired in as a
separate code path, see `docs/roadmap.md`), giving `certify_independent`
an overall `O(m * n^2)`. `certify`'s coupled construction is `O((n+m)
log(n+m) + m(n+m))` overall. For any fixed `m`, `n^2` eventually
dominates `n+m` as `n` grows, so the coupled construction should keep
winning at larger `n` than tested here too — but this has only been
measured at `n <= 2000`, not proven as a universal ordering, and a
different `n`/`m` ratio (very large `m`, small `n`) was not swept.
