# Guarantee taxonomy

`risksieve` never collapses a risk-control result into a single boolean
such as `is_valid`. Every certificate carries a
[`GuaranteeKind`](../src/guarantee.rs), and every `GuaranteeKind` means
exactly one of the following. Read this file before trusting or displaying
a certificate's guarantee.

| Variant | Quantity controlled | Sample regime | Source |
|---|---|---|---|
| `ExpectedRisk` | `E[R(theta_hat)] <= alpha` | fixed-sample | Angelopoulos, Bates, Fisch, Lei, Schuster (2024), arXiv:2208.02814 |
| `AnytimeHighProbability` | `P(risk <= alpha for all covered calibration times) >= 1 - delta` | anytime-valid | Hultberg, Zachariah, Ribeiro (2026), arXiv:2602.04364, Definition 2.7 |
| `MarginalDeploymentRisk` | `E[L * deploy] <= alpha` | fixed-sample, selective | Bai and Jin (2026), arXiv:2603.24704, SCoRE-MDR |
| `TotalDeploymentRisk` | `E[sum of deployed risk] <= alpha * m` | fixed-sample, selective | Bai and Jin (2026), arXiv:2603.24704 |
| `SelectiveDeploymentRisk` | `E[average risk among deployed items] <= alpha` | fixed-sample, selective | Bai and Jin (2026), arXiv:2603.24704, SCoRE-SDR |
| `Asymptotic` | depends on the specific limiting argument used (for example a consistent but not exactly known importance-weight estimator) | limiting, not finite-sample | varies; the certificate's diagnostics must say which limit |
| `EmpiricalOnly` | no theorem-backed quantity; a diagnostic | none | none — must never be described as certified |

## Rules for using this taxonomy

- A `GuaranteeKind` describes the guarantee itself, not the confidence a
  user should place in the implementation. Even a correct implementation of
  `ExpectedRisk` says nothing about `AnytimeHighProbability`.
- `Asymptotic` and `EmpiricalOnly` are not weaker versions of a
  finite-sample guarantee; they are a different kind of claim and must be
  presented as such in any downstream tooling.
- Every certificate's `assumptions` field (see `assumptions.md`) must be
  consistent with its `guarantee`. For example, an `ExpectedRisk`
  certificate whose `stability` is `StabilityEvidence::Unknown` for a
  non-monotonic method is a bug, not a valid combination — see AGENTS.md
  section 6.4.
- Extending this enum for a new guarantee (for example a future
  distribution-shift-specific variant) requires updating this table in the
  same change.

## Status

Three controllers populate this taxonomy so far:

- `crc::monotone::certify` (Milestone 1) produces `ExpectedRisk`.
- `anytime::AnytimeController::update` (Milestone 2) produces
  `AnytimeHighProbability`.
- `nonmonotone::stability::certify` (Milestone 3) produces `ExpectedRisk`
  normally, and downgrades to `EmpiricalOnly` when its `StabilityEvidence`
  is `Estimated` rather than `Analytic`/`UserSupplied` — the "is a bug"
  rule above (an `ExpectedRisk` certificate for a non-monotonic method
  with `stability: Unknown`) is enforced by that function rejecting
  `Unknown` outright, not left as a documentation-only promise.
- `selective::mdr::certify` (Milestone 4) produces `MarginalDeploymentRisk`.
  Unlike the other three controllers, the certified bound is on
  `E[loss * deploy]` over the joint draw of calibration and test data, not
  a property of the single returned decision — see `src/selective/mdr.rs`
  module docs for why this certificate's `parameter: bool` must not be
  read as "this deployment has risk `<= alpha`." `TotalDeploymentRisk` is
  not separately populated: it follows immediately by summing `m`
  independent `MarginalDeploymentRisk` certificates at the same `alpha`
  (documented, not computed).
- `selective::sdr::certify` (Milestone 5) produces `SelectiveDeploymentRisk`
  for a batch of test points. Like MDR, the bound is on the expectation of
  a ratio over the joint draw, not a property of the one realized selected
  set; a realized batch's actual selective risk is only recoverable
  post-hoc via `selective::sdr::realized_selective_risk`, which returns a
  plain `f64` with no `GuaranteeKind` attached specifically so it cannot
  be mistaken for a second certificate. An empty selection is a valid
  `SelectiveDeploymentRisk` certificate (the guarantee holds trivially via
  the `1 v |R|` denominator), not an error.
- `anytime::AnytimeShiftedController::update` (Milestone 6) produces
  `AnytimeHighProbability` when `weight_source` is
  `ImportanceWeightSource::KnownDensityRatio` (Theorem 4.7's actual
  hypothesis), and `Asymptotic` when it is `Estimated` instead — the same
  downgrade pattern as `StabilityEvidence::Estimated`, applied here
  because the paper establishes no finite-sample guarantee for estimated
  weights. This is the first controller to actually populate `Asymptotic`
  and the first to use `ShiftAssumption::CovariateShift` for anything
  other than a placeholder value.

Six of the seven `GuaranteeKind` variants are now populated by a shipped
controller (`ExpectedRisk`, `AnytimeHighProbability`,
`MarginalDeploymentRisk`, `SelectiveDeploymentRisk`, `Asymptotic`,
`EmpiricalOnly`); only `TotalDeploymentRisk` remains documented-only (see
above). See `validation.md` for how each variant is tested.
