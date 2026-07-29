//! Tier 4 statistical validity: Monte Carlo checks for SCoRE-SDR
//! (`docs/validation.md`'s tier 4, opened by this file).
//!
//! Bai and Jin (2026), arXiv:2603.24704, Theorem 3.3 / Theorem 5.1:
//! `SDR := E[ sum_j L_{n+j} * 1{j in R} / (1 v |R|) ] <= alpha`. This file
//! estimates the left-hand side by simulation and checks it against
//! `alpha` with an explicit Monte Carlo error bound, for both
//! `sdr::certify` (the coupled Equation 5.1 construction) and
//! `sdr::certify_independent` (Equation 4.1 applied per test point).
//!
//! ## Data-generating process
//!
//! Each of the `n + m` points in a replication is drawn i.i.d. as
//! `(U_i, L_i)` with `U_i ~ Uniform(0,1)` (used directly as the score
//! `s(X_i)`) and `L_i | U_i ~ Bernoulli(U_i)` (so a higher score
//! genuinely means a higher chance of loss `1`, though the crate never
//! requires this). Because every point is drawn from the identical i.i.d.
//! law, any partition into `n` calibration points and `m` test points is
//! exchangeable by construction -- the first `n` draws become calibration
//! and the remaining `m` become the test batch, with no extra shuffling
//! needed, matching Theorem 5.1's hypothesis of joint exchangeability over
//! all `n + m` points.
//!
//! ## RNG
//!
//! A hand-rolled SplitMix64 generator (Vigna, public domain; the
//! reference algorithm, not a novel construction) rather than a new
//! dependency, per this crate's dependency policy (AGENTS.md section 12)
//! and this PR's explicit scope (no new RNG dependency). It is used only
//! by this test file, never by the library.
//!
//! ## Acceptance criterion
//!
//! Per-replication SDR (`sum(selected_loss) / max(1, selected_count)`) is
//! bounded in `[0, 1]`, so Hoeffding's inequality gives, for `R`
//! repetitions and failure probability `delta`, a half-width
//! `sqrt(ln(2/delta) / (2R))` around the true mean at confidence
//! `1 - delta`. The test accepts `observed_mean <= alpha + half_width`
//! rather than asserting `observed_mean <= alpha` directly, since a finite
//! Monte Carlo estimate can exceed the true mean by chance. At the fast
//! smoke test's repetition count this half-width is wide (see the
//! constant below and `docs/validation.md`); the slower `--ignored` test
//! uses many more repetitions for a tighter check.

use risksieve::selective::sdr::{certify, certify_independent, realized_selective_risk};
use risksieve::{ClosedUnitInterval, OpenUnitInterval};

const SEED: u64 = 20260729;
const CALIBRATION_SIZE: usize = 30;
const TEST_BATCH_SIZE: usize = 8;
const ALPHA: f64 = 0.3;
const GAMMA: f64 = 0.3;
const DELTA: f64 = 0.05;

/// SplitMix64 (Vigna, public domain): a minimal, deterministic,
/// auditable PRNG -- see the module docs for why this crate hand-rolls it
/// rather than adding a dependency.
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

    /// A uniform value in `[0, 1)` from the top 53 bits (the full `f64`
    /// mantissa) of a 64-bit draw.
    fn next_unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }

    fn next_bernoulli(&mut self, p: f64) -> f64 {
        if self.next_unit() < p { 1.0 } else { 0.0 }
    }
}

fn hoeffding_half_width(repetitions: u32, delta: f64) -> f64 {
    ((2.0 / delta).ln() / (2.0 * repetitions as f64)).sqrt()
}

struct Replication {
    coupled_sdr: f64,
    independent_sdr: f64,
}

fn run_replication(
    rng: &mut SplitMix64,
    alpha: OpenUnitInterval,
    gamma: OpenUnitInterval,
) -> Replication {
    let total = CALIBRATION_SIZE + TEST_BATCH_SIZE;
    let mut scores = Vec::with_capacity(total);
    let mut losses = Vec::with_capacity(total);
    for _ in 0..total {
        let u = rng.next_unit();
        let loss = rng.next_bernoulli(u);
        scores.push(u);
        losses.push(loss);
    }

    let calibration_scores = &scores[..CALIBRATION_SIZE];
    let calibration_losses: Vec<ClosedUnitInterval> = losses[..CALIBRATION_SIZE]
        .iter()
        .map(|&l| ClosedUnitInterval::new("loss", l).unwrap())
        .collect();
    let test_scores = &scores[CALIBRATION_SIZE..];
    let test_losses_realized = &losses[CALIBRATION_SIZE..];

    let coupled_certificate = certify(
        &calibration_losses,
        calibration_scores,
        test_scores,
        alpha,
        gamma,
    )
    .expect("valid simulated input");
    let independent_certificate = certify_independent(
        &calibration_losses,
        calibration_scores,
        test_scores,
        alpha,
        gamma,
    )
    .expect("valid simulated input");

    let selected_losses = |selected: &[usize]| -> Vec<ClosedUnitInterval> {
        selected
            .iter()
            .map(|&j| ClosedUnitInterval::new("realized_loss", test_losses_realized[j]).unwrap())
            .collect()
    };

    Replication {
        coupled_sdr: realized_selective_risk(&selected_losses(&coupled_certificate.parameter)),
        independent_sdr: realized_selective_risk(&selected_losses(
            &independent_certificate.parameter,
        )),
    }
}

