//! Generic eBH (e-value Benjamini-Hochberg) selection, Theorem 3.3.
//!
//! Bai and Jin (2026), arXiv:2603.24704, Theorem 3.3: given `m` e-values
//! `E_1, ..., E_m`, each individually satisfying Definition 3.1 (see
//! [`crate::selective::evalue`]) for its own test point, applying eBH at
//! level `alpha` selects
//!
//! ```text
//! R = { j in [m] : E_j >= m / (alpha * tau_hat) }
//! tau_hat = max{ tau in {1, ..., m} : |{ j : E_j >= m/(alpha*tau) }| >= tau }
//! ```
//!
//! and this controls the Selective Deployment Risk:
//!
//! ```text
//! SDR := E[ (sum_{j=1}^m L_j * 1{j in R}) / (1 v |R|) ] <= alpha
//! ```
//!
//! **This function is deliberately generic over how the e-values were
//! constructed.** Theorem 3.3's hypothesis is only that each `E_j`
//! individually obeys Definition 3.1 -- it does not require the specific
//! per-test-point construction of Equation 5.1 (see
//! [`crate::selective::sdr`] for why this crate uses [`crate::selective::evalue`]'s
//! simpler, already-verified construction instead of Equation 5.1's
//! cross-test-point-coupled one). Any caller with `m` values each
//! individually satisfying Definition 3.1 can use this directly.
//!
//! ## Zero selection
//!
//! `tau_hat` is undefined (`None`) when no candidate `tau` satisfies the
//! condition; `R` is then empty. This is a valid outcome, not an error --
//! `SDR = 0 <= alpha` holds trivially via the `1 v |R|` denominator.
//!
//! ## Tie-breaking
//!
//! The paper does not specify how to break ties among equal e-values.
//! This implementation sorts descending via [`f64::total_cmp`] with a
//! stable sort, so ties keep their original (ascending index) relative
//! order -- a deliberate, documented choice (AGENTS.md's "deterministic
//! ordering and tie handling" requirement), not one derived from the
//! paper. It does not change the *value* of `tau_hat` (that depends only
//! on the sorted values, not which index carries which value), only the
//! order in which equal values are considered; the final selected set is
//! always determined by comparing every e-value directly against the
//! resulting threshold, never by taking a "top-`tau_hat`" slice of the
//! sorted array (those two differ exactly when values tie at the
//! threshold).
//!
//! **Provenance:** this paper postdates this project's training-data
//! cutoff. Theorem 3.3's selection rule and SDR guarantee were
//! cross-checked across independent fetches of the paper's own text and
//! agreed digit-for-digit.

use crate::probability::{NonNegative, OpenUnitInterval};

/// The result of applying eBH (Theorem 3.3) to `m` e-values.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EbhSelection {
    /// Indices into the input slice that were selected, sorted ascending.
    pub selected_indices: Vec<usize>,
    /// The critical value `tau_hat`; `None` if the selected set is empty.
    pub tau_hat: Option<usize>,
}

