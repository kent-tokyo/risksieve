//! Tier 4 statistical validity: Monte Carlo check for weighted SCoRE-MDR
//! under covariate shift (Equation 6.1, Theorem 6.2; `docs/validation.md`'s
//! tier 4, opened by `tests/statistical_validity.rs` for unweighted SDR).
//!
//! ## Data-generating process
//!
//! Each point's covariate is a pair `(X1, X2)`. `X1` is the score
//! coordinate: `X1 ~ Uniform(0,1)` under *both* calibration (`P`) and test
//! (`Q`), so it carries no information about the shift. `X2` is the risk
//! coordinate: `X2 ~ P = Uniform(0,1)` for calibration, `X2 ~ Q` for the
//! test point, with `Q` having density `q(x2) = 2*x2` on `[0,1]` (sampled
//! by inverse CDF: `X2 = sqrt(U)`). `X1` and `X2` are independent, so the
//! known density ratio depends only on the covariate,
//! `w(x1,x2) = dQ/dP(x1,x2) = 2*x2`, satisfying Assumption 6.1. The loss
//! is `L | X ~ Bernoulli(X2)` under both `P` and `Q` (same conditional
//! law given the covariate), and the score is `s(x1,x2) = x1`.
//!
//! This decoupling is deliberate, not incidental: an earlier version of
//! this test used a *single* coordinate as both the score and the risk
//! driver (`s(x) = x`, `L | X ~ Bernoulli(X)`, shift on that same `x`).
//! That design was vacuous -- the unweighted procedure's score-based
//! threshold already "sees" the shift (a higher covariate both scores
//! higher and shifts more likely under `Q`), so it self-corrects for
//! exactly the shift being introduced, and passes even with every weight
//! set to `1`. Passing a check that an unweighted procedure would also
//! pass demonstrates nothing about Equation 6.1. Decoupling the score
//! (`X1`) from the risk driver (`X2`) removes that confound: the
//! unweighted procedure's threshold, based only on `X1`, cannot detect or
//! compensate for a shift that lives entirely in `X2`. See the `naive`
//! arm below, which confirms this empirically.
//!
//! ## Three arms, same replication loop
//!
//! - `weighted`: the test point is drawn from `Q`; `certify_weighted` is
//!   given the true weights `w(x1,x2) = 2*x2` and
//!   `ImportanceWeightSource::KnownDensityRatio`. Theorem 6.2 says this
//!   arm's Monte Carlo mean should respect `alpha`.
//! - `naive`: the *same* `Q`-drawn test point as `weighted`, but run
//!   through plain unweighted `certify` (equivalently, `certify_weighted`
//!   with every weight `1`), which is the wrong thing to do under shift.
//!   This arm is expected to *exceed* `alpha` -- its purpose is to prove
//!   the DGP is not vacuous, not to check a guarantee this crate makes.
//! - `control`: shares the unshifted score coordinate `X1` with the other
//!   two arms, but re-draws the risk coordinate from `P` instead of `Q`
//!   (`X2 ~ Uniform(0,1)`), weights `1`, `certify_weighted` with
//!   `KnownDensityRatio`. Sanity check that nothing breaks, and that the
//!   bound holds just as comfortably, when there is in fact no shift.
//!
//! ## What this test does and does not establish
//!
//! Theorem 6.2 is a finite-sample guarantee for `KnownDensityRatio`; the
//! `weighted` and `control` arms are consistent with that theorem holding,
//! for this one DGP, at this one `alpha`. This file deliberately does
//! **not** exercise `ImportanceWeightSource::Estimated`: a Monte Carlo pass
//! under any estimated-weight DGP would only ever bear on Theorem 6.4's
//! asymptotic (`limsup`) conclusion, not a finite-sample claim, and could
//! easily be misread as validating one -- see `mdr.rs`'s module docs for
//! the `Asymptotic` downgrade this crate already applies to that case.
//!
//! ## RNG and acceptance criterion
//!
//! Same hand-rolled SplitMix64 generator and Hoeffding-bound acceptance
//! criterion as `tests/statistical_validity.rs` (see that file's module
//! docs for the rationale); duplicated here rather than shared, per this
//! crate's preference for self-contained test files over a shared
//! test-utility module. Each replication's contribution
//! (`realized_loss * deploy_indicator`) is bounded in `[0,1]`, so the same
//! `sqrt(ln(2/delta) / (2R))` half-width applies to the Monte Carlo mean of
//! `R` i.i.d. replications, in either direction (the `naive` arm's
//! assertion is the mirror image: `mean > alpha + half_width` would be too
//! strict a bound to demand exceedance by, so it instead asserts exceedance
//! of `alpha` alone, which the observed margin clears with room to spare).

