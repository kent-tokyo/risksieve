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
| Proposition 1 (monotone CRC is the `beta = 0` special case) | not implemented or validated — checking it would require computing both sides of the beta-stability definition empirically (leave-one-out-average vs. full-sample risk); see `docs/roadmap.md` |
| Proposition 2 (bounded-loss discretization) | not yet implemented — its stated guarantee is an asymptotic `Õ(1/√n)` bound involving the Lambert W function, extracted with lower confidence than Theorem 1; see `docs/roadmap.md` before implementing |
| Proposition 3 / Corollary 2 (continuous Lipschitz losses) | not yet implemented (Milestone 3 follow-up) |
| Proposition 4 / 5 / Corollary 3 (selective classification) | not yet implemented (Milestone 3 follow-up) |
| Proposition 6 / Corollary 4 (regularized ERM) | not yet implemented (Milestone 3 follow-up) |
| Theorem 2, Propositions 7-8 (multivariate / gradient extensions) | not yet implemented — note `nonmonotone::stability::certify`'s `Parameter` type is already unconstrained and generic, so multidimensional parameters work today for Theorem 1 itself |

## SCoRE

Tian Bai and Ying Jin. **Conformal Selective Prediction with General Risk
Control.** 2026. arXiv:2603.24704. <https://arxiv.org/abs/2603.24704>

Reference implementation (MIT): <https://github.com/Tian-Bai/SCoRE>,
commit `401b7caf6d030825ff67e8f08e44ba15ee8c94af` (package version
`0.1.1`; `SCoRE/SCoRE.py` blob SHA
`aa9d111b92fcf574b77f232039410e8a4c23f3f5`) — used as a behavioral test
oracle and for independent cross-checking; this crate reimplements the
paper idiomatically rather than translating the reference line by line.
This paper postdates this project's training-data cutoff; see the
provenance note in `src/selective/evalue.rs` for how Definition 3.1,
Equation 4.1/4.2, Theorem 4.2, and Remark 4.5 were extracted and
cross-checked, and `src/selective/coupled.rs` for Equation 5.1, Theorem
5.1, and Algorithm 3.

