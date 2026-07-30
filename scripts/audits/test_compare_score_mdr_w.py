"""Unit tests for `compare_score_mdr_w.py`'s pure-Python helper function
`_naive_pre_overlap_decision`. Deliberately does not require numpy or a
Tian-Bai/SCoRE checkout (unlike the audit itself, which needs both and is
not run as part of this suite) -- matching the
`python-provenance-tests` CI job's self-contained, dependency-free
scope (see the workflow file)."""

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from compare_score_mdr_w import _naive_pre_overlap_decision  # noqa: E402


class NaivePreOverlapDecisionTests(unittest.TestCase):
    def test_generous_gamma_with_zero_calibration_loss_deploys(self):
        # numer = wtest + sum(wcalib*Lcalib*indicator) = 1.0 + 0 = 1.0
        # denom = wtest + calib_w_sum = 2.0; ratio 0.5 <= gamma 0.5.
        decision = _naive_pre_overlap_decision(
            Lcalib=[0.0],
            Scalib=[0.0],
            Wcalib=[1.0],
            Stest_j=0.0,
            Wtest_j=1.0,
            gamma=0.5,
        )
        self.assertTrue(decision)

    def test_strict_gamma_with_full_calibration_loss_abstains(self):
        # numer = 1.0 + 1.0*1.0*1 = 2.0; denom = 2.0; ratio 1.0 > gamma 0.1.
        decision = _naive_pre_overlap_decision(
            Lcalib=[1.0],
            Scalib=[0.0],
            Wcalib=[1.0],
            Stest_j=0.0,
            Wtest_j=1.0,
            gamma=0.1,
        )
        self.assertFalse(decision)

    def test_calibration_score_above_test_score_is_excluded_by_indicator(self):
        # Scalib (5.0) > Stest_j (0.0), so the indicator is 0 regardless
        # of Lcalib: numer = 1.0 + 0 = 1.0, denom = 2.0, ratio 0.5.
        decision = _naive_pre_overlap_decision(
            Lcalib=[1.0],
            Scalib=[5.0],
            Wcalib=[1.0],
            Stest_j=0.0,
            Wtest_j=1.0,
            gamma=0.4,
        )
        self.assertFalse(decision)  # ratio 0.5 > gamma 0.4
        decision_generous = _naive_pre_overlap_decision(
            Lcalib=[1.0],
            Scalib=[5.0],
            Wcalib=[1.0],
            Stest_j=0.0,
            Wtest_j=1.0,
            gamma=0.5,
        )
        self.assertTrue(decision_generous)  # ratio 0.5 <= gamma 0.5


if __name__ == "__main__":
    unittest.main()
