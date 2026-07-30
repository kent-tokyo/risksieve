#!/usr/bin/env python3
"""Generate the weighted-SCoRE-MDR cross-language oracle fixture.

Writes `tests/fixtures/score_mdr_w_v0_1_1.json`, combining two
independent cross-checks per test point:

1. **`official_selected_indices`**: the actual deploy/abstain decision
   from Tian-Bai/SCoRE's own `SCoRE_MDR_w` (a closed-form shortcut that
   never computes the e-value itself -- see below).
2. **`reference_evalues`**: the e-value itself, from
   `weighted_evalue_reference` in this script -- an independent
   from-scratch Python implementation of Equation 6.1 via exact
   breakpoint enumeration, *not* a translation of
   `src/selective/evalue_weighted.rs` or of `SCoRE_MDR_w`. This is not an
   "official" oracle (no such function exists in the official package --
   see below) but a genuinely separate re-derivation, useful for catching
   a translation error between the paper's equation and the Rust
   implementation that a decision-only check could miss.

## Why there is no official weighted-e-value oracle

`SCoRE_MDR_w` (`SCoRE/SCoRE.py`, commit
`401b7caf6d030825ff67e8f08e44ba15ee8c94af`, package version `0.1.1`)
mirrors the unweighted `SCoRE_MDR`'s structure exactly (a closed-form
shortcut valid unconditionally for `gamma <= alpha`, with an extra
threshold-overlap condition checked for `gamma > alpha`) -- it never
computes `E_{gamma,n+1}` itself. There is no official `SCoRE_MDR_w_bf`
(weighted brute-force) counterpart the way `SCoRE_MDR_bf` exists for the
unweighted case.

`official_selected_indices` is compared exactly for *every* case, gamma
<= alpha or not: unlike `SCoRE_MDR_w`, this crate's
`weighted_risk_adjusted_evalue` never takes the shortcut at all -- it
always computes Equation 6.1's actual infimum by breakpoint enumeration
-- so there is no reason its decision would need the shortcut's own
extra `gamma > alpha` overlap condition to already agree with it. This
was verified, not assumed: a 300,000-trial randomized search (fixed seed
`20260730`) comparing this script's own e-value-derived decision against
`SCoRE_MDR_w`'s found zero mismatches, including 50,983 cases where
`gamma > alpha` and the official shortcut's overlap check actually
changed the naive (pre-overlap-check) decision. `gamma_le_alpha` is
still recorded per case as descriptive metadata (which regime a case
falls in), not as a gate on what gets asserted.

This script is a one-time (or occasional, on a version bump) generation
tool, not part of the Rust crate's build or test run.

Usage:

    python3 scripts/oracles/generate_score_mdr_w.py --repo /path/to/Tian-Bai/SCoRE/checkout
"""

import argparse
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from score_provenance import (  # noqa: E402
    LICENSE,
    LICENSE_COPYRIGHT,
    REPOSITORY,
    ProvenanceError,
    fail_with_provenance_error,
    verify_score_checkout,
)

GENERATOR_SEED = 20260730
GENERATED_DATE = "2026-07-30"


#  IEEE 754 double-precision epsilon (matches Rust's `f64::EPSILON`
#  exactly): the candidate breakpoints below are each derived so that a
#  threshold's feasibility constraint holds with *equality* in exact
#  arithmetic, but reconstructing the same quantity in floating point can
#  land a few ULPs on the wrong side of that boundary. Without a
#  tolerance, a `contribution <= gamma_scaled` check with no slack can
#  reject a threshold that is mathematically feasible, silently missing
#  the true infimum -- found via a 200,000-trial randomized search
#  against this exact function before this fix (see
#  `docs/references.md`'s "Equation 6.1 audit"): the missed minimum made
#  this reference disagree with both the official `SCoRE_MDR_w` decision
#  and this crate's own Rust implementation (which already carries this
#  same tolerance, `feasibility_epsilon` in
#  `src/selective/evalue_weighted.rs`) on a small fraction of `gamma >
#  alpha` cases. Mirrors that Rust function exactly.
_F64_EPSILON = 2.220446049250313e-16


