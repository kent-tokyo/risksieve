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
   covers Theorem 3.3 (eBH selection) and Algorithm 2, including a
   hand-computed `m=3` eBH trace and the zero-selection convention; it
   composes Milestone 4's e-value construction rather than the paper's own
   Equation 5.1 (deferred — see `src/selective/sdr.rs` and
   `tasks/todo.md`). `tests/paper_anytime_shifted.rs` covers Hultberg,
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
   scheduled runs. None exist yet; the fast/slow split and the recording
   requirements (RNG, seed, repetitions, data-generating process, target
   alpha/delta, acceptance criterion, software version) apply starting
   with Milestone 1.
5. **Cross-language oracle tests** — the MIT-licensed SCoRE and
   foundational-CRC reference repositories may be used to generate
   fixtures; the non-monotonic repository must not be copied from until
   its license is clarified. Checked-in fixtures must record provenance
   and a generation script. None exist yet.
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
has seven property tests so far:
`anytime::calibration::tests::anytime_threshold_sequence_is_non_increasing`
and `anytime_theorem_4_7_threshold_sequence_is_non_increasing` (in
`tests/paper_anytime_shifted.rs`), covering the invariant AGENTS.md
section 9.3 names explicitly ("the anytime threshold sequence follows the
monotonicity/running-minimum rule") with randomized alpha, delta, and
observation streams (and, for the shifted case, randomized weights too);
`selective::evalue::tests::construction_is_permutation_invariant` and
`selective::ebh::tests::selected_set_is_permutation_invariant`, covering
"permutation invariance of symmetric procedures" for the SCoRE e-value
construction and the eBH selection engine respectively; and
`selective::mdr::tests::score_proposition_4_4_shortcut_matches_general_decision`,
which cross-checks Bai and Jin (2026)'s Proposition 4.4 closed-form
shortcut against the general Equation 4.1 computation over randomized
calibration sets — the "efficient computation shortcut only after a
transparent reference implementation passes tests" requirement in
AGENTS.md's Milestone 4 description; and
`nonmonotone_proposition_1_leave_one_out_never_beats_the_oracle` (in
`tests/paper_nonmonotone.rs`), fuzzing Proposition 1's per-dataset
domination inequality over randomized observation vectors. Run all of
tiers 1-3 with:

```bash
cargo test --all-features
```

Tiers 4-6 (statistical validity, cross-language oracles, regressions)
have no entries yet. Each remaining tier activates as the milestone in
AGENTS.md section 7 that needs it lands; this file will list the actual
test names and fixture locations as they are added, rather than
describing them abstractly.
