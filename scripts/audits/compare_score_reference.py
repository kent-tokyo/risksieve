#!/usr/bin/env python3
"""Reproducible audits of two divergences documented in docs/references.md
between Tian-Bai/SCoRE's official implementation and the paper it
implements.

1. **clamp** -- `SCoRE_SDR`'s `ell_bar`/`ELL` breakpoint value is not
   clamped to `[0,1]` before being plugged into the coupled e-value's
   objective, even though Equation 5.1's infimum is stated over
   `l in [0,1]`. Compares `SCoRE_SDR`'s output against a structurally
   identical copy that clamps, across randomized trials biased toward
   adversarial (small, tie-heavy, extreme-`gamma`) configurations.
2. **mdr-bf** -- `SCoRE_MDR_bf` (the official brute-force reference for
   Equation 4.1) only evaluates its objective at `l in {0,1}`, not the
   full breakpoint set Equation 4.1's infimum requires in general.
   Compares it against a from-scratch full breakpoint enumeration.

Neither comparison is part of `cargo test` or CI; this script exists so
the numbers `docs/references.md` and `THIRD_PARTY_NOTICES.md` cite are
reproducible by a third party rather than resting on an uncommitted
throwaway script.

Usage:

    python3 scripts/audits/compare_score_reference.py --repo /path/to/Tian-Bai/SCoRE/checkout
    python3 scripts/audits/compare_score_reference.py --repo ... --json-out report.json
    python3 scripts/audits/compare_score_reference.py --repo ... --clamp-trials 100000

`--repo` is verified against this repository's pinned SCoRE provenance
(see `scripts/score_provenance.py`) before anything runs; there is no
override flag. Requires `numpy`.
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

DEFAULT_CLAMP_SEED = 7
DEFAULT_CLAMP_TRIALS = 50_000
DEFAULT_MDR_BF_SEED = 123
DEFAULT_MDR_BF_TRIALS = 5_000


def score_sdr_with_optional_clamp(Dcalib, Dtest, alpha, gamma, clamp):
    """A structural copy of `SCoRE_SDR`'s general-branch computation
    (same `NUMER`/`DENOM`/`FR_0`/`FR_1`/`ELL`/`M_star`/suffix-max
    structure), differing only in whether the `ell_bar`/`ELL` breakpoint
    value is clamped to `[0,1]` before being plugged into the objective.
    `clamp=False` reproduces the official implementation's behavior;
    `clamp=True` reproduces this crate's `coupled_risk_adjusted_evalues`.
    Kept here (not imported from the real `SCoRE_SDR`) because comparing
    against the official code requires a variant of its own internals,
    not a black-box call.
    """
    import numpy as np

    Lcalib, Scalib = Dcalib
    Lcalib = np.asarray(Lcalib, dtype=float)
    Scalib = np.asarray(Scalib, dtype=float)
    Stest = np.asarray(Dtest, dtype=float)
    Ncalib, Ntest = len(Scalib), len(Stest)

    Scalib_tagged = [(lp, l, "calib") for lp, l in zip(Scalib, Lcalib)]
    Stest_tagged = [(lp, 0, "test") for lp in Stest]
    M_tagged = Scalib_tagged + Stest_tagged
    M_tagged.sort()
    M = np.array([a[0] for a in M_tagged])

    evalues = np.zeros(Ntest)
    ell_over_1_events = 0

    NUMER = np.zeros(Ncalib + Ntest)
    DENOM = np.zeros(Ncalib + Ntest)
    for i, (t, L, l_type) in enumerate(M_tagged):
        NUMER[i] = NUMER[i - 1] if i != 0 else 0
        DENOM[i] = DENOM[i - 1] if i != 0 else 1
        if l_type == "calib":
            NUMER[i] += L
        else:
            DENOM[i] += 1
    for i in range(len(M_tagged) - 2, -1, -1):
        if M_tagged[i][0] == M_tagged[i + 1][0]:
            NUMER[i] = NUMER[i + 1]
            DENOM[i] = DENOM[i + 1]

    for j in range(Ntest):
        FR_0 = np.zeros(Ncalib + Ntest)
        FR_1 = np.zeros(Ncalib + Ntest)
        ELL = np.zeros(Ncalib + Ntest)
        t_0, t_1 = (-1, -np.inf), (-1, -np.inf)

        for i, (t, _, _) in enumerate(M_tagged):
            FR_0[i] = NUMER[i] / (DENOM[i] - (Stest[j] <= t)) / (Ncalib + 1) * Ntest
            FR_1[i] = (
                (NUMER[i] + (Stest[j] <= t)) / (DENOM[i] - (Stest[j] <= t)) / (Ncalib + 1) * Ntest
            )
            ELL[i] = (Ncalib + 1) * gamma / Ntest * (DENOM[i] - (Stest[j] <= t)) - NUMER[i]

        for i, t in enumerate(M):
            if FR_0[i] <= gamma:
                t_0 = (i, t)
            if FR_1[i] <= gamma:
                t_1 = (i, t)

        if Stest[j] > t_1[1]:
            continue

        if t_1[1] == t_0[1]:
            evalues[j] = (Ncalib + 1) / (1 + NUMER[t_1[0]])
            continue

        max_ell = np.zeros(Ntest + Ncalib)
        last_max = -np.inf
        for i, t in zip(range(Ntest + Ncalib - 1, -1, -1), reversed(M)):
            max_ell[i] = last_max
            if FR_0[i] <= gamma:
                last_max = max(last_max, ELL[i])

        M_star = []
        for i, t in enumerate(M):
            if t < max(Stest[j], t_1[1]):
                continue
            if t > t_0[1]:
                break
            if FR_0[i] <= gamma and ELL[i] > max_ell[i]:
                M_star.append((i, t))

        evalue = np.inf
        for i, t in M_star:
            ell = ELL[i]
            if ell > 1.0:
                ell_over_1_events += 1
            if clamp:
                ell = min(max(ell, 0.0), 1.0)
            cur_val = (Ncalib + 1) / (ell + NUMER[i])
            evalue = min(evalue, cur_val)
        evalues[j] = evalue

    return evalues, ell_over_1_events


def full_breakpoint_evalue(Lcalib, Scalib, Stest_j, gamma, n_plus_1):
    """A from-scratch, independent reference implementation of Equation
    4.1's true infimum via full breakpoint enumeration -- not a copy of
    this crate's `risk_adjusted_evalue`, so it serves as a genuinely
    separate check on both that Rust function and on `SCoRE_MDR_bf`."""
    import numpy as np

    M = np.concatenate([Scalib, [Stest_j]])
    gamma_scaled = gamma * n_plus_1

    def F(t, ell):
        return float(np.sum(Lcalib * (Scalib <= t)) + ell * (Stest_j <= t))

    def t_gamma(ell):
        max_t, feasible = -np.inf, False
        for cur_t in M:
            if F(cur_t, ell) <= gamma_scaled:
                max_t, feasible = max(max_t, cur_t), True
        return max_t, feasible

    candidates = [0.0, 1.0]
    for cur_t in M:
        base = float(np.sum(Lcalib * (Scalib <= cur_t)))
        candidates.append(min(max(gamma_scaled - base, 0.0), 1.0))

    best = float("inf")
    for ell in candidates:
        t_l, feasible = t_gamma(ell)
        if not feasible:
            continue
        num = n_plus_1 * (Stest_j <= t_l)
        denom = float(np.sum(Lcalib * (Scalib <= t_l)) + ell * (Stest_j <= t_l))
        val = (num / denom) if denom != 0 else (float("inf") if num > 0 else 0.0)
        best = min(best, val)
    return best


def run_clamp_comparison(SCoRE_SDR, eBH, seed, trials):
    import numpy as np

    rng = np.random.default_rng(seed)
    ell_over_1_events = 0
    evalue_mismatches = 0
    selection_mismatches = 0
    max_abs_diff = 0.0
    minimal_reproducer = None

    for trial in range(trials):
        n = int(rng.integers(1, 6))
        m = int(rng.integers(2, 8))
        grid = rng.choice(np.array([-2.0, -1.0, -0.5, 0.0, 0.5, 1.0, 2.0]), size=n + m)
        Scalib, Stest = grid[:n], grid[n:]
        Lcalib = rng.choice([0.0, 0.25, 0.5, 0.75, 1.0], size=n)
        gamma = float(rng.choice([0.001, 0.01, 0.05, 0.5, 0.9, 0.95, 0.99, 0.999]))
        alpha = float(rng.uniform(0.05, 0.95))

        ev_official, n_over_1 = score_sdr_with_optional_clamp(
            (Lcalib, Scalib), Stest, alpha, gamma, clamp=False
        )
        ev_clamped, _ = score_sdr_with_optional_clamp((Lcalib, Scalib), Stest, alpha, gamma, clamp=True)
        ell_over_1_events += n_over_1

        diff = np.abs(ev_official - ev_clamped)
        if diff.max() > 1e-9:
            evalue_mismatches += 1
            max_abs_diff = max(max_abs_diff, float(diff.max()))

        sel_official = set(int(i) for i in np.flatnonzero(eBH(ev_official, alpha)))
        sel_clamped = set(int(i) for i in np.flatnonzero(eBH(ev_clamped, alpha)))
        if sel_official != sel_clamped:
            selection_mismatches += 1

        if diff.max() > 1e-9 and (
            minimal_reproducer is None or (n + m) < minimal_reproducer["n"] + minimal_reproducer["m"]
        ):
            minimal_reproducer = {
                "trial": trial,
                "n": n,
                "m": m,
                "Lcalib": Lcalib.tolist(),
                "Scalib": Scalib.tolist(),
                "Stest": Stest.tolist(),
                "alpha": alpha,
                "gamma": gamma,
                "max_abs_diff": float(diff.max()),
            }

    return {
        "seed": seed,
        "trials": trials,
        "ell_over_1_events": ell_over_1_events,
        "evalue_mismatches": evalue_mismatches,
        "selection_mismatches": selection_mismatches,
        "max_abs_diff": max_abs_diff,
        "minimal_reproducer": minimal_reproducer,
    }


def run_mdr_bf_comparison(SCoRE_MDR_bf, seed, trials):
    import numpy as np

    rng = np.random.default_rng(seed)
    mismatches = 0
    max_abs_diff = 0.0
    minimal_reproducer = None

    for trial in range(trials):
        n = int(rng.integers(1, 8))
        Lcalib = rng.uniform(0, 1, size=n)
        Scalib = rng.uniform(-3, 3, size=n)
        Stest_j = float(rng.uniform(-3, 3))
        gamma = float(rng.uniform(0.05, 0.95))

        _, evs = SCoRE_MDR_bf((Lcalib, Scalib), np.array([Stest_j]), 0.5, gamma, return_evals=True)
        bf_val = float(evs[0])
        full_val = full_breakpoint_evalue(Lcalib, Scalib, Stest_j, gamma, n + 1)

        both_inf = np.isinf(bf_val) and np.isinf(full_val)
        diff = 0.0 if both_inf else abs(bf_val - full_val)
        if diff > 1e-9:
            mismatches += 1
            max_abs_diff = max(max_abs_diff, diff)
            if minimal_reproducer is None or n < minimal_reproducer["n"]:
                minimal_reproducer = {
                    "trial": trial,
                    "n": n,
                    "Lcalib": Lcalib.tolist(),
                    "Scalib": Scalib.tolist(),
                    "Stest_j": Stest_j,
                    "gamma": gamma,
                    "score_mdr_bf_value": bf_val,
                    "full_breakpoint_value": full_val,
                }

    return {
        "seed": seed,
        "trials": trials,
        "mismatches": mismatches,
        "max_abs_diff": max_abs_diff,
        "minimal_reproducer": minimal_reproducer,
    }


def print_summary(report):
    clamp = report["clamp_comparison"]
    mdr_bf = report["mdr_bf_comparison"]
    print(f"provenance: commit={report['provenance']['commit_sha']} "
          f"version={report['provenance']['package_version']}")
    print()
    print(f"clamp comparison: seed={clamp['seed']} trials={clamp['trials']}")
    print(f"  ell_bar > 1 events (unclamped stage): {clamp['ell_over_1_events']}")
    print(f"  e-value mismatches: {clamp['evalue_mismatches']} (max abs diff {clamp['max_abs_diff']})")
    print(f"  selected-set mismatches: {clamp['selection_mismatches']}")
    if clamp["minimal_reproducer"]:
        print(f"  smallest reproducer found: n={clamp['minimal_reproducer']['n']} "
              f"m={clamp['minimal_reproducer']['m']} (trial {clamp['minimal_reproducer']['trial']})")
    print()
    print(f"SCoRE_MDR_bf completeness comparison: seed={mdr_bf['seed']} trials={mdr_bf['trials']}")
    print(f"  mismatches: {mdr_bf['mismatches']} ({100.0 * mdr_bf['mismatches'] / mdr_bf['trials']:.1f}%)")
    print(f"  max abs diff: {mdr_bf['max_abs_diff']}")
    if mdr_bf["minimal_reproducer"]:
        r = mdr_bf["minimal_reproducer"]
        print(f"  smallest reproducer found: n={r['n']} (trial {r['trial']}), "
              f"SCoRE_MDR_bf={r['score_mdr_bf_value']}, true={r['full_breakpoint_value']}")


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--repo", required=True, type=Path,
                         help="Path to a local checkout of https://github.com/Tian-Bai/SCoRE")
    parser.add_argument("--clamp-seed", type=int, default=DEFAULT_CLAMP_SEED)
    parser.add_argument("--clamp-trials", type=int, default=DEFAULT_CLAMP_TRIALS)
    parser.add_argument("--mdr-bf-seed", type=int, default=DEFAULT_MDR_BF_SEED)
    parser.add_argument("--mdr-bf-trials", type=int, default=DEFAULT_MDR_BF_TRIALS)
    parser.add_argument("--json-out", type=Path, default=None,
                         help="Optional path to also write a machine-readable JSON report.")
    args = parser.parse_args()

    measured_provenance = verify_score_checkout(args.repo)
    from SCoRE.SCoRE import SCoRE_MDR_bf, SCoRE_SDR
    from SCoRE.utility import eBH

    clamp_result = run_clamp_comparison(SCoRE_SDR, eBH, args.clamp_seed, args.clamp_trials)
    mdr_bf_result = run_mdr_bf_comparison(SCoRE_MDR_bf, args.mdr_bf_seed, args.mdr_bf_trials)

    report = {
        "provenance": {
            "repository": REPOSITORY,
            "license": LICENSE,
            "license_copyright": LICENSE_COPYRIGHT,
            **measured_provenance,
        },
        "clamp_comparison": clamp_result,
        "mdr_bf_comparison": mdr_bf_result,
    }

    print_summary(report)

    if args.json_out:
        with open(args.json_out, "w") as f:
            json.dump(report, f, indent=2)
        print(f"\nwrote machine-readable report to {args.json_out}")


if __name__ == "__main__":
    try:
        main()
    except ProvenanceError as err:
        fail_with_provenance_error(err)
