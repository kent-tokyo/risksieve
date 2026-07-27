//! Compensated summation for accumulated losses and weights.
//!
//! AGENTS.md section 8: "Use compensated or pairwise summation for
//! accumulated losses and weights." A naive running sum of many
//! floating-point terms accumulates rounding error; Kahan summation
//! tracks a running compensation term that cancels most of it out, at
//! the cost of a few extra flops per element.

/// Sums `values` using Kahan compensated summation.
pub fn kahan_sum(values: impl IntoIterator<Item = f64>) -> f64 {
    let mut sum = 0.0_f64;
    let mut compensation = 0.0_f64;
    for value in values {
        let adjusted = value - compensation;
        let new_sum = sum + adjusted;
        compensation = (new_sum - sum) - adjusted;
        sum = new_sum;
    }
    sum
}

/// Computes the mean of `values` using [`kahan_sum`].
///
/// Returns `0.0` for an empty input; callers that must distinguish "no
/// observations" from "mean of zero" should check length separately
/// (see [`crate::error::RiskSieveError::EmptyCalibrationSet`]).
pub fn kahan_mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    kahan_sum(values.iter().copied()) / values.len() as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input_sums_to_zero() {
        assert_eq!(kahan_sum([]), 0.0);
        assert_eq!(kahan_mean(&[]), 0.0);
    }

    #[test]
    fn matches_naive_sum_for_well_conditioned_input() {
        let values = [0.1, 0.2, 0.3, 0.4];
        let naive: f64 = values.iter().sum();
        assert!((kahan_sum(values) - naive).abs() < 1e-12);
    }

    #[test]
    fn recovers_precision_lost_by_naive_summation() {
        // Classic ill-conditioned ordering: 1000 unit increments are
        // individually too small to change a 1e16-magnitude running sum
        // (the representable gap there is 2.0), so naive summation loses
        // all of them once the large value is subtracted back out.
        let mut values = vec![1.0e16];
        values.extend(std::iter::repeat_n(1.0, 1000));
        values.push(-1.0e16);

        let naive = values.iter().fold(0.0_f64, |acc, v| acc + v);
        assert_eq!(naive, 0.0);
        assert_eq!(kahan_sum(values), 1000.0);
    }

    #[test]
    fn mean_of_repeated_value_is_exact() {
        let values = vec![0.3; 10];
        assert!((kahan_mean(&values) - 0.3).abs() < 1e-15);
    }
}
