# References

Complete bibliography for `risksieve`, and the mapping from each cited
result to the code that implements it. Update the "Implemented in" column
in the same change that implements a result; do not let this file drift
from the code.

## Foundational CRC

Anastasios N. Angelopoulos, Stephen Bates, Adam Fisch, Lihua Lei, and Tal
Schuster. **Conformal Risk Control.** ICLR 2024. arXiv:2208.02814.
<https://arxiv.org/abs/2208.02814>

Reference implementation (MIT): the paper's companion repository.

| Result | Implemented in |
|---|---|
| Theorem 1 (bounded monotone loss, expected-risk control) | `src/crc/monotone.rs::certify`, tested in `tests/paper_crc.rs` |

## Anytime-valid CRC

Bror Hultberg, Dave Zachariah, and Antônio H. Ribeiro. **Anytime-Valid
Conformal Risk Control.** 2026. arXiv:2602.04364.
<https://arxiv.org/abs/2602.04364>

No official implementation was identified during the initial project
review; this crate implements the paper's mathematics independently,
extracted from the paper's HTML rendering (arxiv.org and ar5iv) since it
postdates this project's training-data cutoff. Cross-checked across three
independent fetches of the theorem statement before implementing; see the
provenance note in `src/anytime/boundary.rs`'s test module for the
Python-computed fixture used to validate the Rust implementation.

| Result | Implemented in |
|---|---|
| Definition 2.7 (anytime-valid risk control) | `src/anytime/calibration.rs::AnytimeController::update` (guarantee kind) |
| Theorem 4.1 (bounded monotone loss, anytime correction) | `src/anytime/boundary.rs` (boundary functions), `src/anytime/calibration.rs` (threshold search + running minimum), tested in `tests/paper_anytime.rs` |
| Corollary 4.2 (miscoverage specialization) | not yet implemented — `AnytimeController` is generic over any `[0, B]` loss, so `B = 1` already covers this case, but no dedicated miscoverage-loss built-in exists yet |
| Proposition 4.5 (asymptotic tightness diagnostics) | not yet implemented (Milestone 2 follow-up) |
| Theorem 4.7 (importance-weighted extension under distribution shift) | `src/anytime/boundary.rs::weighted_term`, `src/anytime/shifted.rs::AnytimeShiftedController`, tested in `tests/paper_anytime_shifted.rs`. Every independent fetch of this theorem's correction term read its boundary-function call as `h_{B,m*,delta}(...)`; a dimensional argument (confirmed numerically against the unweighted correction) shows this is impossible, and the correct reading uses a different, stripped-down boundary function instead — see `src/anytime/boundary.rs`'s `weighted_term` doc for the full argument. This is the one case in this crate where independent derivation overrode digit-for-digit-consistent fetched text. |

## Non-monotonic CRC

Anastasios N. Angelopoulos. **Conformal Risk Control for Non-Monotonic
Losses.** 2026. arXiv:2602.20151. <https://arxiv.org/abs/2602.20151>

Reference experiment repository:
<https://github.com/aangelopoulos/nonmonotonic-crc> — README displays an
MIT badge, but no root `LICENSE` file was found during the initial
review. Treated as all-rights-reserved for code-reuse purposes until
clarified; implemented from the paper as a clean-room implementation, not
from this repository's source. This paper postdates this project's
training-data cutoff; see the provenance note in
`src/nonmonotone/stability.rs` for how its theorem statement was
extracted and cross-checked.

| Result | Implemented in |
|---|---|
| Theorem 1 (risk control from symmetry and beta-stability) | `src/nonmonotone/stability.rs::certify`, tested in `tests/paper_nonmonotone.rs` |
| Proposition 1 (monotone CRC is the `beta = 0` special case) | not implemented or validated — checking it would require computing both sides of the beta-stability definition empirically (leave-one-out-average vs. full-sample risk); see `tasks/todo.md` |
| Proposition 2 (bounded-loss discretization) | not yet implemented — its stated guarantee is an asymptotic `Õ(1/√n)` bound involving the Lambert W function, extracted with lower confidence than Theorem 1; see `tasks/todo.md` before implementing |
| Proposition 3 / Corollary 2 (continuous Lipschitz losses) | not yet implemented (Milestone 3 follow-up) |
| Proposition 4 / 5 / Corollary 3 (selective classification) | not yet implemented (Milestone 3 follow-up) |
| Proposition 6 / Corollary 4 (regularized ERM) | not yet implemented (Milestone 3 follow-up) |
| Theorem 2, Propositions 7-8 (multivariate / gradient extensions) | not yet implemented — note `nonmonotone::stability::certify`'s `Parameter` type is already unconstrained and generic, so multidimensional parameters work today for Theorem 1 itself |

## SCoRE

Tian Bai and Ying Jin. **Conformal Selective Prediction with General Risk
Control.** 2026. arXiv:2603.24704. <https://arxiv.org/abs/2603.24704>

Reference implementation (MIT): <https://github.com/Tian-Bai/SCoRE> — may
be used as a behavioral test oracle and for independent cross-checking;
this crate reimplements the paper idiomatically rather than translating
the reference line by line. This paper postdates this project's
training-data cutoff; see the provenance note in `src/selective/evalue.rs`
for how Definition 3.1, Equation 4.1/4.2, Theorem 4.2, and Remark 4.5 were
extracted and cross-checked.

