//! Informal wall-clock comparison between `sdr::certify` (coupled,
//! Equation 5.1) and `sdr::certify_independent` (Equation 4.1 applied per
//! test point), at three fixed input sizes.
//!
//! This is a one-off comparative measurement, not a tracked performance
//! regression benchmark: no `criterion` dependency was added for it,
//! since a single point-in-time comparison across two code paths that
//! both already have an established `O(...)` bound (see
//! `src/selective/coupled.rs` and `src/selective/evalue.rs`'s module
//! docs) does not need statistical benchmark infrastructure to be
//! informative -- it needs a fixed input, a warm-up, several repetitions,
//! and a median. If this crate later needs regression tracking across
//! commits, that is a `criterion`-justifying change on its own.
//!
//! Not run by default (`#[ignore]`); per this crate's timing-methodology
//! requirement, run it in release mode so the numbers reflect optimized
//! code, not debug-build overhead:
//!
//! ```bash
//! cargo test --release --test timing_comparison -- --ignored --nocapture
//! ```
//!
//! **What this does and does not show:** `certify`'s coupled construction
//! does a per-test-point `O(n+m)` scan (`coupled_risk_adjusted_evalues`);
//! `certify_independent`'s per-test-point cost is `O(n^2)`
//! (`risk_adjusted_evalue`'s reference breakpoint scan, `evalue.rs`'s own
//! module docs). At the sizes below, whichever is faster depends on how
//! `n` and `m` relate, not a fixed ordering -- this file reports what was
//! actually measured at these three sizes, not a general "X is always
//! faster than Y" claim.

use risksieve::selective::sdr::{certify, certify_independent};
use risksieve::{ClosedUnitInterval, OpenUnitInterval};
use std::time::{Duration, Instant};

/// Deterministic, RNG-free pseudo-spread: no statistical properties are
/// needed here (unlike `statistical_validity.rs`'s SplitMix64), only a
/// fixed, reproducible, non-degenerate input.
fn pseudo_score(index: usize) -> f64 {
    ((index as u64).wrapping_mul(2_654_435_761) % 1_000_000) as f64 / 1000.0 - 500.0
}

fn pseudo_loss(index: usize) -> f64 {
    ((index as u64).wrapping_mul(40_503).wrapping_add(7) % 1000) as f64 / 1000.0
}

struct Fixture {
    calibration_losses: Vec<ClosedUnitInterval>,
    calibration_scores: Vec<f64>,
    test_scores: Vec<f64>,
}

fn build_fixture(n: usize, m: usize) -> Fixture {
    let calibration_losses = (0..n)
        .map(|i| ClosedUnitInterval::new("loss", pseudo_loss(i)).unwrap())
        .collect();
    let calibration_scores = (0..n).map(pseudo_score).collect();
    let test_scores = (n..n + m).map(pseudo_score).collect();
    Fixture {
        calibration_losses,
        calibration_scores,
        test_scores,
    }
}

fn median(mut samples: Vec<Duration>) -> Duration {
    samples.sort();
    samples[samples.len() / 2]
}

const WARMUP_ITERATIONS: usize = 5;
const MEASURED_ITERATIONS: usize = 20;

fn measure(mut run_once: impl FnMut()) -> Duration {
    for _ in 0..WARMUP_ITERATIONS {
        run_once();
    }
    let samples: Vec<Duration> = (0..MEASURED_ITERATIONS)
        .map(|_| {
            let start = Instant::now();
            run_once();
            start.elapsed()
        })
        .collect();
    median(samples)
}

/// Measures both constructions at three fixed sizes and prints a table.
/// Records, per this crate's Tier-4-adjacent timing methodology: sizes
/// (`n`, `m`), warm-up count (`5`), measured repetitions (`20`, median
/// reported), `rustc` version, and target OS/arch -- see the module docs
/// for why no `criterion` dependency was added.
#[test]
#[ignore]
fn coupled_vs_independent_timing() {
    let alpha = OpenUnitInterval::new("alpha", 0.3).unwrap();
    let gamma = OpenUnitInterval::new("gamma", 0.3).unwrap();

    println!(
        "target: {}-{} (record `rustc --version` alongside this output separately -- \
         not available as a compile-time constant without a build script)",
        std::env::consts::ARCH,
        std::env::consts::OS,
    );
    println!(
        "warm-up: {WARMUP_ITERATIONS} iterations, measured: {MEASURED_ITERATIONS} iterations, reporting median"
    );
    println!(
        "{:>10} {:>6} {:>6} {:>14} {:>18}",
        "size", "n", "m", "coupled (median)", "independent (median)"
    );

    for (label, n, m) in [
        ("small", 20usize, 5usize),
        ("medium", 200, 50),
        ("large", 2000, 500),
    ] {
        let fixture = build_fixture(n, m);

        let coupled_time = measure(|| {
            certify(
                &fixture.calibration_losses,
                &fixture.calibration_scores,
                &fixture.test_scores,
                alpha,
                gamma,
            )
            .unwrap();
        });
        let independent_time = measure(|| {
            certify_independent(
                &fixture.calibration_losses,
                &fixture.calibration_scores,
                &fixture.test_scores,
                alpha,
                gamma,
            )
            .unwrap();
        });

        println!(
            "{label:>10} {n:>6} {m:>6} {:>14?} {:>18?}",
            coupled_time, independent_time
        );
    }
}
