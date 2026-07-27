//! Paper-traceable tests for anytime-valid conformal risk control.
//!
//! Source: Hultberg, Zachariah, and Ribeiro (2026), *Anytime-Valid
//! Conformal Risk Control*, arXiv:2602.04364, Theorem 4.1 and Definition
//! 2.7.
//!
//! Interpretation required for this implementation: as in
//! `tests/paper_crc.rs`, the paper's `lambda_n = inf{...}` is realized as
//! a search over a caller-supplied ascending candidate grid. See
//! `src/anytime/boundary.rs` for the boundary-function fixtures and
//! `src/anytime/calibration.rs` for the running-minimum fixture; this
//! file exercises the same controller through its public API with two
//! additional checks: `m*` at the paper's own Section 6.1 simulation
//! parameters, and non-increase of the deployed parameter across many
//! updates on non-trivial (not hand-picked-to-cooperate) data.

use risksieve::anytime::AnytimeController;
use risksieve::{BoundedLoss, ClosedInterval, GuaranteeKind, RiskSieveError};

struct ExceedsThreshold;

impl BoundedLoss<f64, f64> for ExceedsThreshold {
    fn bounds(&self) -> ClosedInterval {
        ClosedInterval::new(0.0, 1.0).unwrap()
    }

    fn evaluate(&self, observation: &f64, parameter: &f64) -> Result<f64, RiskSieveError> {
        Ok(if observation > parameter { 1.0 } else { 0.0 })
    }
}

fn controller(alpha: f64, delta: f64) -> AnytimeController<ExceedsThreshold, f64> {
    let candidates: Vec<f64> = (0..=20).map(|i| i as f64 / 20.0).collect();
    AnytimeController::builder()
        .target_risk(alpha)
        .unwrap()
        .failure_probability(delta)
        .unwrap()
        .loss_bound(1.0)
        .unwrap()
        .loss(ExceedsThreshold)
        .candidates(candidates)
        .build()
        .unwrap()
}

/// `m*` at the paper's own Section 6.1 simulation parameters (alpha = 5%,
/// delta = 10%, B = 1), independently computed in Python; see
/// `src/anytime/boundary.rs` for the script.
#[test]
fn anytime_theorem_4_1_m_star_at_paper_simulation_parameters() {
    let c = controller(0.05, 0.10);
    assert_eq!(c.minimum_eligible_calibration_size(), 325);
}

/// "The threshold sequence will be non-increasing and, when necessary, we
/// use a running minimum." This must hold over a long, non-cooperative
/// data stream, not just the short hand-picked fixture in
/// `calibration.rs`. Scores come from a fixed low-discrepancy sequence
/// (golden-ratio increments) rather than a hand-picked cooperative
/// pattern, so the raw per-step threshold is not monotone by accident.
#[test]
fn anytime_theorem_4_1_deployed_parameter_is_non_increasing() {
    let mut c = controller(0.3, 0.2);
    let mut previous = f64::INFINITY;
    let mut saw_uninformative = false;
    let mut saw_informative = false;

    for i in 0..2000u32 {
        let score = ((i as f64) * 0.618_033_988_75).fract();
        let certificate = c.update(&score).unwrap();

        assert!(
            certificate.parameter <= previous,
            "deployed parameter increased at step {i}: {} -> {}",
            previous,
            certificate.parameter
        );
        previous = certificate.parameter;

        assert_eq!(certificate.guarantee, GuaranteeKind::AnytimeHighProbability);
        assert_eq!(certificate.calibration_size, (i + 1) as usize);

        match certificate.diagnostics.uninformative_result {
            Some(true) => saw_uninformative = true,
            Some(false) => saw_informative = true,
            None => panic!("uninformative_result must be populated"),
        }
    }

    // With 2000 updates at these parameters the run must pass through
    // both the pre-m* fallback and the post-m* search regime; otherwise
    // this test would not actually be exercising both code paths.
    assert!(saw_uninformative, "never hit the uninformative fallback");
    assert!(saw_informative, "never certified an informative parameter");
}