| Result | Implemented in |
|---|---|
| Definition 3.1 (risk-adjusted e-values) | `src/selective/evalue.rs::risk_adjusted_evalue`, tested in `tests/paper_score_mdr.rs` |
| Equation 4.1/4.2 (e-value construction) | `src/selective/evalue.rs::risk_adjusted_evalue`, via an exact breakpoint-enumeration reference implementation (see the module docs for the derivation) |
| Theorem 4.2 (e-value validity for any fixed gamma) | relied on but not independently tested beyond `risk_adjusted_evalue`'s own construction |
| MDR definition and Theorem 3.2 (MDR control) | `src/selective/mdr.rs::certify`, tested in `tests/paper_score_mdr.rs` |
| Algorithm 1 (SCoRE-MDR) | `src/selective/mdr.rs::certify` |
| Implied TDR (summing MDR across m test points) | documented only (`src/selective/mdr.rs` module docs); no batch API yet |
| Proposition 4.4 (efficient shortcut for gamma <= alpha) | verified against `risk_adjusted_evalue` by a property test (`src/selective/mdr.rs`'s `score_proposition_4_4_shortcut_matches_general_decision`) but not wired in as a separate code path; see `docs/roadmap.md` |
| Remark 4.5 / Theorem 4.6 (gamma > alpha power loss, extra thresholding condition) | Remark 4.5's guidance is documented; Theorem 4.6's extra condition is not implemented — `gamma > alpha` remains accepted (valid per Theorem 4.2) but with undocumented power, see `docs/roadmap.md` |
| SDR definition (Equation 2.3) | `src/selective/ebh.rs`, `src/selective/sdr.rs`, tested in `tests/paper_score_sdr.rs` |
| Theorem 3.3 (eBH selection controls SDR) | `src/selective/ebh.rs::select` — deliberately generic over any e-values individually satisfying Definition 3.1, works for either e-value construction below |
| Algorithm 2 (SCoRE-SDR), paper-exact e-value | `src/selective/sdr.rs::certify` (default), composing `ebh::select` with `coupled::coupled_risk_adjusted_evalues` (Equation 5.1) |
| Algorithm 2 (SCoRE-SDR), independent e-value | `src/selective/sdr.rs::certify_independent`, composing `ebh::select` with Milestone 4's `evalue::risk_adjusted_evalue` applied independently per test point; kept for comparison and backward compatibility |
| Equation 5.1 / Theorem 5.1 / Algorithm 3 (the paper's own cross-test-point-coupled e-value, its validity, and its efficient computation) | `src/selective/coupled.rs::coupled_risk_adjusted_evalues`, independently derived from the equation (not translated from `SCoRE_SDR`) and cross-checked against it; see "Equation 5.1 audit" below |
| Zero-selection behavior | `src/selective/ebh.rs::select` (returns an empty, valid selection when no `tau` qualifies) and `src/selective/sdr.rs::realized_selective_risk` (the `max(1, selected_count)` denominator) |
| Weighted extensions under covariate shift | not yet implemented — deferred within Milestone 6 in favor of the shifted anytime controller (AGENTS.md's backlog item 18 before item 19); see `docs/roadmap.md` |

### Equation 5.1 audit

Correspondence between the paper's own notation, `SCoRE_SDR`'s variable
names, and this crate's `src/selective/coupled.rs` (the full derivation,
including why a suffix maximum is used and why the objective is clamped
to `[0,1]`, lives in that module's doc comment; this is the compact
summary AGENTS.md's citation policy and this PR's own review process
call for):

| Concept | Paper | `SCoRE_SDR` | This crate |
|---|---|---|---|
| Cumulative calibration loss at/below a threshold | `sum_i L_i * 1{s(X_i)<=t}` | `NUMER[i]` | `calib_prefix[k]` |
| Count of *other* test points at/below a threshold, plus 1 | `1 + sum_{k'!=j} 1{s(X_{n+k'})<=t}` | `DENOM[i] - (Stest[j]<=t)` | `denom_excl_j(k)` |
| Why test point `j` is excluded from its own denominator | its own indicator is priced by the numerator's `l * 1{...}` term instead, which the infimum varies; counting it in the denominator too would double-count the same indicator | (same reasoning, implicit) | see `coupled.rs` module docs |
| `FR_0`, `FR_1` | `FR_{n+j}(t;0)`, `FR_{n+j}(t;1)` | `FR_0[i]`, `FR_1[i]` | `fr0_feasible`/`fr1_feasible` (cross-multiplied comparisons, avoiding a division) |
| `ell` / breakpoint candidate | Algorithm 3's own `l-bar(t)` (`(n+1)*gamma/m*(...) - sum L_i*1{...}`, see the paper's Section 5) | `ELL[i]` | `ell_bar(k)` |
| `t_0`, `t_1` | `t_{gamma,n+j}(0)`, `t_{gamma,n+j}(1)` | `t_0`, `t_1` | `t0`, `t1` |
| Candidate set `M_star` | thresholds surviving Algorithm 3's pruning | `M_star` | the `k` range scanned after the suffix-maximum filter |
| Why a suffix maximum | a threshold's breakpoint is irrelevant once a strictly larger, also-feasible threshold has an equal-or-larger breakpoint (that larger threshold already covers every `l` the smaller one would have been optimal for) | `max_ell` (backward pass) | `suffix_max_ell` (backward pass) |
| Final e-value | `E_{gamma,n+j}` | `evalues[j]` | the returned `NonNegative` per test point |
| Composition with eBH | Theorem 3.3 applied to `{E_{gamma,n+j}}` | `eBH(evalues, alpha)` | `sdr::certify` calling `ebh::select` |
| Exchangeability required | `{(X_i,Y_i)}_{i=1}^{n+m}` jointly | (assumed by the caller) | documented in `sdr.rs` and `coupled.rs` module docs |

**Differences from the official implementation, found by construction and
confirmed numerically, not guessed:**

- **`ell_bar` clamping.** Equation 5.1's infimum ranges over `l in [0,1]`,
  but `SCoRE_SDR` evaluates its objective at the raw, unclamped
  `ell_bar(t)`, which can exceed `1` (confirmed: `5,142` such events across
  `50,000` randomized/adversarial trials, seed `7`). This crate clamps
  `ell_bar(t)` to `[0,1]` before evaluating the objective, matching the
  equation's stated domain. The same 50,000-trial comparison found
  **zero** resulting differences in the final e-value or selected set
  between the clamped and unclamped versions — the divergence exists in
  the code but was not observed to change any output. Reproduce with:

  ```bash
  python3 scripts/audits/compare_score_reference.py --repo /path/to/Tian-Bai/SCoRE/checkout
  ```
- **`gamma`'s domain.** The paper states Theorem 5.1 for any `gamma > 0`
  with no upper bound (Section 5), strictly wider than Equation 4.1's
  `gamma in (0,1)` (Theorem 4.2) — Equation 5.1's normalizer carries an
  extra `m/(n+1)` scale factor Equation 4.1 does not. `SCoRE_SDR`'s own
  `_validate_gamma`, however, rejects `gamma > 1`. This crate follows the
  official implementation's narrower `(0,1)` domain (via `OpenUnitInterval`,
  the same type Equation 4.1 already uses) rather than the paper's wider
  one, for three reasons: the oracle fixtures this module is cross-checked
  against cannot exercise `gamma > 1` either, since `SCoRE_SDR` itself
  refuses it; the paper's own recommended default (`gamma = alpha`) is
  always in `(0,1)`; and it keeps `gamma`'s type consistent across
  `evalue::risk_adjusted_evalue`, `sdr::certify_independent`, and
  `sdr::certify`. At `gamma = 0` exactly, the coupled e-value's true
  mathematical infimum can be `+infinity` (every `M_star` candidate's
  denominator is `gamma*(n+1)/m*denom_excl_j`, which vanishes only at
  `gamma = 0`) — `OpenUnitInterval` already excludes this endpoint, which
  is why no dedicated infinite-e-value type was introduced. A constructed
  scenario stressing the `m/(n+1)` scaling (large `m`, few test points
  below the calibration region) did not find a case where allowing `gamma`
  up to `5.0` changed the selected set relative to capping it at `1.0`;
  see `src/selective/coupled.rs`'s module docs for the numbers.