use risksieve::selective::mdr::{certify, certify_weighted};
use risksieve::{ClosedUnitInterval, ImportanceWeightSource, NonNegative, OpenUnitInterval};

const SEED: u64 = 20260730;
const CALIBRATION_SIZE: usize = 30;
const ALPHA: f64 = 0.3;
const GAMMA: f64 = 0.3;
const DELTA: f64 = 0.05;

/// SplitMix64 (Vigna, public domain) -- see `tests/statistical_validity.rs`'s
/// module docs.
struct SplitMix64(u64);

impl SplitMix64 {
    fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A uniform value in `[0, 1)`.
    fn next_unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }

    fn next_bernoulli(&mut self, p: f64) -> f64 {
        if self.next_unit() < p { 1.0 } else { 0.0 }
    }

    /// `X2 ~ Q`, density `q(x2) = 2*x2` on `[0,1]`, by inverse CDF.
    fn next_shifted_risk_coordinate(&mut self) -> f64 {
        self.next_unit().sqrt()
    }
}

fn hoeffding_half_width(repetitions: u32, delta: f64) -> f64 {
    ((2.0 / delta).ln() / (2.0 * repetitions as f64)).sqrt()
}

fn weight(v: f64) -> NonNegative {
    NonNegative::new("weight", v).unwrap()
}

struct Replication {
    weighted_mdr_contribution: f64,
    naive_mdr_contribution: f64,
    control_mdr_contribution: f64,
}

fn run_replication(
    rng: &mut SplitMix64,
    alpha: OpenUnitInterval,
    gamma: OpenUnitInterval,
) -> Replication {
    let mut scores = Vec::with_capacity(CALIBRATION_SIZE);
    let mut risk_coordinates = Vec::with_capacity(CALIBRATION_SIZE);
    let mut calibration_losses = Vec::with_capacity(CALIBRATION_SIZE);
    for _ in 0..CALIBRATION_SIZE {
        let x1 = rng.next_unit();
        let x2 = rng.next_unit();
        let loss = rng.next_bernoulli(x2);
        scores.push(x1);
        risk_coordinates.push(x2);
        calibration_losses.push(ClosedUnitInterval::new("loss", loss).unwrap());
    }
    let calibration_weights_shifted: Vec<NonNegative> = risk_coordinates
        .iter()
        .map(|&x2| weight(2.0 * x2))
        .collect();
    let calibration_weights_one: Vec<NonNegative> =
        risk_coordinates.iter().map(|_| weight(1.0)).collect();

    // Shared test point for the `weighted` and `naive` arms: score X1 is
    // unshifted, risk coordinate X2 is drawn from Q.
    let test_score = rng.next_unit();
    let test_risk_shifted = rng.next_shifted_risk_coordinate();
    let test_loss_shifted = rng.next_bernoulli(test_risk_shifted);

    let weighted_certificate = certify_weighted(
        &calibration_losses,
        &scores,
        &calibration_weights_shifted,
        test_score,
        weight(2.0 * test_risk_shifted),
        alpha,
        gamma,
        ImportanceWeightSource::KnownDensityRatio,
    )
    .expect("valid simulated input");

    let naive_certificate = certify(&calibration_losses, &scores, test_score, alpha, gamma)
        .expect("valid simulated input");

    // Independently re-drawn no-shift replication for the `control` arm.
    let test_risk_control = rng.next_unit();
    let test_loss_control = rng.next_bernoulli(test_risk_control);
    let control_certificate = certify_weighted(
        &calibration_losses,
        &scores,
        &calibration_weights_one,
        test_score,
        weight(1.0),
        alpha,
        gamma,
        ImportanceWeightSource::KnownDensityRatio,
    )
    .expect("valid simulated input");

    Replication {
        weighted_mdr_contribution: if weighted_certificate.parameter {
            test_loss_shifted
        } else {
            0.0
        },
        naive_mdr_contribution: if naive_certificate.parameter {
            test_loss_shifted
        } else {
            0.0
        },
        control_mdr_contribution: if control_certificate.parameter {
            test_loss_control
        } else {
            0.0
        },
    }
}

