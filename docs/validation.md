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
   name; see `src/anytime/boundary.rs`'s `weighted_term` doc). The
   remaining `tests/paper_*.rs` files land as their theorems are
   implemented.
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
has ten property tests so far:
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
identities. No monotonicity/dominance property between the coupled and
independent constructions is asserted, since neither the paper nor this
crate proves one. Run all of tiers 1-3 with:

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

Tier 5 has `tests/score_sdr_oracle.rs`, cross-checking
`coupled_risk_adjusted_evalues` (Equation 5.1) against 30 cases generated
from `Tian-Bai/SCoRE`'s `SCoRE_SDR` (commit
`401b7caf6d030825ff67e8f08e44ba15ee8c94af`, package version `0.1.1`) —
e-values with a combined absolute/relative tolerance (`1e-9`), selected
indices and `tau_hat` exactly. There is no independent-construction
(Equation 4.1) oracle column in this fixture; see `docs/references.md`
for why `SCoRE_MDR_bf` was found unsuitable as an oracle for it.

Tier 6 (regressions) has no entries yet. Each remaining tier or
per-milestone gap activates as the work in `docs/roadmap.md` that needs it
lands; this file will list the actual test names and fixture locations as
they are added, rather than describing them abstractly.