- **`SCoRE_MDR_bf` (the official brute-force reference for Equation 4.1)
  is incomplete, not just differently-scoped.** It evaluates its objective
  at exactly `l in {0, 1}`, missing the interior breakpoints Equation
  4.1's infimum requires in general (this crate's own
  `risk_adjusted_evalue` sweeps all of them). A 5,000-trial comparison
  (seed `123`) against a from-scratch full breakpoint enumeration found
  1,380 mismatches (`27.6%`), including cases where `SCoRE_MDR_bf` reports
  a badly wrong finite value: the smallest reproducer found (`n=1`,
  `Lcalib=[0.0538...]`, `Scalib=[-1.6778...]`, `Stest_j=-1.8938...`,
  `gamma=0.2083...`) has `SCoRE_MDR_bf` report `37.16...` where the true
  value is `4.80...` — reproduce with the same command as above (the
  `mdr-bf` comparison runs alongside the `clamp` one by default).
  Consequence: this crate's oracle fixture
  (`tests/fixtures/score_sdr_v0_1_1.json`) has no independent-construction
  column sourced from `SCoRE_MDR_bf` — `certify_independent`
  /`risk_adjusted_evalue` were already validated in a prior milestone via
  hand-derivation and property tests, and are cross-checked against the
  coupled construction purely in Rust
  (`tests/paper_score_sdr.rs::score_algorithm_2_sdr_coupled_and_independent_can_disagree`),
  not against this known-incomplete function.

**Selection-power comparison (measured, not asserted):** the coupled and
independent constructions select different sets on some inputs (see the
fixture above: coupled selects a test point the independent construction
does not, because the coupled denominator only has to account for the
test batch's own size, one point of which is excluded as "self", while
the independent denominator gets no such adjustment). On other inputs
(for example, several test points tied at an identical score) they
coincide, by symmetry. Neither the paper nor this crate proves the
coupled construction dominates the independent one in general — see
`src/selective/sdr.rs`'s module docs and `docs/roadmap.md` for what a
systematic power study would still need to check.

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

**Milestone 5** provides batch SCoRE-SDR, including the paper's own
cross-test-point-coupled e-value (Theorem 3.3, Algorithm 2, Equation 5.1,
Theorem 5.1; randomized pruning and weighted SDR deferred — see
`docs/roadmap.md`):

- `src/selective/ebh.rs::select` — the generic eBH selection engine.
- `src/selective/coupled.rs::coupled_risk_adjusted_evalues` — the
  paper-exact, cross-test-point-coupled e-value construction (Equation
  5.1 / Algorithm 3).
- `src/selective/sdr.rs::certify` — the default batch entry point (uses
  the coupled construction) and `SelectiveDeploymentRisk` certificate.
- `src/selective/sdr.rs::certify_independent` — the same entry point using
  Milestone 4's e-value construction applied independently per test
  point, kept for comparison and backward compatibility.
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
