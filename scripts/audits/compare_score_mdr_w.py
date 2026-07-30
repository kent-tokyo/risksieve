#!/usr/bin/env python3
"""Reproducible audit backing the claim in `docs/references.md` ("Equation
6.1 audit") and this crate's `tests/score_mdr_w_oracle.rs` module docs:
that `official_selected_indices` can be compared exactly against
Tian-Bai/SCoRE's `SCoRE_MDR_w` for *every* fixture case, `gamma <= alpha`
or not, with no need to gate the comparison on `gamma <= alpha`.

`SCoRE_MDR_w` (`SCoRE/SCoRE.py`, commit
`401b7caf6d030825ff67e8f08e44ba15ee8c94af`, package version `0.1.1`) is a
closed-form shortcut: for `gamma <= alpha` it applies unconditionally; for
`gamma > alpha` it first computes the same naive check, then applies an
additional threshold-overlap condition that can flip a naive "deploy" to
"abstain". This crate's `weighted_risk_adjusted_evalue` never takes any
shortcut -- it always computes Equation 6.1's actual infimum by breakpoint
enumeration -- so there is no structural reason its decision would need
the shortcut's extra `gamma > alpha` condition to already agree with it.
That is an empirical claim, not a proof, so this script exists to make it
reproducible by a third party rather than resting on an uncommitted
throwaway search.

This script compares, per randomized trial:

- **the official decision**: `SCoRE_MDR_w`'s own, actual return value
  (never a reimplementation -- always the real, imported function).
- **the reference decision**: whether `generate_score_mdr_w.py`'s
  `weighted_evalue_reference` (an independent, from-scratch breakpoint
  enumeration of Equation 6.1) clears the `1/alpha` deployment threshold.

and separately tallies, for descriptive purposes only (not part of the
pass/fail verdict): how many trials have `gamma > alpha`, and of those,
how many have the official shortcut's naive (pre-overlap-check) decision
flipped by its overlap condition. The naive decision is not exposed by
the public `SCoRE_MDR_w` function, so `_naive_pre_overlap_decision` below
is a minimal structural copy of just that one internal expression
(`SCoRE/SCoRE.py` lines 233-235 at the pinned commit) -- not a
reimplementation of the whole procedure, and not what the pass/fail
verdict is checked against.

Usage (the default seed and trial count reproduce the number cited in
`docs/references.md` and the PR discussion -- 300,000 trials, zero
mismatches, tens of thousands of `gamma > alpha` overlap-condition
flips):

    python3 scripts/audits/compare_score_mdr_w.py --repo /path/to/Tian-Bai/SCoRE
    python3 scripts/audits/compare_score_mdr_w.py --repo ... --trials 300000 --seed 20260730

`--repo` is verified against this repository's pinned SCoRE provenance
(see `scripts/score_provenance.py`) before anything runs; there is no
override flag. Requires `numpy`. Exits non-zero if any mismatch is found.
Not part of `cargo test` or CI -- 300,000 trials takes some tens of
seconds, deliberately heavier than what should run on every push.
"""

import argparse
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "oracles"))
from score_provenance import (  # noqa: E402
    LICENSE,
    LICENSE_COPYRIGHT,
    REPOSITORY,
    ProvenanceError,
    fail_with_provenance_error,
    verify_score_checkout,
)
from generate_score_mdr_w import weighted_evalue_reference  # noqa: E402

DEFAULT_SEED = 20260730
DEFAULT_TRIALS = 300_000


def _naive_pre_overlap_decision(Lcalib, Scalib, Wcalib, Stest_j, Wtest_j, gamma):
    """`phi` from `SCoRE_MDR_w` (`SCoRE/SCoRE.py`, commit
    `401b7caf6d030825ff67e8f08e44ba15ee8c94af`, lines 233-235) *before*
    its `gamma > alpha` overlap check is applied. This one expression is
    reproduced here only because the public function does not return it
    separately; the actual official decision used for the pass/fail
    verdict always comes from the real, imported `SCoRE_MDR_w` below.

    Deliberately plain Python (no numpy), unlike the rest of this script,
    so `test_compare_score_mdr_w.py` can unit-test it without numpy
    installed -- matching `python-provenance-tests` CI job's own
    self-contained, dependency-free scope (see the workflow file).
    Accepts any iterable for `Lcalib`/`Scalib`/`Wcalib` (plain lists in
    tests, numpy arrays from `run_audit` below).
    """
    calib_w_sum = sum(Wcalib)
    weighted_loss_below = sum(
        w * l for w, l, s in zip(Wcalib, Lcalib, Scalib) if s <= Stest_j
    )
    numer = Wtest_j + weighted_loss_below
    denom = Wtest_j + calib_w_sum
    return numer / denom <= gamma


