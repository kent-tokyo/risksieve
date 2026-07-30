//! `risksieve` sieves predictions and decisions through finite-sample and
//! anytime-valid conformal risk guarantees.
//!
//! # Scientific basis
//!
//! Implementations in this crate are traceable to:
//!
//! - Angelopoulos, Bates, Fisch, Lei, and Schuster (2024), **Conformal Risk
//!   Control**, ICLR 2024, arXiv:2208.02814 — classical finite-sample
//!   expected-risk control for bounded monotone losses.
//! - Hultberg, Zachariah, and Ribeiro (2026), **Anytime-Valid Conformal
//!   Risk Control**, arXiv:2602.04364 — high-probability risk control over
//!   growing calibration sets.
//! - Angelopoulos (2026), **Conformal Risk Control for Non-Monotonic
//!   Losses**, arXiv:2602.20151 — risk control via symmetry and
//!   beta-stability for non-monotonic, multidimensional parameters.
//! - Bai and Jin (2026), **Conformal Selective Prediction with General
//!   Risk Control**, arXiv:2603.24704 — risk-adjusted e-values, MDR, and
//!   SDR for selective deployment.
//!
//! See `docs/references.md` for the complete bibliography and the mapping
//! from each theorem to the code that implements it.
//!
//! # Example
//!
//! The vocabulary composes even where no controller exists yet: validated
//! inputs, a checked loss evaluation, and a certificate that names its
//! own guarantee and every assumption it rests on. See
//! [`crc::monotone::certify`] for a worked example of an actual
//! theorem-backed certificate.
//!
//! ```
//! use risksieve::{
//!     Assumptions, BoundedLoss, ClosedInterval, Diagnostics, ExchangeabilityAssumption,
//!     GuaranteeKind, MonotonicityAssumption, OpenUnitInterval, RiskCertificate,
//!     ShiftAssumption, StabilityEvidence, SymmetryAssumption, ZeroOneLoss,
//! };
//!
//! // Every alpha is validated once, not passed around as a raw `f64`.
//! let alpha = OpenUnitInterval::new("alpha", 0.1)?;
//!
//! // Losses are bounded by contract and checked at evaluation time,
//! // never silently clamped.
//! let observed = ZeroOneLoss.evaluate_checked(&"cat", &"dog")?;
//! assert_eq!(observed, 1.0);
//!
//! // A certificate always carries its guarantee kind and every assumption
//! // it depends on. This one is intentionally `EmpiricalOnly`: no theorem
//! // has been implemented that would back a stronger claim.
//! let certificate = RiskCertificate {
//!     parameter: 0.3_f64,
//!     target_risk: alpha.get(),
//!     certified_upper_bound: 1.0,
//!     guarantee: GuaranteeKind::EmpiricalOnly,
//!     assumptions: Assumptions {
//!         exchangeability: ExchangeabilityAssumption::Exchangeable,
//!         bounded_loss: ClosedInterval::new(0.0, 1.0)?,
//!         monotonicity: MonotonicityAssumption::NonMonotone,
//!         right_continuity: false,
//!         symmetry: SymmetryAssumption::NotEstablished,
//!         stability: StabilityEvidence::Unknown,
//!         shift: ShiftAssumption::NoShift,
//!     },
//!     calibration_size: 0,
//!     diagnostics: Diagnostics::default(),
//! };
//! assert_eq!(certificate.guarantee, GuaranteeKind::EmpiricalOnly);
//! # Ok::<(), risksieve::RiskSieveError>(())
//! ```
//!
//! # Status
//!
//! - **Milestone 0** (vocabulary): validated numeric types
//!   ([`probability`]), the bounded-loss contract ([`loss`]), the
//!   guarantee and assumption taxonomy ([`guarantee`]), the certificate
//!   type ([`certificate`]), and the error taxonomy ([`error`]). Done.
//! - **Milestone 1** (classical monotone CRC): [`crc::monotone::certify`]
//!   implements Angelopoulos, Bates, Fisch, Lei, and Schuster (2024),
//!   Theorem 1. Done.
//! - **Milestone 2** (anytime-valid monotone CRC):
//!   [`anytime::AnytimeController`] implements Hultberg, Zachariah, and
//!   Ribeiro (2026), Theorem 4.1 and Definition 2.7, over a cumulatively
//!   growing calibration stream. Done.
//! - **Milestone 3** (non-monotonic CRC), partial:
//!   [`nonmonotone::stability::certify`] implements Angelopoulos (2026),
//!   Theorem 1, the general symmetry + beta-stability reduction. The
//!   paper's concrete stability instances (discretized losses, Lipschitz
//!   losses, selective classification, regularized ERM) are not
//!   implemented yet.
//! - **Milestone 4** (SCoRE-MDR), partial: [`selective::mdr::certify`]
//!   implements Bai and Jin (2026), Algorithm 1 and Theorem 3.2 —
//!   risk-adjusted e-values ([`selective::evalue`]), the direct
//!   deployment decision, an explicit `gamma` parameter, and implied
//!   total-deployment-risk reporting.
//! - **Milestone 5** (SCoRE-SDR), partial: [`selective::sdr::certify`]
//!   implements Bai and Jin (2026), Algorithm 2 and Theorem 3.3 — batch
//!   test-item handling, a generic eBH selection engine
//!   ([`selective::ebh`]), zero-selection as a valid certificate, and a
//!   post-hoc realized-risk helper. Reuses Milestone 4's per-test-point
//!   e-value construction independently for each batch item rather than
//!   the paper's own cross-test-point-coupled construction (Equation
//!   5.1); see [`selective::sdr`]'s module docs for why.
//! - **Milestone 6** (distribution shift), partial:
//!   [`anytime::AnytimeShiftedController`] implements Hultberg,
//!   Zachariah, and Ribeiro (2026), Theorem 4.7 — importance-weighted
//!   anytime-valid risk control, with weight validation and diagnostics
//!   in [`shift::importance`], `m*` discovered at runtime as a stopping
//!   time on the realized weights, and a [`guarantee::GuaranteeKind::Asymptotic`]
//!   downgrade for [`guarantee::ImportanceWeightSource::Estimated`]
//!   weights. Weighted SCoRE is not implemented yet.
//! - Milestone 7 (downstream examples) is not implemented yet; see
//!   AGENTS.md section 7 for the planned sequence.
//!
//! # Safety posture
//!
//! This crate produces statistical certificates, not generic confidence
//! scores. Every [`certificate::RiskCertificate`] states exactly what is
//! guaranteed, under which [`guarantee::Assumptions`], via its
//! [`guarantee::GuaranteeKind`]. Never treat a
//! [`guarantee::GuaranteeKind::EmpiricalOnly`] or
//! [`guarantee::GuaranteeKind::Asymptotic`] result as a finite-sample
//! theorem-backed guarantee.

pub mod anytime;
pub mod certificate;
pub mod crc;
pub mod error;
pub mod guarantee;
pub mod loss;
pub mod nonmonotone;
pub mod numerics;
pub mod probability;
pub mod selective;
pub mod shift;

pub use certificate::{Diagnostics, EValue, RiskCertificate};
pub use error::RiskSieveError;
pub use guarantee::{
    Assumptions, ExchangeabilityAssumption, GuaranteeKind, ImportanceWeightSource,
    MonotonicityAssumption, ShiftAssumption, StabilityEstimationMethod, StabilityEvidence,
    SymmetryAssumption,
};
pub use loss::{AbsoluteErrorLoss, BoundedLoss, ZeroOneLoss};
pub use probability::{ClosedInterval, ClosedUnitInterval, NonNegative, OpenUnitInterval};
