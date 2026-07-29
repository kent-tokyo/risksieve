# Third-party notices

## `Tian-Bai/SCoRE` — behavioral oracle for the coupled SDR construction

- **Repository:** <https://github.com/Tian-Bai/SCoRE>
- **Commit:** `401b7caf6d030825ff67e8f08e44ba15ee8c94af`
- **Package version:** `0.1.1`
- **Source file:** `SCoRE/SCoRE.py`
  (blob SHA `aa9d111b92fcf574b77f232039410e8a4c23f3f5`), specifically the
  `SCoRE_SDR` function.
- **License:** MIT.
- **Copyright:** Copyright (c) 2026 Tian Bai and Ying Jin.
- **What was used:** `SCoRE_SDR`'s numeric *output* was used to generate
  the cross-language oracle fixture
  `tests/fixtures/score_sdr_v0_1_1.json` (via
  `scripts/oracles/generate_score_sdr.py`), and its source was read to
  understand Algorithm 3's computational structure (the `NUMER`/`DENOM`
  prefix sums, the `FR_0`/`FR_1`/`ELL`/`t_0`/`t_1`/`M_star` quantities)
  where Algorithm 3's own text in the paper was not fully recoverable by
  fetch (see `docs/references.md`'s provenance note).
- **How this crate restructured it:** `src/selective/coupled.rs` is an
  independent derivation from Equation 5.1 (the paper's formula), not a
  line-by-line translation of `SCoRE_SDR`. Concretely:
  - Pooled calibration-and-test scores are grouped into distinct sorted
    values with an aggregated loss sum and test count *before* any prefix
    sum is computed, rather than sorting a tuple list with duplicates and
    correcting for ties in a second pass (`SCoRE_SDR`'s `M_tagged.sort()`
    followed by its tie-correction loop).
  - The `FR_0`/`FR_1` feasibility checks are computed via cross-multiplied
    integer/loss comparisons rather than a floating-point division
    compared against `gamma`, to avoid an extra source of rounding error.
  - The `ell_bar`/`ELL` breakpoint value is clamped to `[0.0, 1.0]` before
    being plugged into the objective, matching Equation 5.1's stated
    domain (`inf_{l in [0,1]}`); `SCoRE_SDR` does not clamp it. This is a
    deliberate, documented divergence — see `src/selective/coupled.rs`'s
    module docs and `docs/references.md` for the numerical comparison
    (50,000 trials, zero resulting output differences) that motivated
    keeping the clamped version anyway.
  - Numeric comments and docstrings in `SCoRE/SCoRE.py` were read for
    understanding but not copied; all comments and documentation in
    `src/selective/coupled.rs` are original.
- **Fixture provenance:** `tests/fixtures/score_sdr_v0_1_1.json`'s own
  `provenance` object repeats the repository, commit, package version,
  blob SHA, license, and copyright above, plus the generator seed
  (`20260729`) and generation date. `tests/score_sdr_oracle.rs` reads only
  this committed JSON; Python is never invoked by `cargo test` or CI.
- **Comparison script not committed:** the ad hoc script that ran the
  50,000-trial `ell_bar`-clamping comparison and the 5,000-trial
  `SCoRE_MDR_bf` completeness check (both cited in
  `docs/references.md`) was a throwaway analysis script, not committed to
  this repository; the trial counts, seeds, and findings it produced are
  recorded in `docs/references.md` and in this file so the claims remain
  auditable even without the script itself.

## Historical note

No other code or fixtures from external repositories have been adapted
into `risksieve` as of this writing. This file will record, for each
future adaptation:

- the original file and commit;
- its license;
- the required notice;
- an explanation of the transformation;
- confirmation that the original copyright notice was preserved.

## Reference repositories and their licensing status

For context when that day comes (AGENTS.md section 11):

- **Foundational CRC** reference repository: MIT. Adaptation permitted
  subject to license and attribution requirements.
- **SCoRE** (<https://github.com/Tian-Bai/SCoRE>): MIT. Adaptation
  permitted subject to license and attribution requirements. May also be
  used as a behavioral test oracle.
- **Non-monotonic CRC** (<https://github.com/aangelopoulos/nonmonotonic-crc>):
  README displays an MIT badge, but no root `LICENSE` file was found
  during the initial project review. Treated as all-rights-reserved for
  code-reuse purposes until clarified. Only independently generated
  fixtures may be used from this repository; its source must not be
  copied or translated.
- **Anytime-valid CRC**: no official implementation was identified in the
  paper during the initial review; implemented independently from the
  paper.

Citing a paper and independently implementing its mathematical method is
not the same as reusing its prose, figures, notebooks, or source code; see
AGENTS.md section 11 for the full clean-room policy.