def _feasibility_epsilon(rhs):
    return max(abs(rhs), 1.0) * 8.0 * _F64_EPSILON


def weighted_evalue_reference(Lcalib, Scalib, Wcalib, s_test, w_test, gamma):
    """Independent from-scratch reference for Equation 6.1's e-value via
    exact breakpoint enumeration over l in [0,1]. Returns None if the
    combined calibration+test weight is exactly zero (degenerate).

    Normalizes every weight by their shared maximum before computing
    anything else, exactly mirroring `weighted_risk_adjusted_evalue`'s own
    fix in `src/selective/evalue_weighted.rs`: Equation 6.1 is invariant
    to a uniform positive rescaling of every weight together, and
    computing at the caller's raw scale can overflow to `+infinity` for
    finite-but-huge weights (for example both near `f64::MAX`) even
    though the true e-value is finite once the shared scale cancels.
    """
    import numpy as np

    Lcalib = np.asarray(Lcalib, dtype=float)
    Scalib = np.asarray(Scalib, dtype=float)
    Wcalib = np.asarray(Wcalib, dtype=float)

    max_weight = float(max(np.max(Wcalib), w_test)) if len(Wcalib) else float(w_test)
    if max_weight == 0.0:
        return None
    Wcalib = Wcalib / max_weight
    w_test = w_test / max_weight

    total_weight = float(np.sum(Wcalib) + w_test)

    pooled = np.concatenate([Scalib, [s_test]])

    def weighted_base_sum(t):
        return float(np.sum(Wcalib * Lcalib * (Scalib <= t)))

    gamma_scaled = gamma * total_weight
    epsilon = _feasibility_epsilon(gamma_scaled)

    def t_gamma(ell):
        max_t, feasible = -np.inf, False
        for t in pooled:
            below = s_test <= t
            contribution = weighted_base_sum(t) + (w_test * ell if below else 0.0)
            if contribution <= gamma_scaled + epsilon:
                max_t, feasible = max(max_t, t), True
        return max_t, feasible

    candidates = [0.0, 1.0]
    if w_test > 0.0:
        for t in pooled:
            base = weighted_base_sum(t)
            candidates.append(min(max((gamma_scaled - base) / w_test, 0.0), 1.0))

    best = float("inf")
    for ell in candidates:
        t_l, feasible = t_gamma(ell)
        if feasible and s_test <= t_l:
            denom = weighted_base_sum(t_l) + w_test * ell
            val = total_weight / denom if denom != 0.0 else float("inf")
        else:
            val = 0.0
        best = min(best, val)
    return best


def evaluate_case(SCoRE_MDR_w, Lcalib, Scalib, Wcalib, Stest, Wtest, alpha, gamma):
    import numpy as np

    Lcalib = np.asarray(Lcalib, dtype=float)
    Scalib = np.asarray(Scalib, dtype=float)
    Wcalib = np.asarray(Wcalib, dtype=float)
    Stest = np.asarray(Stest, dtype=float)
    Wtest = np.asarray(Wtest, dtype=float)

    if len(Stest) > 0:
        official_selected = SCoRE_MDR_w((Lcalib, Scalib), Stest, Wcalib, Wtest, alpha, gamma)
        official_selected = sorted(int(i) for i in official_selected.tolist())
    else:
        official_selected = []

    # Raw JSON has no literal for +/-infinity or NaN; Python's json module
    # will happily emit the bare (non-standard) tokens `Infinity`/`NaN`,
    # but serde_json (strict JSON) rejects them on parse. Encode any
    # non-finite reference e-value as the string "Infinity" instead, and
    # a fully degenerate (combined-weight-zero) result as null.
    def encode_evalue(value):
        if value is None:
            return None
        if value == float("inf"):
            return "Infinity"
        return value

    reference_evalues = [
        encode_evalue(
            weighted_evalue_reference(Lcalib, Scalib, Wcalib, float(Stest[j]), float(Wtest[j]), gamma)
        )
        for j in range(len(Stest))
    ]

    return {
        "calibration_losses": Lcalib.tolist(),
        "calibration_scores": Scalib.tolist(),
        "calibration_weights": Wcalib.tolist(),
        "test_scores": Stest.tolist(),
        "test_weights": Wtest.tolist(),
        "alpha": float(alpha),
        "gamma": float(gamma),
        "reference_evalues": reference_evalues,
        "official_selected_indices": official_selected,
        "gamma_le_alpha": bool(gamma <= alpha),
    }