fn run_simulation(repetitions: u32) -> (f64, f64) {
    let alpha = OpenUnitInterval::new("alpha", ALPHA).unwrap();
    let gamma = OpenUnitInterval::new("gamma", GAMMA).unwrap();
    let mut rng = SplitMix64::new(SEED);

    let mut coupled_sum = 0.0_f64;
    let mut independent_sum = 0.0_f64;
    for _ in 0..repetitions {
        let replication = run_replication(&mut rng, alpha, gamma);
        coupled_sum += replication.coupled_sdr;
        independent_sum += replication.independent_sdr;
    }

    (
        coupled_sum / repetitions as f64,
        independent_sum / repetitions as f64,
    )
}

/// Fast, deterministic CI smoke test.
///
/// Recorded per AGENTS.md section 9.4 / this crate's Tier 4 policy:
/// - RNG: hand-rolled SplitMix64 (see module docs), seed `20260729`.
/// - Repetitions: `500`.
/// - DGP: i.i.d. `(U_i, L_i)`, `U_i ~ Uniform(0,1)` as the score,
///   `L_i | U_i ~ Bernoulli(U_i)` as the loss (see module docs).
/// - Calibration size `30`, test batch size `8` (small enough that both
///   constructions select a non-trivial number of test points on
///   average, rather than the independent construction selecting nothing
///   every replication -- its e-value's numerator is capped at
///   `calibration_size + 1`, so a larger test batch can raise the eBH
///   threshold past what it can ever clear; see the module docs).
/// - `alpha = gamma = 0.3`.
/// - Acceptance: `observed_mean_sdr <= alpha + hoeffding_half_width(500, 0.05)`
///   (half-width `~0.061` at this repetition count -- wide enough that
///   this smoke test mainly catches a badly broken implementation, not a
///   tight bound; see `docs/validation.md` and the slower test below for
///   a tighter check).
/// - risksieve version: `env!("CARGO_PKG_VERSION")`.
#[test]
fn sdr_monte_carlo_smoke_test() {
    const REPETITIONS: u32 = 500;
    let half_width = hoeffding_half_width(REPETITIONS, DELTA);
    let (coupled_mean, independent_mean) = run_simulation(REPETITIONS);

    println!(
        "risksieve {}: SDR smoke test, {REPETITIONS} reps, half_width={half_width:.4}, \
         coupled_mean={coupled_mean:.4}, independent_mean={independent_mean:.4}",
        env!("CARGO_PKG_VERSION")
    );

    assert!(
        coupled_mean <= ALPHA + half_width,
        "coupled SDR {coupled_mean} exceeds alpha {ALPHA} + half-width {half_width}"
    );
    assert!(
        independent_mean <= ALPHA + half_width,
        "independent SDR {independent_mean} exceeds alpha {ALPHA} + half-width {half_width}"
    );
}

/// Slower, larger-repetition version of the same check, for a materially
/// tighter Monte Carlo error bound. Not run by default; per AGENTS.md
/// section 17:
///
/// ```bash
/// cargo test --test statistical_validity -- --ignored --nocapture
/// ```
///
/// Same RNG, seed, DGP, calibration/test sizes, and alpha/gamma as the
/// smoke test above; repetitions raised to `20000`, giving a half-width
/// of `~0.0096` at `delta = 0.05`.
#[test]
#[ignore]
fn sdr_monte_carlo_large_scale() {
    const REPETITIONS: u32 = 20_000;
    let half_width = hoeffding_half_width(REPETITIONS, DELTA);
    let (coupled_mean, independent_mean) = run_simulation(REPETITIONS);

    println!(
        "risksieve {}: SDR large-scale test, {REPETITIONS} reps, half_width={half_width:.5}, \
         coupled_mean={coupled_mean:.5}, independent_mean={independent_mean:.5}",
        env!("CARGO_PKG_VERSION")
    );

    assert!(
        coupled_mean <= ALPHA + half_width,
        "coupled SDR {coupled_mean} exceeds alpha {ALPHA} + half-width {half_width}"
    );
    assert!(
        independent_mean <= ALPHA + half_width,
        "independent SDR {independent_mean} exceeds alpha {ALPHA} + half-width {half_width}"
    );
}