| Result | Implemented in |
|---|---|
| Definition 3.1 (risk-adjusted e-values) | `src/selective/evalue.rs::risk_adjusted_evalue`, tested in `tests/paper_score_mdr.rs` |
| Equation 4.1/4.2 (e-value construction) | `src/selective/evalue.rs::risk_adjusted_evalue`, via an exact breakpoint-enumeration reference implementation (see the module docs for the derivation) |
| Theorem 4.2 (e-value validity for any fixed gamma) | relied on but not independently tested beyond `risk_adjusted_evalue`'s own construction |
| MDR definition and Theorem 3.2 (MDR control) | `src/selective/mdr.rs::certify`, tested in `tests/paper_score_mdr.rs` |
| Algorithm 1 (SCoRE-MDR) | `src/selective/mdr.rs::certify` |
| Implied TDR (summing MDR across m test points) | documented only (`src/selective/mdr.rs` module docs); no batch API yet |
| Proposition 4.4 (efficient shortcut for gamma <= alpha) | verified against `risk_adjusted_evalue` by a property test (`src/selective/mdr.rs`'s `score_proposition_4_4_shortcut_matches_general_decision`) but not wired in as a separate code path; see `tasks/todo.md` |
| Remark 4.5 / Theorem 4.6 (gamma > alpha power loss, extra thresholding condition) | Remark 4.5's guidance is documented; Theorem 4.6's extra condition is not implemented — `gamma > alpha` remains accepted (valid per Theorem 4.2) but with undocumented power, see `tasks/todo.md` |
| SDR definition (Equation 2.3) | `src/selective/ebh.rs`, `src/selective/sdr.rs`, tested in `tests/paper_score_sdr.rs` |
| Theorem 3.3 (eBH selection controls SDR) | `src/selective/ebh.rs::select` — deliberately generic over any e-values individually satisfying Definition 3.1, not specific to Equation 5.1 |
| Algorithm 2 (SCoRE-SDR) | `src/selective/sdr.rs::certify`, composing `ebh::select` with Milestone 4's `evalue::risk_adjusted_evalue` applied independently per test point |
| Equation 5.1 / Algorithm 3 (the paper's own cross-test-point-coupled e-value and its efficient computation) | not implemented — Algorithm 3's exact steps were not extractable across several independent, targeted fetches (consistently truncated), and Equation 5.1's normalizing function is a ratio of two non-decreasing-in-`t` quantities, so the monotonicity argument `evalue.rs`'s Equation 4.1 derivation relies on does not obviously extend; see `src/selective/sdr.rs` module docs and `tasks/todo.md`. `sdr::certify` remains valid without it (Theorem 3.3's hypothesis does not require this specific construction) but is presumably less powerful than the paper's own version. |
| Zero-selection behavior | `src/selective/ebh.rs::select` (returns an empty, valid selection when no `tau` qualifies) and `src/selective/sdr.rs::realized_selective_risk` (the `max(1, selected_count)` denominator) |
| Weighted extensions under covariate shift | not yet implemented — deferred within Milestone 6 in favor of the shifted anytime controller (AGENTS.md's backlog item 18 before item 19); see `tasks/todo.md` |

## Implemented so far

**Milestone 0** provides the shared vocabulary every controller builds on:

- validated numeric types — `src/probability.rs`
- the bounded-loss contract — `src/loss.rs`
- the guarantee and assumption taxonomy — `src/guarantee.rs`
- the certificate type — `src/certificate.rs`
- the error taxonomy — `src/error.rs`

**Milestone 1** provides the classical monotone CRC baseline:

- `src/crc/monotone.rs::certify` — Theorem 1, cited above.
- `src/numerics/summation.rs` — compensated summation used to compute the
  empirical risk (AGENTS.md section 8).

**Milestone 2** provides anytime-valid monotone CRC:

- `src/anytime/boundary.rs` — the `h`, `f`, `m*`, and correction-term
  functions from Theorem 4.1.
- `src/anytime/calibration.rs::AnytimeController` — incremental
  empirical-risk state, the uninformative fallback below `m*`, and the
  running-minimum threshold sequence.

**Milestone 3** provides the general non-monotonic reduction (Theorem 1
only; see the table above for what is deferred):

- `src/nonmonotone/stability.rs::certify` — the symmetry + beta-stability
  reduction, generic over any `Parameter` type (including multidimensional
  parameters, with no extra work needed).

**Milestone 4** provides the SCoRE-MDR direct deployment decision
(Definition 3.1, Equation 4.1, Algorithm 1, Theorem 3.2 only; see the
table above for what is deferred):

- `src/selective/evalue.rs::risk_adjusted_evalue` — the risk-adjusted
  e-value construction.
- `src/selective/mdr.rs::certify` — the deployment decision and
  `MarginalDeploymentRisk` certificate.

**Milestone 5** provides batch SCoRE-SDR (Theorem 3.3 and Algorithm 2,
composed from Milestone 4's e-value construction; Equation 5.1/Algorithm
3 deferred — see the table above):

- `src/selective/ebh.rs::select` — the generic eBH selection engine.
- `src/selective/sdr.rs::certify` — the batch entry point and
  `SelectiveDeploymentRisk` certificate.
- `src/selective/sdr.rs::realized_selective_risk` — the post-hoc,
  label-requiring realized-risk helper.

**Milestone 6** provides importance-weighted anytime-valid CRC (Theorem
4.7; weighted SCoRE deferred — see the table above):

- `src/shift/importance.rs::WeightAccumulator` — non-negative finite
  weight validation, compensated running sum/sum-of-squares, min/max,
  Kish effective sample size, and degenerate-weight rejection.
- `src/anytime/boundary.rs::weighted_term` — the corrected boundary
  function (see the table above for the fetched-text correction it
  required).
- `src/anytime/shifted.rs::AnytimeShiftedController` — the shifted
  controller, with `m*` discovered at runtime as a stopping time on the
  realized weights rather than precomputed at build time.

See `guarantees.md` and `assumptions.md` for the vocabulary itself, and
AGENTS.md section 7 for the full milestone sequence.
