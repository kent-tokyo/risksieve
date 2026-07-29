#!/usr/bin/env python3
"""Generate the SCoRE-SDR cross-language oracle fixture.

Runs Tian-Bai/SCoRE's own `SCoRE_SDR` (the coupled, Equation-5.1 e-value)
against a fixed set of calibration/test cases, and writes the results plus
provenance metadata to `tests/fixtures/score_sdr_v0_1_1.json`.

This script is a one-time (or occasional, on a version bump) generation
tool. It is not part of the Rust crate's build or test run: the committed
JSON fixture is read by `tests/score_sdr_oracle.rs` directly, and Python is
never invoked by `cargo test`.

Usage:

    python3 scripts/oracles/generate_score_sdr.py --repo /path/to/Tian-Bai/SCoRE/checkout

`--repo` must point to a local checkout of
https://github.com/Tian-Bai/SCoRE whose git HEAD, `SCoRE/SCoRE.py` blob,
and `SCoRE.__version__` exactly match this repository's pinned provenance
(see `scripts/score_provenance.py`) -- this is verified and fails the
script immediately if it does not hold, with no override flag. Requires
`numpy` (the only runtime dependency `SCoRE.SCoRE` itself has).

## Why there is no "independent" (Equation 4.1) oracle column

`SCoRE`'s only per-test-point brute-force reference for that construction,
`SCoRE_MDR_bf`, evaluates its objective at exactly `l in {0, 1}` and does
not sweep the interior breakpoints Equation 4.1's infimum requires in
general (this crate's own `risk_adjusted_evalue` does sweep them, per its
module docs). `scripts/audits/compare_score_reference.py` measures how
often this diverges from the true infimum -- see its own output and
`docs/references.md` for the numbers. `certify_independent` /
`risk_adjusted_evalue` were already validated in a prior milestone via
hand-derivation and property tests and are not re-validated against this
known-incomplete function here; this fixture's oracle comparison is scoped
to the coupled construction, which is this PR's actual subject.
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

GENERATOR_SEED = 20260729
GENERATED_DATE = "2026-07-29"


def evaluate_case(SCoRE_SDR, Lcalib, Scalib, Stest, alpha, gamma):
    import numpy as np

    Lcalib = np.asarray(Lcalib, dtype=float)
    Scalib = np.asarray(Scalib, dtype=float)
    Stest = np.asarray(Stest, dtype=float)

    sel, coupled_evalues = SCoRE_SDR((Lcalib, Scalib), Stest, alpha, gamma, return_evals=True)

    # tau_hat is not returned by SCoRE_SDR directly; recover it from the
    # same eBH convention this crate implements (Theorem 3.3): the largest
    # tau such that the tau-th largest e-value clears m/(alpha*tau).
    m = len(Stest)
    tau_hat = None
    if m > 0:
        sorted_desc = sorted(coupled_evalues.tolist(), reverse=True)
        for tau in range(1, m + 1):
            threshold = m / (alpha * tau)
            if sorted_desc[tau - 1] >= threshold:
                tau_hat = tau

    return {
        "calibration_losses": Lcalib.tolist(),
        "calibration_scores": Scalib.tolist(),
        "test_scores": Stest.tolist(),
        "alpha": float(alpha),
        "gamma": float(gamma),
        "coupled_evalues": coupled_evalues.tolist(),
        "selected_indices": sorted(int(i) for i in sel.tolist()),
        "tau_hat": tau_hat,
    }


def build_fixed_cases(SCoRE_SDR):
    cases = []

    def add(name, description, **kwargs):
        case = evaluate_case(SCoRE_SDR, **kwargs)
        case["name"] = name
        case["description"] = description
        cases.append(case)

    # 1. Hand-computable minimal example (matches
    #    tests/paper_score_sdr.rs::score_equation_5_1_coupled_evalue_matches_hand_computation).
    add(
        "minimal_hand_computable",
        "n=1 zero-loss calibration, two distinct test points, traced by "
        "hand in tests/paper_score_sdr.rs.",
        Lcalib=[0.0],
        Scalib=[0.0],
        Stest=[-1.0, 1.0],
        alpha=0.5,
        gamma=0.5,
    )

    # 2. Calibration score ties.
    add(
        "calibration_score_ties",
        "Two calibration points tied at the same score; their losses must "
        "land in the same pooled-score group.",
        Lcalib=[0.5, 0.5, 0.1],
        Scalib=[2.0, 2.0, -1.0],
        Stest=[0.5, 2.0, 3.0],
        alpha=0.5,
        gamma=0.6,
    )

    # 3. Test score ties.
    add(
        "test_score_ties",
        "Three test points tied at the same score; each excludes only "
        "itself from the coupled denominator, not the whole tied group.",
        Lcalib=[0.2, 0.2],
        Scalib=[-1.0, 1.0],
        Stest=[0.0, 0.0, 0.0],
        alpha=0.5,
        gamma=0.5,
    )

    # 4. Calibration and test share an identical score.
    add(
        "calibration_and_test_share_a_score",
        "A calibration point and a test point land in the same pooled "
        "score group.",
        Lcalib=[0.3, 0.6],
        Scalib=[1.0, -2.0],
        Stest=[1.0, 5.0],
        alpha=0.6,
        gamma=0.5,
    )

    # 5. All losses zero.
    add(
        "all_losses_zero",
        "Calibration loss is 0 everywhere.",
        Lcalib=[0.0, 0.0, 0.0],
        Scalib=[-1.0, 0.0, 1.0],
        Stest=[-0.5, 0.5, 2.0],
        alpha=0.5,
        gamma=0.3,
    )

    # 6. All losses one.
    add(
        "all_losses_one",
        "Calibration loss is 1 (the loss upper bound) everywhere.",
        Lcalib=[1.0, 1.0, 1.0],
        Scalib=[-1.0, 0.0, 1.0],
        Stest=[-0.5, 0.5, 2.0],
        alpha=0.9,
        gamma=0.9,
    )

    # 7. Empty test batch.
    add(
        "empty_test_batch",
        "No test points at all; must not error and must select nothing.",
        Lcalib=[1.0, 0.0],
        Scalib=[0.0, 1.0],
        Stest=[],
        alpha=0.5,
        gamma=0.5,
    )

    # 8. Selection count 0.
    add(
        "zero_selections",
        "gamma too strict for any threshold to be feasible; every "
        "e-value is 0 and eBH selects nothing.",
        Lcalib=[1.0],
        Scalib=[0.0],
        Stest=[0.0, 0.0],
        alpha=0.5,
        gamma=1e-6,
    )

    # 9. All test points selected.
    add(
        "all_selected",
        "Zero calibration loss and a generous gamma/alpha make every "
        "test point's e-value clear the eBH threshold.",
        Lcalib=[0.0, 0.0],
        Scalib=[-1.0, 1.0],
        Stest=[-2.0, -1.5, 0.0, 1.5, 2.0],
        alpha=0.9,
        gamma=0.9,
    )

    # 10. Coupled vs independent disagreement fixture. This script only
    #     records the coupled (Python-oracle) side; the independent side is
    #     computed and compared purely in Rust, against this crate's own
    #     already-validated `risk_adjusted_evalue`
    #     (see `tests/paper_score_sdr.rs::score_algorithm_2_sdr_coupled_and_independent_can_disagree`
    #     and `docs/references.md` for why no Python "independent" oracle
    #     is used).
    add(
        "coupled_and_independent_disagree",
        "The coupled construction selects the low-scoring test point "
        "(its denominator only has to account for m=2 test points, one "
        "excluded as 'self'); the independent construction (checked "
        "separately in Rust, not via this script) does not.",
        Lcalib=[0.118, 0.9619, 0.9086, 0.6997, 0.2659],
        Scalib=[2.8151, 1.6725, 1.3013, -0.3038, -1.3666],
        Stest=[-2.4217, 2.4156],
        alpha=0.519,
        gamma=0.3417,
    )

    return cases


def build_random_cases(SCoRE_SDR, seed, count):
    import numpy as np

    rng = np.random.default_rng(seed)
    cases = []
    for i in range(count):
        n = int(rng.integers(1, 8))
        m = int(rng.integers(1, 8))
        # Half the draws use a small discrete score grid so ties remain
        # common among the random fixtures too, not just the fixed ones.
        if i % 2 == 0:
            grid = rng.choice(np.array([-2.0, -1.0, 0.0, 1.0, 2.0]), size=n + m)
            Scalib = grid[:n]
            Stest = grid[n:]
        else:
            Scalib = rng.uniform(-3, 3, size=n)
            Stest = rng.uniform(-3, 3, size=m)
        Lcalib = rng.uniform(0, 1, size=n)
        alpha = float(rng.uniform(0.05, 0.95))
        gamma = float(rng.uniform(0.05, 0.95))

        case = evaluate_case(SCoRE_SDR, Lcalib, Scalib, Stest, alpha, gamma)
        case["name"] = f"random_seed_{seed}_index_{i}"
        case["description"] = (
            f"Randomized fixture #{i} from numpy.random.default_rng({seed}); "
            "even indices use a small discrete score grid to keep ties common."
        )
        cases.append(case)
    return cases


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--repo",
        required=True,
        type=Path,
        help="Path to a local checkout of https://github.com/Tian-Bai/SCoRE",
    )
    parser.add_argument(
        "--out",
        type=Path,
        default=Path(__file__).resolve().parents[2] / "tests" / "fixtures" / "score_sdr_v0_1_1.json",
        help="Output JSON fixture path.",
    )
    parser.add_argument(
        "--random-count",
        type=int,
        default=20,
        help="Number of fixed-seed random fixtures to generate.",
    )
    args = parser.parse_args()

    measured_provenance = verify_score_checkout(args.repo)
    from SCoRE.SCoRE import SCoRE_SDR

    cases = build_fixed_cases(SCoRE_SDR)
    cases.extend(build_random_cases(SCoRE_SDR, GENERATOR_SEED, args.random_count))

    fixture = {
        "provenance": {
            "repository": REPOSITORY,
            "license": LICENSE,
            "license_copyright": LICENSE_COPYRIGHT,
            **measured_provenance,
            "generator_seed": GENERATOR_SEED,
            "generated_date": GENERATED_DATE,
            "generator_script": "scripts/oracles/generate_score_sdr.py",
            "notes": (
                "coupled_evalues come from SCoRE_SDR (Equation 5.1). There "
                "is no independent (Equation 4.1) oracle column: "
                "SCoRE_MDR_bf only checks l in {0,1} and was found to "
                "diverge from the true infimum in a nontrivial fraction of "
                "randomized trials, so it is not used as an oracle -- see "
                "scripts/audits/compare_score_reference.py and "
                "docs/references.md for the measured numbers."
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
