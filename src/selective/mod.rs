//! Selective deployment via risk-adjusted e-values (SCoRE).
//!
//! Implements Bai and Jin (2026), *Conformal Selective Prediction with
//! General Risk Control*, arXiv:2603.24704. See [`evalue`] for the
//! e-value construction (Definition 3.1, Equation 4.1), [`mdr`] for the
//! SCoRE-MDR deployment decision (Algorithm 1, Theorem 3.2), [`ebh`] for
//! the generic eBH selection engine (Theorem 3.3), and [`sdr`] for the
//! batch SCoRE-SDR entry point (Algorithm 2) built on top of them.

pub mod ebh;
pub mod evalue;
pub mod mdr;
pub mod sdr;
