//! Paper-traceable tests for SCoRE-SDR.
//!
//! Source: Bai and Jin (2026), *Conformal Selective Prediction with
//! General Risk Control*, arXiv:2603.24704, Theorem 3.3 (eBH selection
//! and its SDR guarantee), Algorithm 2 (SCoRE-SDR), and Equation 5.1 /
//! Theorem 5.1 (the paper's own cross-test-point-coupled e-value). See
//! `src/selective/ebh.rs`, `src/selective/sdr.rs`, and
//! `src/selective/coupled.rs` for the module docs this file assumes as
//! background, including why `sdr::certify` now uses Equation 5.1 by
//! default while `sdr::certify_independent` keeps the earlier
//! per-test-point-independent composition available under its own name.

use risksieve::selective::coupled::coupled_risk_adjusted_evalues;
use risksieve::selective::ebh::select;
use risksieve::selective::sdr::{certify, certify_independent, realized_selective_risk};
use risksieve::{ClosedUnitInterval, GuaranteeKind, OpenUnitInterval};

fn loss(value: f64) -> ClosedUnitInterval {
    ClosedUnitInterval::new("loss", value).unwrap()
}

/// Hand trace: `m=3`, `alpha=0.5`, e-values `[10.0, 0.1, 8.0]` (indices
/// `0, 1, 2`). Sorted descending: `[10.0, 8.0, 0.1]`.
///
/// - `tau=1`: threshold `= 3/(0.5*1) = 6.0`; `sorted[0]=10.0 >= 6.0`: holds.
/// - `tau=2`: threshold `= 3/(0.5*2) = 3.0`; `sorted[1]=8.0 >= 3.0`: holds.
/// - `tau=3`: threshold `= 3/(0.5*3) = 2.0`; `sorted[2]=0.1 >= 2.0`: fails.
///
/// The largest satisfying `tau` is `2` (Theorem 3.3 takes the *max*, so
/// `tau=1` also holding does not matter), giving threshold `3.0`;
/// comparing every e-value directly against `3.0` (not "top 2 of the
/// sorted list", which would coincidentally agree here but differs in
/// general at ties) selects indices `0` (`10.0`) and `2` (`8.0`).
#[test]
fn score_theorem_3_3_ebh_selection_matches_reference() {
    let alpha = OpenUnitInterval::new("alpha", 0.5).unwrap();
    let evalues = [
        risksieve::NonNegative::new("e", 10.0).unwrap(),
        risksieve::NonNegative::new("e", 0.1).unwrap(),
        risksieve::NonNegative::new("e", 8.0).unwrap(),
    ];
    let selection = select(&evalues, alpha);
    assert_eq!(selection.tau_hat, Some(2));
    assert_eq!(selection.selected_indices, vec![0, 2]);
}

/// Hand trace: calibration `(s_1, L_1) = (1.0, 0.0)`, `gamma = 0.5`, so
/// (per `src/selective/mdr.rs`'s own hand-traced fixture with the same
/// numbers) each test point at score `0.0` independently gets e-value
/// `2.0`. With three identical test points, `m=3`: sorted descending
/// `[2.0, 2.0, 2.0]`.
///
/// - `tau=1`: threshold `3/(0.5*1)=6.0`; `2.0 >= 6.0`: fails.
/// - `tau=2`: threshold `3/(0.5*2)=3.0`; `2.0 >= 3.0`: fails.
/// - `tau=3`: threshold `3/(0.5*3)=2.0`; `2.0 >= 2.0`: holds (equality).
///
/// `tau_hat=3`, threshold `2.0`, and every e-value clears it, so all
/// three test points are selected. This is `certify_independent`'s
/// construction (Equation 4.1 applied to each test point on its own).
#[test]
fn score_algorithm_2_sdr_matches_reference_independent() {
    let alpha = OpenUnitInterval::new("alpha", 0.5).unwrap();
    let certificate =
        certify_independent(&[loss(0.0)], &[1.0], &[0.0, 0.0, 0.0], alpha, alpha).unwrap();

    assert_eq!(certificate.parameter, vec![0, 1, 2]);
    assert_eq!(certificate.diagnostics.ebh_tau_hat, Some(3));
    assert_eq!(
        certificate.guarantee,
        GuaranteeKind::SelectiveDeploymentRisk
    );
}

/// The same fixture through `certify`'s default coupled construction
/// (Equation 5.1). With three *identical* test points, each one's "other
/// test points below any threshold" count is the same by symmetry, so the
/// coupled e-value happens to agree with the independent one (`2.0`) here
/// too -- a property of this specific tied fixture, not a general
/// equivalence between the two constructions (see
/// `score_algorithm_2_sdr_coupled_and_independent_can_disagree` below for
/// a fixture where they do not).
#[test]
fn score_algorithm_2_sdr_matches_reference_coupled() {
    let alpha = OpenUnitInterval::new("alpha", 0.5).unwrap();
    let gamma = OpenUnitInterval::new("gamma", 0.5).unwrap();
    let evalues =
        coupled_risk_adjusted_evalues(&[loss(0.0)], &[1.0], &[0.0, 0.0, 0.0], gamma).unwrap();
    assert!(evalues.iter().all(|e| e.get() == 2.0));

    let certificate = certify(&[loss(0.0)], &[1.0], &[0.0, 0.0, 0.0], alpha, gamma).unwrap();
    assert_eq!(certificate.parameter, vec![0, 1, 2]);
    assert_eq!(certificate.diagnostics.ebh_tau_hat, Some(3));
    assert_eq!(
        certificate.guarantee,
        GuaranteeKind::SelectiveDeploymentRisk
    );
}