/// Applies eBH at level `alpha` to `evalues`, where `evalues[j]` is
/// assumed to individually satisfy Definition 3.1 for test point `j`.
///
/// An empty `evalues` slice returns an empty selection with `tau_hat:
/// None`, matching the zero-selection convention rather than being
/// treated as an error -- there is nothing invalid about a batch of zero
/// test points.
///
/// # Example
///
/// ```
/// use risksieve::selective::ebh::select;
/// use risksieve::{NonNegative, OpenUnitInterval};
///
/// let evalues = [
///     NonNegative::new("e", 10.0).unwrap(),
///     NonNegative::new("e", 0.1).unwrap(),
///     NonNegative::new("e", 8.0).unwrap(),
/// ];
/// let alpha = OpenUnitInterval::new("alpha", 0.5).unwrap();
/// let selection = select(&evalues, alpha);
/// // Worked out in tests/paper_score_sdr.rs.
/// assert_eq!(selection.selected_indices, vec![0, 2]);
/// ```
pub fn select(evalues: &[NonNegative], alpha: OpenUnitInterval) -> EbhSelection {
    let m = evalues.len();
    if m == 0 {
        return EbhSelection {
            selected_indices: Vec::new(),
            tau_hat: None,
        };
    }

    let mut sorted_desc: Vec<f64> = evalues.iter().map(|e| e.get()).collect();
    sorted_desc.sort_by(|a, b| b.total_cmp(a));

    // The predicate is not guaranteed monotone in `tau` for an arbitrary
    // e-value multiset, so every candidate is checked and the largest
    // satisfying one wins, matching the `max{...}` in Theorem 3.3.
    let mut tau_hat: Option<usize> = None;
    for tau in 1..=m {
        let threshold = m as f64 / (alpha.get() * tau as f64);
        if sorted_desc[tau - 1] >= threshold {
            tau_hat = Some(tau);
        }
    }

    let selected_indices = match tau_hat {
        None => Vec::new(),
        Some(tau) => {
            let threshold = m as f64 / (alpha.get() * tau as f64);
            let mut indices: Vec<usize> = evalues
                .iter()
                .enumerate()
                .filter(|(_, e)| e.get() >= threshold)
                .map(|(index, _)| index)
                .collect();
            indices.sort_unstable();
            indices
        }
    };

    EbhSelection {
        selected_indices,
        tau_hat,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn evalue(v: f64) -> NonNegative {
        NonNegative::new("e", v).unwrap()
    }

    #[test]
    fn empty_batch_selects_nothing() {
        let alpha = OpenUnitInterval::new("alpha", 0.5).unwrap();
        let selection = select(&[], alpha);
        assert_eq!(selection.selected_indices, Vec::<usize>::new());
        assert_eq!(selection.tau_hat, None);
    }

    /// Hand trace: m=3, alpha=0.5, e-values [10.0, 0.1, 8.0] (indices
    /// 0,1,2). Sorted descending: [10.0, 8.0, 0.1].
    ///
    /// - tau=1: threshold = 3/(0.5*1) = 6.0; sorted[0]=10.0 >= 6.0: holds.
    /// - tau=2: threshold = 3/(0.5*2) = 3.0; sorted[1]=8.0 >= 3.0: holds.
    /// - tau=3: threshold = 3/(0.5*3) = 2.0; sorted[2]=0.1 >= 2.0: fails.
    ///
    /// Largest satisfying tau is 2, so threshold = 3.0; indices with
    /// e-value >= 3.0 are {0 (10.0), 2 (8.0)}.
    #[test]
    fn matches_hand_computation() {
        let alpha = OpenUnitInterval::new("alpha", 0.5).unwrap();
        let evalues = [evalue(10.0), evalue(0.1), evalue(8.0)];
        let selection = select(&evalues, alpha);
        assert_eq!(selection.tau_hat, Some(2));
        assert_eq!(selection.selected_indices, vec![0, 2]);
    }

    #[test]
    fn no_qualifying_tau_selects_nothing() {
        // All e-values far too small to clear any threshold at this alpha.
        let alpha = OpenUnitInterval::new("alpha", 0.1).unwrap();
        let evalues = [evalue(0.01), evalue(0.02)];
        let selection = select(&evalues, alpha);
        assert_eq!(selection.tau_hat, None);
        assert_eq!(selection.selected_indices, Vec::<usize>::new());
    }

    #[test]
    fn ties_do_not_change_which_values_are_selected() {
        let alpha = OpenUnitInterval::new("alpha", 0.5).unwrap();
        // Two tied at the value that ends up defining the threshold.
        let evalues = [evalue(4.0), evalue(4.0), evalue(0.1)];
        let selection = select(&evalues, alpha);
        // m=3: tau=1 threshold=6.0 (4.0 fails); tau=2 threshold=3.0 (4.0
        // holds); tau=3 threshold=2.0 (0.1 fails). tau_hat=2, threshold=3.0,
        // both tied 4.0-valued indices (0 and 1) clear it.
        assert_eq!(selection.tau_hat, Some(2));
        assert_eq!(selection.selected_indices, vec![0, 1]);
    }

    proptest::proptest! {
        #[test]
        fn selected_set_is_permutation_invariant(
            mut values in proptest::collection::vec(0.0f64..20.0, 0..10),
            shuffle_keys in proptest::collection::vec(0i32..1000, 0..10),
            alpha_num in 1u32..16,
        ) {
            let n = values.len();
            let mut shuffle_keys = shuffle_keys;
            shuffle_keys.resize(n, 0);
            let alpha = OpenUnitInterval::new("alpha", alpha_num as f64 / 16.0).unwrap();

            let evalues: Vec<NonNegative> = values.iter().map(|&v| evalue(v)).collect();
            let original = select(&evalues, alpha);
            let original_values: Vec<f64> = original
                .selected_indices
                .iter()
                .map(|&i| values[i])
                .collect();

            let mut order: Vec<usize> = (0..n).collect();
            order.sort_by_key(|&i| shuffle_keys[i]);
            values = order.iter().map(|&i| values[i]).collect();
            let permuted_evalues: Vec<NonNegative> = values.iter().map(|&v| evalue(v)).collect();
            let permuted = select(&permuted_evalues, alpha);
            let permuted_values: Vec<f64> = permuted
                .selected_indices
                .iter()
                .map(|&i| values[i])
                .collect();

            let mut original_sorted = original_values;
            original_sorted.sort_by(f64::total_cmp);
            let mut permuted_sorted = permuted_values;
            permuted_sorted.sort_by(f64::total_cmp);

            proptest::prop_assert_eq!(original.tau_hat, permuted.tau_hat);
            proptest::prop_assert_eq!(original_sorted, permuted_sorted);
        }
    }
}