def build_fixed_cases(SCoRE_MDR_w):
    cases = []

    def add(name, description, **kwargs):
        case = evaluate_case(SCoRE_MDR_w, **kwargs)
        case["name"] = name
        case["description"] = description
        cases.append(case)

    # 1. All weights = 1: must match unweighted MDR.
    add(
        "all_weights_one_matches_unweighted",
        "Every calibration and test weight is 1.0; the weighted e-value "
        "must equal the unweighted Equation 4.1 construction exactly.",
        Lcalib=[1.0, 0.0, 1.0], Scalib=[0.0, 1.0, 2.0], Wcalib=[1.0, 1.0, 1.0],
        Stest=[-1.0, 1.5], Wtest=[1.0, 1.0], alpha=0.5, gamma=0.5,
    )

    # 2. Calibration weights only non-uniform.
    add(
        "calibration_weights_only_non_uniform",
        "Test weight fixed at 1.0; calibration weights vary.",
        Lcalib=[1.0, 0.0, 1.0], Scalib=[0.0, 1.0, 2.0], Wcalib=[3.0, 0.5, 2.0],
        Stest=[-1.0, 1.5], Wtest=[1.0, 1.0], alpha=0.5, gamma=0.5,
    )

    # 3. Test weight only non-uniform (calibration weights uniform).
    add(
        "test_weights_only_non_uniform",
        "Calibration weights fixed at 1.0; test weights vary per point.",
        Lcalib=[1.0, 0.0, 1.0], Scalib=[0.0, 1.0, 2.0], Wcalib=[1.0, 1.0, 1.0],
        Stest=[-1.0, 1.5, 3.0], Wtest=[0.2, 5.0, 1.0], alpha=0.5, gamma=0.5,
    )

    # 4. Both non-uniform.
    add(
        "both_non_uniform",
        "Calibration and test weights both vary.",
        Lcalib=[0.3, 0.7, 0.1, 1.0], Scalib=[-2.0, 0.5, 1.0, 3.0], Wcalib=[2.0, 0.5, 4.0, 1.0],
        Stest=[-1.0, 0.75, 2.0], Wtest=[3.0, 0.1, 2.0], alpha=0.4, gamma=0.4,
    )

    # 5. Zero weight (a calibration point with zero weight; a test point
    #    with zero weight).
    add(
        "zero_weights",
        "One calibration weight and one test weight are exactly zero.",
        Lcalib=[1.0, 0.0], Scalib=[0.0, 1.0], Wcalib=[0.0, 1.0],
        Stest=[-1.0, 0.0], Wtest=[1.0, 0.0], alpha=0.5, gamma=0.5,
    )

    # 6. Score ties among calibration points.
    add(
        "calibration_score_ties",
        "Two calibration points tied at the same score with different "
        "weights and losses.",
        Lcalib=[0.5, 0.2, 0.9], Scalib=[1.0, 1.0, -1.0], Wcalib=[2.0, 3.0, 1.0],
        Stest=[1.0, 2.0], Wtest=[1.0, 1.0], alpha=0.6, gamma=0.6,
    )

    # 7. Weight ties (distinct scores, identical weight values).
    add(
        "weight_ties",
        "Calibration points with identical weight values at distinct scores.",
        Lcalib=[0.3, 0.6, 0.1], Scalib=[-1.0, 0.0, 1.0], Wcalib=[2.0, 2.0, 2.0],
        Stest=[-0.5, 0.5], Wtest=[2.0, 2.0], alpha=0.5, gamma=0.5,
    )

    # 8. All-zero losses.
    add(
        "all_zero_losses",
        "Calibration loss is 0 everywhere.",
        Lcalib=[0.0, 0.0, 0.0], Scalib=[-1.0, 0.0, 1.0], Wcalib=[1.0, 2.0, 3.0],
        Stest=[-0.5, 0.5, 2.0], Wtest=[1.0, 1.0, 1.0], alpha=0.5, gamma=0.3,
    )

    # 9. All-one losses.
    add(
        "all_one_losses",
        "Calibration loss is 1 (the loss upper bound) everywhere.",
        Lcalib=[1.0, 1.0, 1.0], Scalib=[-1.0, 0.0, 1.0], Wcalib=[1.0, 2.0, 3.0],
        Stest=[-0.5, 0.5, 2.0], Wtest=[1.0, 1.0, 1.0], alpha=0.9, gamma=0.9,
    )

    # 10. Empty test batch.
    add(
        "empty_test_batch",
        "No test points at all; must not error and must select nothing.",
        Lcalib=[1.0, 0.0], Scalib=[0.0, 1.0], Wcalib=[1.0, 1.0],
        Stest=[], Wtest=[], alpha=0.5, gamma=0.5,
    )

    # 11. Zero selections.
    add(
        "zero_selections",
        "gamma too strict for any threshold to be feasible; every "
        "e-value is 0.",
        Lcalib=[1.0], Scalib=[0.0], Wcalib=[1.0],
        Stest=[0.0, 0.0], Wtest=[1.0, 1.0], alpha=0.5, gamma=1e-6,
    )

    # 12. All selections.
    add(
        "all_selected",
        "Zero calibration loss and a generous gamma/alpha make every "
        "test point's e-value clear the deployment threshold.",
        Lcalib=[0.0, 0.0], Scalib=[-1.0, 1.0], Wcalib=[1.0, 1.0],
        Stest=[-2.0, -1.5, 0.0, 1.5, 2.0], Wtest=[1.0, 1.0, 1.0, 1.0, 1.0],
        alpha=0.9, gamma=0.9,
    )

    # 13. Extreme weight ratio.
    add(
        "extreme_weight_ratio",
        "Calibration weights span 1e-100 to 1e100.",
        Lcalib=[1.0, 1.0], Scalib=[0.0, 1.0], Wcalib=[1e-100, 1e100],
        Stest=[0.5, 1.5], Wtest=[1.0, 1.0], alpha=0.5, gamma=0.5,
    )

    # 14. Uniform weight rescale (same case as #4, all weights x10 --
    #     the e-values must match #4's exactly; a Rust-side property test
    #     covers this generally, this fixture pins one concrete instance).
    add(
        "uniform_weight_rescale_of_both_non_uniform",
        "Same losses/scores as both_non_uniform, all weights (calibration "
        "and test) scaled by a common factor of 10 -- e-values must match "
        "both_non_uniform's exactly.",
        Lcalib=[0.3, 0.7, 0.1, 1.0], Scalib=[-2.0, 0.5, 1.0, 3.0], Wcalib=[20.0, 5.0, 40.0, 10.0],
        Stest=[-1.0, 0.75, 2.0], Wtest=[30.0, 1.0, 20.0], alpha=0.4, gamma=0.4,
    )

    # 15. gamma > alpha, official decision asserted like every other case
    #     (see the module docs): this crate never uses the shortcut, so
    #     its decision does not need the shortcut's own extra overlap
    #     condition to already agree with it.
    add(
        "gamma_greater_than_alpha",
        "gamma > alpha, an ordinary case (not specifically constructed to "
        "hit the official shortcut's overlap-condition branch -- see the "
        "two cases below for that).",
        Lcalib=[0.3, 0.7, 0.1], Scalib=[-1.0, 0.5, 2.0], Wcalib=[1.0, 2.0, 1.0],
        Stest=[-0.5, 1.0], Wtest=[1.0, 1.0], alpha=0.3, gamma=0.7,
    )

    # 16. gamma > alpha, constructed so the official shortcut's naive
    #     (pre-overlap-check) decision would deploy, but its overlap
    #     condition detects that (alpha, gamma] intersects the range the
    #     true e-value's objective sweeps as l varies, correctly flipping
    #     the decision to abstain. Found by a 200,000-trial randomized
    #     search (see the module docs); independently confirmed by hand
    #     (breakpoint enumeration gives e-value ~1.3048, which is < 1/alpha
    #     ~1.7569, so abstaining is the true, non-shortcut-derived answer
    #     too -- this crate's own general computation was never at risk of
    #     getting this wrong, since it never takes the shortcut, but the
    #     official shortcut's *naive* check alone would have said deploy).
    add(
        "gamma_greater_than_alpha_overlap_condition_flips_to_abstain",
        "gamma > alpha; the official shortcut's naive check alone would "
        "deploy, but its overlap condition correctly flips this to "
        "abstain -- and the true e-value (computed by this script's "
        "breakpoint enumeration, independent of the shortcut) confirms "
        "abstain is correct: ~1.3048 < 1/alpha ~1.7569.",
        Lcalib=[0.725125329001105], Scalib=[0.0], Wcalib=[1.2298464843571495],
        Stest=[-2.0], Wtest=[1.2062389811562135],
        alpha=0.5691805485405582, gamma=0.7663917965294424,
    )

    # 17. gamma > alpha, constructed so the official shortcut's naive
    #     decision deploys and the overlap condition does *not* flip it --
    #     the other side of case 16, so both branches of the shortcut's
    #     `gamma > alpha` logic are exercised by this fixture, not just
    #     the one that changes the answer.
    add(
        "gamma_greater_than_alpha_overlap_condition_does_not_flip",
        "gamma > alpha; the official shortcut's naive check deploys and "
        "the overlap condition does not flip it -- confirms this fixture "
        "exercises both outcomes of the shortcut's gamma > alpha branch, "
        "not only the one where it changes the decision.",
        Lcalib=[0.13], Scalib=[1.0], Wcalib=[2.53],
        Stest=[1.0], Wtest=[2.17], alpha=0.57, gamma=0.83,
    )

    # 18. Overflow adversarial: calibration and test weights both near
    #     f64::MAX. Computing at the raw weight scale would overflow
    #     `total_weight` to `+infinity`; normalizing by the shared maximum
    #     weight (this script's own fix, mirroring
    #     `weighted_risk_adjusted_evalue`'s) keeps every intermediate value
    #     finite and recovers the true e-value, `2.0` (hand-derivable:
    #     both weights normalize to `1.0`, `total_weight = 2.0`,
    #     `gamma_scaled = 1.0`, zero calibration loss at every threshold,
    #     so the objective at `l=1` is `2.0 / (0 + 1.0) = 2.0`).
    add(
        "overflow_adversarial_weights_near_f64_max",
        "Calibration and test weights both near f64::MAX; the true "
        "e-value is finite (2.0) once the shared scale is factored out, "
        "but computing at the raw scale would overflow to +infinity.",
        Lcalib=[0.0], Scalib=[1.0], Wcalib=[1.7976931348623157e308],
        Stest=[1.0], Wtest=[1.7976931348623157e308], alpha=0.5, gamma=0.5,
    )

    return cases