/// Hand trace of Equation 5.1 itself, distinct from the tied fixture
/// above: `n=1`, calibration `(s_1, L_1) = (0.0, 0.0)` (zero calibration
/// loss, so `calib_prefix` is `0` everywhere), two *distinct* test points
/// `Stest = [-1.0, 1.0]`, `gamma = 0.5`, so `n+1=2`, `m=2`,
/// `gamma_scale = gamma*(n+1)/m = 0.5`.
///
/// Pooled scores sorted: `M = [-1.0 (test A), 0.0 (calib), 1.0 (test B)]`.
/// For test point `A` (its own score `-1.0`, the smallest):
/// `denom_excl_j` (one plus the *other* test point's count at or below
/// each pooled score) is `1` at the first two pooled scores (`B`'s score
/// `1.0` is not yet included) and `2` at the last. Since calibration loss
/// is `0` everywhere, `FR(t;0) <= gamma` holds at every threshold, so
/// `t_gamma(0)` is the largest pooled score, `1.0`. At `l=1`,
/// `FR(t;1) = 1{A<=t} / denom_excl_j(t) * gamma_scale`'s inverse
/// condition works out to hold only at the last pooled score too (`1 <=
/// 0.5*2 = 1.0`, equality) -- so `t_gamma(1)` is *also* `1.0`, the same
/// threshold as `l=0`. Equal thresholds collapse to Equation 4.1's own
/// `l=1` formula (see `src/selective/coupled.rs`'s module docs): e-value
/// `= (n+1) / (1 + calib_prefix) = 2 / (1 + 0) = 2.0`. The identical
/// argument holds for test point `B` by symmetry (loss is `0`
/// everywhere, so which test point is "self" does not change the
/// arithmetic). Independently confirmed against `Tian-Bai/SCoRE`'s
/// `SCoRE_SDR` (commit `401b7caf6d030825ff67e8f08e44ba15ee8c94af`), which
/// returns the identical `[2.0, 2.0]`.
#[test]
fn score_equation_5_1_coupled_evalue_matches_hand_computation() {
    let gamma = OpenUnitInterval::new("gamma", 0.5).unwrap();
    let evalues = coupled_risk_adjusted_evalues(&[loss(0.0)], &[0.0], &[-1.0, 1.0], gamma).unwrap();
    assert_eq!(evalues[0].get(), 2.0);
    assert_eq!(evalues[1].get(), 2.0);
}

/// The coupled and independent constructions do not always agree.
/// Found by random search against the official `Tian-Bai/SCoRE` oracle
/// (see `scripts/oracles/generate_score_sdr.py`) and re-verified against
/// this crate: with `n=5` calibration points and two well-separated test
/// scores, the low-scoring test point's coupled e-value (`5.8530875`,
/// because the calibration-region threshold's denominator only has to
/// account for `m=2` test points total, one of which is excluded as
/// "self") clears the `alpha=0.519`, `m=2` eBH threshold at `tau=1`
/// (`2/(0.519*1) = 3.8536...`), while its independent (Equation 4.1)
/// e-value (`3.0525...`, computed against the *same* calibration set but
/// without any batch-size adjustment) does not.
#[test]
fn score_algorithm_2_sdr_coupled_and_independent_can_disagree() {
    let calib_losses = [
        loss(0.118),
        loss(0.9619),
        loss(0.9086),
        loss(0.6997),
        loss(0.2659),
    ];
    let calib_scores = [2.8151, 1.6725, 1.3013, -0.3038, -1.3666];
    let test_scores = [-2.4217, 2.4156];
    let alpha = OpenUnitInterval::new("alpha", 0.519).unwrap();
    let gamma = OpenUnitInterval::new("gamma", 0.3417).unwrap();

    let coupled = certify(&calib_losses, &calib_scores, &test_scores, alpha, gamma).unwrap();
    let independent =
        certify_independent(&calib_losses, &calib_scores, &test_scores, alpha, gamma).unwrap();

    assert_eq!(coupled.parameter, vec![0]);
    assert_eq!(independent.parameter, Vec::<usize>::new());
}

/// A batch too strict for `gamma` to support any threshold must still
/// return a valid (empty) certificate, not an error -- SDR's `1 v |R|`
/// denominator makes `SDR = 0 <= alpha` hold trivially when nothing is
/// selected (Milestone 5's "zero-selection behavior" requirement).
#[test]
fn score_zero_selection_is_a_valid_certificate_not_an_error() {
    let alpha = OpenUnitInterval::new("alpha", 0.5).unwrap();
    let strict_gamma = OpenUnitInterval::new("gamma", 0.1).unwrap();
    let certificate = certify(&[loss(1.0)], &[0.0], &[0.0, 0.0], alpha, strict_gamma).unwrap();

    assert_eq!(certificate.parameter, Vec::<usize>::new());
    assert_eq!(certificate.diagnostics.selected_count, Some(0));
    assert_eq!(certificate.diagnostics.uninformative_result, Some(true));
}

/// AGENTS.md: "Never replace the denominator `max(1, selected_count)`
/// with a different convention." A zero-selection batch's realized risk
/// must be exactly `0.0`, never `NaN` from a `0/0` division.
#[test]
fn score_realized_selective_risk_denominator_never_divides_by_zero() {
    assert_eq!(realized_selective_risk(&[]), 0.0);
}