fn run_simulation(repetitions: u32) -> (f64, f64, f64) {
    let alpha = OpenUnitInterval::new("alpha", ALPHA).unwrap();
    let gamma = OpenUnitInterval::new("gamma", GAMMA).unwrap();
    let mut rng = SplitMix64::new(SEED);

    let mut weighted_sum = 0.0_f64;
    let mut naive_sum = 0.0_f64;
    let mut control_sum = 0.0_f64;
    for _ in 0..repetitions {
        let replication = run_replication(&mut rng, alpha, gamma);
        weighted_sum += replication.weighted_mdr_contribution;
        naive_sum += replication.naive_mdr_contribution;
        control_sum += replication.control_mdr_contribution;
    }

    (
        weighted_sum / repetitions as f64,
        naive_sum / repetitions as f64,
        control_sum / repetitions as f64,
    )
}

/// Fast, deterministic CI smoke test.
///
/// Recorded per AGENTS.md section 9.4 / this crate's Tier 4 policy:
/// - RNG: hand-rolled SplitMix64 (see module docs), seed `20260730`.
/// - Repetitions: `500`.
/// - DGP: decoupled score/risk-coordinate `KnownDensityRatio` covariate
///   shift, plus a `naive` (unweighted, on the same shifted test point)
///   arm and a no-shift `control` arm; see module docs.
/// - Calibration size `30`, `alpha = gamma = 0.3`.
/// - Acceptance: `weighted_mean <= alpha + hoeffding_half_width(500, 0.05)`
///   and same for `control_mean`; `naive_mean > alpha` (proves the DGP is
///   not vacuous -- see module docs). Half-width `~0.061` at this
///   repetition count -- wide enough that this smoke test mainly catches a
///   badly broken implementation, not a tight bound; see
///   `docs/validation.md` and the slower test below.
/// - risksieve version: `env!("CARGO_PKG_VERSION")`.
#[test]
fn weighted_mdr_monte_carlo_smoke_test() {
    const REPETITIONS: u32 = 500;
    let half_width = hoeffding_half_width(REPETITIONS, DELTA);
    let (weighted_mean, naive_mean, control_mean) = run_simulation(REPETITIONS);

    println!(
        "risksieve {}: weighted MDR smoke test, {REPETITIONS} reps, half_width={half_width:.4}, \
         weighted_mean={weighted_mean:.4}, naive_mean={naive_mean:.4}, control_mean={control_mean:.4}",
        env!("CARGO_PKG_VERSION")
    );

    assert!(
        weighted_mean <= ALPHA + half_width,
        "weighted MDR {weighted_mean} exceeds alpha {ALPHA} + half-width {half_width}"
    );
    assert!(
        control_mean <= ALPHA + half_width,
        "control (no-shift) MDR {control_mean} exceeds alpha {ALPHA} + half-width {half_width}"
    );
    assert!(
        naive_mean > ALPHA,
        "naive (unweighted-on-shifted-data) MDR {naive_mean} did not exceed alpha {ALPHA}; \
         the DGP may have become vacuous with respect to the weighting -- see module docs"
    );
}

/// Slower, larger-repetition version of the same check, for a materially
/// tighter Monte Carlo error bound. Not run by default; per AGENTS.md
/// section 17:
///
/// ```bash
/// cargo test --test statistical_validity_weighted_mdr -- --ignored --nocapture
/// ```
///
/// Same RNG, seed, DGP, calibration size, and alpha/gamma as the smoke
/// test above; repetitions raised to `20000`, giving a half-width of
/// `~0.0096` at `delta = 0.05`.
#[test]
#[ignore]
fn weighted_mdr_monte_carlo_large_scale() {
    const REPETITIONS: u32 = 20_000;
    let half_width = hoeffding_half_width(REPETITIONS, DELTA);
    let (weighted_mean, naive_mean, control_mean) = run_simulation(REPETITIONS);

    println!(
        "risksieve {}: weighted MDR large-scale test, {REPETITIONS} reps, \
         half_width={half_width:.5}, weighted_mean={weighted_mean:.5}, naive_mean={naive_mean:.5}, \
         control_mean={control_mean:.5}",
        env!("CARGO_PKG_VERSION")
    );

    assert!(
        weighted_mean <= ALPHA + half_width,
        "weighted MDR {weighted_mean} exceeds alpha {ALPHA} + half-width {half_width}"
    );
    assert!(
        control_mean <= ALPHA + half_width,
        "control (no-shift) MDR {control_mean} exceeds alpha {ALPHA} + half-width {half_width}"
    );
    assert!(
        naive_mean > ALPHA,
        "naive (unweighted-on-shifted-data) MDR {naive_mean} did not exceed alpha {ALPHA}; \
         the DGP may have become vacuous with respect to the weighting -- see module docs"
    );
}
