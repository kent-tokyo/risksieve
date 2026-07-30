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
unweighted case. `official_selected_indices` is therefore only compared
against for fixtures with `gamma <= alpha`, where this crate's own
`risk_adjusted_evalue` already has a property test
(`score_proposition_4_4_shortcut_matches_general_decision`) proving the
analogous unweighted shortcut matches the general computation exactly --
the weighted shortcut has the identical algebraic structure. Fixtures
with `gamma > alpha` are still generated (this crate's `certify_weighted`
does not implement the extra Theorem-4.6-style condition, matching the
already-documented unweighted limitation), but their
`official_selected_indices` is not asserted against in the Rust test,
only recorded for reference (see `gamma_le_alpha` per case).

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


def weighted_evalue_reference(Lcalib, Scalib, Wcalib, s_test, w_test, gamma):
    """Independent from-scratch reference for Equation 6.1's e-value via
    exact breakpoint enumeration over l in [0,1]. Returns None if the
    combined calibration+test weight is exactly zero (degenerate)."""
    import numpy as np

    Lcalib = np.asarray(Lcalib, dtype=float)
    Scalib = np.asarray(Scalib, dtype=float)
    Wcalib = np.asarray(Wcalib, dtype=float)

    total_weight = float(np.sum(Wcalib) + w_test)
    if total_weight == 0.0:
        return None

    pooled = np.concatenate([Scalib, [s_test]])

    def weighted_base_sum(t):
        return float(np.sum(Wcalib * Lcalib * (Scalib <= t)))

    gamma_scaled = gamma * total_weight

    def t_gamma(ell):
        max_t, feasible = -np.inf, False
        for t in pooled:
            below = s_test <= t
            contribution = weighted_base_sum(t) + (w_test * ell if below else 0.0)
            if contribution <= gamma_scaled:
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

    # 15. gamma > alpha: the official shortcut's decision is recorded but
    #     NOT asserted against (see the module docs and gamma_le_alpha) --
    #     certify_weighted does not implement the extra Theorem-4.6-style
    #     condition SCoRE_MDR_w checks in this regime, matching this
    #     crate's already-documented unweighted limitation.
    add(
        "gamma_greater_than_alpha_not_compared",
        "gamma > alpha: reference_evalues is still checked, but "
        "official_selected_indices is recorded for reference only, not "
        "asserted against, since certify_weighted does not implement the "
        "extra thresholding condition SCoRE_MDR_w applies in this regime.",
        Lcalib=[0.3, 0.7, 0.1], Scalib=[-1.0, 0.5, 2.0], Wcalib=[1.0, 2.0, 1.0],
        Stest=[-0.5, 1.0], Wtest=[1.0, 1.0], alpha=0.3, gamma=0.7,
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
        gamma = float(rng.uniform(0.01, alpha))  # keep within the shortcut's unconditional regime

        case = evaluate_case(SCoRE_MDR_w, Lcalib, Scalib, Wcalib, Stest, Wtest, alpha, gamma)
        case["name"] = f"random_seed_{seed}_index_{i}"
        case["description"] = (
            f"Randomized fixture #{i} from numpy.random.default_rng({seed}); "
            "even indices use a small discrete score grid to keep ties common; "
            "gamma drawn <= alpha to stay in SCoRE_MDR_w's unconditionally-valid regime."
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
                "official_selected_indices comes from SCoRE_MDR_w (a "
                "decision-only shortcut, gamma<=alpha unconditionally "
                "valid -- see gamma_le_alpha per case). reference_evalues "
                "comes from this script's own from-scratch breakpoint-"
                "enumeration implementation of Equation 6.1, NOT from an "
                "official function (no official weighted brute-force "
                "e-value function exists) and NOT derived from "
                "src/selective/evalue_weighted.rs -- see "
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