def run_audit(SCoRE_MDR_w, seed, trials):
    import numpy as np

    rng = np.random.default_rng(seed)
    mismatches = 0
    gamma_gt_alpha_count = 0
    overlap_condition_evaluated_count = 0
    overlap_condition_flip_count = 0
    first_mismatch = None

    for trial in range(trials):
        n = int(rng.integers(1, 8))
        if trial % 2 == 0:
            grid = rng.choice(np.array([-2.0, -1.0, 0.0, 1.0, 2.0]), size=n + 1)
            Scalib, Stest = grid[:n], grid[n:]
        else:
            Scalib = rng.uniform(-3, 3, size=n)
            Stest = rng.uniform(-3, 3, size=1)
        Lcalib = rng.uniform(0, 1, size=n)
        # Never exactly zero: a fully-degenerate combined weight is a
        # separate, already-covered concern (`RiskSieveError::DegenerateWeights`
        # / `SCoRE_MDR_w`'s own 0/0 division), not what this audit targets.
        Wcalib = rng.uniform(0.01, 10.0, size=n)
        Wtest = rng.uniform(0.01, 10.0, size=1)
        alpha = float(rng.uniform(0.05, 0.95))
        # Half the trials keep gamma <= alpha (the shortcut's unconditional
        # regime); the other half range up to 2.5x alpha, exercising the
        # shortcut's extra `gamma > alpha` overlap condition.
        if trial % 4 < 2:
            gamma = float(rng.uniform(0.01, alpha))
        else:
            gamma = float(rng.uniform(0.01, min(alpha * 2.5, 0.999)))

        official_selected = SCoRE_MDR_w((Lcalib, Scalib), Stest, Wcalib, Wtest, alpha, gamma)
        official_decision = 0 in set(int(i) for i in official_selected.tolist())

        evalue = weighted_evalue_reference(
            Lcalib, Scalib, Wcalib, float(Stest[0]), float(Wtest[0]), gamma
        )
        reference_decision = evalue is not None and (
            evalue == float("inf") or evalue >= 1.0 / alpha
        )

        if gamma > alpha:
            gamma_gt_alpha_count += 1
            naive = _naive_pre_overlap_decision(
                Lcalib, Scalib, Wcalib, float(Stest[0]), float(Wtest[0]), gamma
            )
            if naive:
                overlap_condition_evaluated_count += 1
                if not official_decision:
                    overlap_condition_flip_count += 1

        if official_decision != reference_decision:
            mismatches += 1
            if first_mismatch is None:
                first_mismatch = {
                    "trial": trial,
                    "Lcalib": Lcalib.tolist(),
                    "Scalib": Scalib.tolist(),
                    "Wcalib": Wcalib.tolist(),
                    "Stest": Stest.tolist(),
                    "Wtest": Wtest.tolist(),
                    "alpha": alpha,
                    "gamma": gamma,
                    "official_decision": official_decision,
                    "reference_decision": reference_decision,
                    "reference_evalue": evalue,
                }

    return {
        "seed": seed,
        "trials": trials,
        "mismatches": mismatches,
        "gamma_gt_alpha_count": gamma_gt_alpha_count,
        "overlap_condition_evaluated_count": overlap_condition_evaluated_count,
        "overlap_condition_flip_count": overlap_condition_flip_count,
        "first_mismatch": first_mismatch,
    }


def print_report(provenance, result):
    print(
        f"provenance: commit={provenance['commit_sha']} "
        f"version={provenance['package_version']}"
    )
    print(f"seed={result['seed']} trials={result['trials']}")
    print(f"  gamma > alpha trials: {result['gamma_gt_alpha_count']}")
    print(
        "  of those, official shortcut's overlap condition evaluated "
        f"(naive decision was 'deploy'): {result['overlap_condition_evaluated_count']}"
    )
    print(
        "  of those, overlap condition flipped the decision to 'abstain': "
        f"{result['overlap_condition_flip_count']}"
    )
    print(f"  mismatches (official vs. reference decision): {result['mismatches']}")
    if result["first_mismatch"]:
        m = result["first_mismatch"]
        print("  first mismatch:")
        print(f"    trial={m['trial']}")
        print(f"    Lcalib={m['Lcalib']} Scalib={m['Scalib']} Wcalib={m['Wcalib']}")
        print(f"    Stest={m['Stest']} Wtest={m['Wtest']}")
        print(f"    alpha={m['alpha']} gamma={m['gamma']}")
        print(f"    official_decision={m['official_decision']}")
        print(f"    reference_decision={m['reference_decision']}")
        print(f"    reference_evalue={m['reference_evalue']}")


def main():
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    parser.add_argument(
        "--repo",
        required=True,
        type=Path,
        help="Path to a local checkout of https://github.com/Tian-Bai/SCoRE",
    )
    parser.add_argument("--seed", type=int, default=DEFAULT_SEED)
    parser.add_argument("--trials", type=int, default=DEFAULT_TRIALS)
    args = parser.parse_args()

    measured_provenance = verify_score_checkout(args.repo)
    from SCoRE.SCoRE import SCoRE_MDR_w

    result = run_audit(SCoRE_MDR_w, args.seed, args.trials)
    provenance = {
        "repository": REPOSITORY,
        "license": LICENSE,
        "license_copyright": LICENSE_COPYRIGHT,
        **measured_provenance,
    }
    print_report(provenance, result)

    sys.exit(1 if result["mismatches"] > 0 else 0)


if __name__ == "__main__":
    try:
        main()
    except ProvenanceError as err:
        fail_with_provenance_error(err)
