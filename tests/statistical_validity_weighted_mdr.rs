//! Tier 4 statistical validity: Monte Carlo check for weighted SCoRE-MDR
//! under covariate shift (Equation 6.1, Theorem 6.2; `docs/validation.md`'s
//! tier 4, opened by `tests/statistical_validity.rs` for unweighted SDR).
//!
//! ## Data-generating process
//!
//! Calibration covariates `X_i ~ P = Uniform(0,1)`, `i = 1..n`. The test
//! covariate `X_{n+1} ~ Q`, with density `q(x) = 2x` on `[0,1]` (sampled by
//! inverse CDF: `X = sqrt(U)`, `U ~ Uniform(0,1)`) -- a deliberately simple
//! shift with a closed-form, exactly *known* density ratio
//! `w(x) = dQ/dP(x) = q(x)/p(x) = 2x`, satisfying Assumption 6.1 (`w`
//! depends only on the covariate, not the label). Every point's loss,
//! calibration or test, is drawn `L | X ~ Bernoulli(X)` -- the *same*
//! conditional law under both `P` and `Q`, since Assumption 6.1 requires
//! `P` and `Q` to share the conditional distribution of `Y` given `X` and
//! differ only in `X`'s marginal. The score is `s(x) = x`.
//!
//! Weights are `w(X_i) = 2*X_i` for every calibration point and the test
//! point alike (Equation 6.1 weights calibration points by `w(X_i)`, not
//! by `1`), passed to `certify_weighted` as
//! `ImportanceWeightSource::KnownDensityRatio`.
//!
//! A no-shift control replaces `Q` with `P` itself and every weight with
//! `1.0`, checking that `certify_weighted` behaves sanely when there is, in
//! fact, no shift.
//!
//! ## What this test does and does not establish
//!
//! Theorem 6.2 is a finite-sample guarantee for `KnownDensityRatio`; this
//! Monte Carlo check is consistent with that theorem holding, for this one
//! DGP, at this one `alpha`. It is deliberately **not** run against
//! `ImportanceWeightSource::Estimated` at all: a Monte Carlo pass under any
//! estimated-weight DGP would only ever bear on Theorem 6.4's asymptotic
//! (`limsup`) conclusion, not on a finite-sample claim, and could easily be
//! misread as validating one -- see `mdr.rs`'s module docs for the
//! `Asymptotic` downgrade this crate already applies to that case.
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
//! `R` i.i.d. replications.

use risksieve::selective::mdr::certify_weighted;
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

    /// `X ~ Q`, density `q(x) = 2x` on `[0,1]`, by inverse CDF.
    fn next_shifted_covariate(&mut self) -> f64 {
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
    shifted_mdr_contribution: f64,
    control_mdr_contribution: f64,
}