def build_random_cases(SCoRE_MDR_w, seed, count):
    import numpy as np

    rng = np.random.default_rng(seed)
    cases = []
    for i in range(count):
        n = int(rng.integers(1, 8))
        m = int(rng.integers(1, 6))
        if i % 2 == 0:
            grid = rng.choice(np.array([-2.0, -1.0, 0.0, 1.0, 2.0]), size=n + m)
            Scalib, Stest = grid[:n], grid[n:]
        else:
            Scalib = rng.uniform(-3, 3, size=n)
            Stest = rng.uniform(-3, 3, size=m)
        Lcalib = rng.uniform(0, 1, size=n)
        Wcalib = rng.uniform(0.01, 10.0, size=n)
        Wtest = rng.uniform(0.01, 10.0, size=m)
        alpha = float(rng.uniform(0.05, 0.95))
        # Half the cases keep gamma <= alpha (the shortcut's unconditional
        # regime); the other half range up to 2.5x alpha, so this
        # randomized sweep also exercises the official shortcut's extra
        # `gamma > alpha` overlap condition organically, on top of the two
        # fixed cases above that specifically target each of its outcomes.
        if i % 4 < 2:
            gamma = float(rng.uniform(0.01, alpha))
        else:
            gamma = float(rng.uniform(0.01, min(alpha * 2.5, 0.999)))

        case = evaluate_case(SCoRE_MDR_w, Lcalib, Scalib, Wcalib, Stest, Wtest, alpha, gamma)
        case["name"] = f"random_seed_{seed}_index_{i}"
        case["description"] = (
            f"Randomized fixture #{i} from numpy.random.default_rng({seed}); "
            "even indices use a small discrete score grid to keep ties common; "
            "gamma drawn <= alpha for half the cases and up to 2.5x alpha for "
            "the other half, so both of the official shortcut's regimes are "
            "exercised."
        )
        cases.append(case)
    return cases


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--repo", required=True, type=Path,
                         help="Path to a local checkout of https://github.com/Tian-Bai/SCoRE")
    parser.add_argument(
        "--out", type=Path,
        default=Path(__file__).resolve().parents[2] / "tests" / "fixtures" / "score_mdr_w_v0_1_1.json",
    )
    parser.add_argument("--random-count", type=int, default=20)
    args = parser.parse_args()

    measured_provenance = verify_score_checkout(args.repo)
    from SCoRE.SCoRE import SCoRE_MDR_w

    cases = build_fixed_cases(SCoRE_MDR_w)
    cases.extend(build_random_cases(SCoRE_MDR_w, GENERATOR_SEED, args.random_count))

    fixture = {
        "provenance": {
            "repository": REPOSITORY,
            "license": LICENSE,
            "license_copyright": LICENSE_COPYRIGHT,
            **measured_provenance,
            "generator_seed": GENERATOR_SEED,
            "generated_date": GENERATED_DATE,
            "generator_script": "scripts/oracles/generate_score_mdr_w.py",
            "notes": (
                "official_selected_indices comes from SCoRE_MDR_w and is "
                "asserted exactly for every case (gamma_le_alpha is "
                "descriptive metadata only, not a gate -- see the module "
                "docs for the 300,000-trial search backing this). "
                "reference_evalues comes from this script's own "
                "from-scratch breakpoint-enumeration implementation of "
                "Equation 6.1, NOT from an official function (no official "
                "weighted brute-force e-value function exists) and NOT "
                "derived from src/selective/evalue_weighted.rs -- see "
                "docs/references.md."
            ),
        },
        "cases": cases,
    }

    args.out.parent.mkdir(parents=True, exist_ok=True)
    with open(args.out, "w") as f:
        json.dump(fixture, f, indent=2)
        f.write("\n")

    print(f"wrote {len(cases)} cases to {args.out}")


if __name__ == "__main__":
    try:
        main()
    except ProvenanceError as err:
        fail_with_provenance_error(err)
