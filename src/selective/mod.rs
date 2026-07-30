//! Selective deployment via risk-adjusted e-values (SCoRE).
//!
//! Implements Bai and Jin (2026), *Conformal Selective Prediction with
//! General Risk Control*, arXiv:2603.24704. See [`evalue`] for the
//! single-test-point e-value construction (Definition 3.1, Equation 4.1),
//! [`evalue_weighted`] for its covariate-shift extension (Equation 6.1,
//! Theorem 6.2/6.4), [`mdr`] for the SCoRE-MDR deployment decision
//! (Algorithm 1, Theorem 3.2, and its weighted counterpart), [`ebh`] for
//! the generic eBH selection engine (Theorem 3.3), [`coupled`] for the
//! paper's own cross-test-point e-value (Equation 5.1, Theorem 5.1), and
//! [`sdr`] for the batch SCoRE-SDR entry point (Algorithm 2) built on top
//! of them.

pub mod coupled;
pub mod ebh;
pub mod evalue;
pub mod evalue_weighted;
pub mod mdr;
pub mod sdr;