fn run_replication(
    rng: &mut SplitMix64,
    alpha: OpenUnitInterval,
    gamma: OpenUnitInterval,
) -> Replication {
    let mut calibration_scores = Vec::with_capacity(CALIBRATION_SIZE);
    let mut calibration_losses = Vec::with_capacity(CALIBRATION_SIZE);
    for _ in 0..CALIBRATION_SIZE {
        let x = rng.next_unit();
        let loss = rng.next_bernoulli(x);
        calibration_scores.push(x);
        calibration_losses.push(ClosedUnitInterval::new("loss", loss).unwrap());
    }
    let calibration_weights_shifted: Vec<NonNegative> = calibration_scores
        .iter()
        .map(|&x| weight(2.0 * x))
        .collect();
    let calibration_weights_control: Vec<NonNegative> =
        calibration_scores.iter().map(|_| weight(1.0)).collect();

    let test_x_shifted = rng.next_shifted_covariate();
    let test_loss_shifted = rng.next_bernoulli(test_x_shifted);
    let test_x_control = rng.next_unit();
    let test_loss_control = rng.next_bernoulli(test_x_control);

    let shifted_certificate = certify_weighted(
        &calibration_losses,
        &calibration_scores,
        &calibration_weights_shifted,
        test_x_shifted,
        weight(2.0 * test_x_shifted),
        alpha,
        gamma,
        ImportanceWeightSource::KnownDensityRatio,
    )
    .expect("valid simulated input");

    let control_certificate = certify_weighted(
        &calibration_losses,
        &calibration_scores,
        &calibration_weights_control,
        test_x_control,
        weight(1.0),
        alpha,
        gamma,
        ImportanceWeightSource::KnownDensityRatio,
    )
    .expect("valid simulated input");

    Replication {
        shifted_mdr_contribution: if shifted_certificate.parameter {
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

fn run_simulation(repetitions: u32) -> (f64, f64) {
    let alpha = OpenUnitInterval::new("alpha", ALPHA).unwrap();
    let gamma = OpenUnitInterval::new("gamma", GAMMA).unwrap();
    let mut rng = SplitMix64::new(SEED);

    let mut shifted_sum = 0.0_f64;
    let mut control_sum = 0.0_f64;
    for _ in 0..repetitions {
        let replication = run_replication(&mut rng, alpha, gamma);
        shifted_sum += replication.shifted_mdr_contribution;
        control_sum += replication.control_mdr_contribution;
    }

    (
        shifted_sum / repetitions as f64,
        control_sum / repetitions as f64,
    )
}

/// Fast, deterministic CI smoke test.
///
/// Recorded per AGENTS.md section 9.4 / this crate's Tier 4 policy:
/// - RNG: hand-rolled SplitMix64 (see module docs), seed `20260730`.
/// - Repetitions: `500`.
/// - DGP: `KnownDensityRatio` covariate shift (`X ~ Q`, density `2x`,
///   weight `w(x) = 2x`) plus a no-shift `weight = 1` control, both against
///   calibration `X_i ~ Uniform(0,1)`; see module docs.
/// - Calibration size `30`, `alpha = gamma = 0.3`.
/// - Acceptance: `observed_mean <= alpha + hoeffding_half_width(500, 0.05)`
///   (half-width `~0.061` at this repetition count -- wide enough that this
///   smoke test mainly catches a badly broken implementation, not a tight
///   bound; see `docs/validation.md` and the slower test below).
/// - risksieve version: `env!("CARGO_PKG_VERSION")`.
#[test]
fn weighted_mdr_monte_carlo_smoke_test() {
    const REPETITIONS: u32 = 500;
    let half_width = hoeffding_half_width(REPETITIONS, DELTA);
    let (shifted_mean, control_mean) = run_simulation(REPETITIONS);

    println!(
        "risksieve {}: weighted MDR smoke test, {REPETITIONS} reps, half_width={half_width:.4}, \
         shifted_mean={shifted_mean:.4}, control_mean={control_mean:.4}",
        env!("CARGO_PKG_VERSION")
    );

    assert!(
        shifted_mean <= ALPHA + half_width,
        "shifted weighted MDR {shifted_mean} exceeds alpha {ALPHA} + half-width {half_width}"
    );
    assert!(
        control_mean <= ALPHA + half_width,
        "control (no-shift) MDR {control_mean} exceeds alpha {ALPHA} + half-width {half_width}"
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
    let (shifted_mean, control_mean) = run_simulation(REPETITIONS);

    println!(
        "risksieve {}: weighted MDR large-scale test, {REPETITIONS} reps, \
         half_width={half_width:.5}, shifted_mean={shifted_mean:.5}, control_mean={control_mean:.5}",
        env!("CARGO_PKG_VERSION")
    );

    assert!(
        shifted_mean <= ALPHA + half_width,
        "shifted weighted MDR {shifted_mean} exceeds alpha {ALPHA} + half-width {half_width}"
    );
    assert!(
        control_mean <= ALPHA + half_width,
        "control (no-shift) MDR {control_mean} exceeds alpha {ALPHA} + half-width {half_width}"
    );
}
