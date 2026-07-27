//! Paper-traceable tests for SCoRE-SDR.
//!
//! Source: Bai and Jin (2026), *Conformal Selective Prediction with
//! General Risk Control*, arXiv:2603.24704, Theorem 3.3 (eBH selection
//! and its SDR guarantee) and Algorithm 2 (SCoRE-SDR). See
//! `src/selective/ebh.rs` and `src/selective/sdr.rs` for the module docs
//! this file assumes as background, including why `sdr::certify` reuses
//! Milestone 4's per-test-point e-value construction independently for
//! each batch item rather than the paper's own Equation 5.1 (deferred;
//! see `tasks/todo.md`).

use risksieve::selective::ebh::select;
use risksieve::selective::sdr::{certify, realized_selective_risk};
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
/// three test points are selected.
#[test]
fn score_algorithm_2_sdr_matches_reference() {
    let alpha = OpenUnitInterval::new("alpha", 0.5).unwrap();
    let certificate = certify(&[loss(0.0)], &[1.0], &[0.0, 0.0, 0.0], alpha, alpha).unwrap();

    assert_eq!(certificate.parameter, vec![0, 1, 2]);
    assert_eq!(certificate.diagnostics.ebh_tau_hat, Some(3));
    assert_eq!(
        certificate.guarantee,
        GuaranteeKind::SelectiveDeploymentRisk
    );
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
