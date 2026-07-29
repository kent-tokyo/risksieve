# AGENTS.md

## 0. Project identity

- **Project name:** `risksieve`
- **One-line description:** A Rust library that sieves predictions and decisions through finite-sample and anytime-valid conformal risk guarantees.
- **Primary language:** Rust
- **Initial deliverable:** one dependency-light library crate named `risksieve`
- **Recommended license:** dual `MIT OR Apache-2.0`
- **MSRV:** declare explicitly in `Cargo.toml`; do not raise it without a documented reason
- **Safety posture:** this library produces statistical certificates, not generic confidence scores. Every certificate must state exactly what is guaranteed and under which assumptions.
- **Name rationale:** “RiskSieve” reflects the library’s central operation: retaining, rejecting, or parameterizing predictions and decisions according to explicit risk-control guarantees.

`risksieve` is intended to become a shared statistical foundation for projects such as `masstrust`, `quietset`, `lineprior`, `veridict`, and `renkin`. The core crate must therefore remain domain-neutral. Domain-specific integrations belong in examples, adapters, or downstream repositories.

---

## 1. Mission

Build a rigorous Rust implementation that unifies four layers:

1. classical conformal risk control for bounded monotone losses;
2. anytime-valid risk control for cumulatively growing calibration data;
3. risk control for non-monotonic losses and multidimensional parameters through algorithmic stability;
4. selective deployment through SCoRE-style risk-adjusted e-values, including MDR and SDR control.

Distribution-shift support is an explicit extension, not an implicit default.

The project is successful only when users can answer all of the following from a returned value:

- What risk quantity is controlled?
- Is the guarantee in expectation or with high probability?
- Is it fixed-sample or anytime-valid?
- Does it concern all predictions or only selected/deployed predictions?
- Which assumptions were required?
- Were importance weights known or estimated?
- Was optimizer stability proven, supplied, or merely estimated?
- Is the result a theorem-backed certificate, an asymptotic statement, or an empirical diagnostic?

---

## 2. Scientific basis

Implementations must be traceable to the following sources.

### Foundational CRC

Anastasios N. Angelopoulos, Stephen Bates, Adam Fisch, Lihua Lei, and Tal Schuster.  
**Conformal Risk Control.** ICLR 2024.  
arXiv:2208.02814  
https://arxiv.org/abs/2208.02814  
https://proceedings.iclr.cc/paper_files/paper/2024/hash/f3549ef9b5ff520a7e41ff3cc306ab2b-Abstract-Conference.html

Use this as the baseline for bounded monotone losses and expected-risk control.

### Anytime-valid CRC

Bror Hultberg, Dave Zachariah, and Antônio H. Ribeiro.  
**Anytime-Valid Conformal Risk Control.** 2026.  
arXiv:2602.04364  
https://arxiv.org/abs/2602.04364

Primary implementation targets:

- Definition 2.7: anytime-valid risk control;
- Theorem 4.1: bounded monotone loss with the anytime correction term;
- Corollary 4.2: miscoverage specialization;
- Proposition 4.5: asymptotic tightness diagnostics;
- Theorem 4.7: importance-weighted extension under distribution shift.

### Non-monotonic CRC

Anastasios N. Angelopoulos.  
**Conformal Risk Control for Non-Monotonic Losses.** 2026.  
arXiv:2602.20151  
https://arxiv.org/abs/2602.20151

Primary implementation targets:

- Theorem 1: risk control from symmetry and beta-stability;
- bounded-loss discretization;
- continuous Lipschitz losses;
- selective classification special case;
- regularized ERM guarantees;
- separation between analytically proven stability and empirically estimated stability.

Reference experiment repository:  
https://github.com/aangelopoulos/nonmonotonic-crc

**Important:** the repository README displays an MIT badge, but a root `LICENSE` file was not found during the initial project review. Until the license is clarified, do not copy or translate source code from this repository. Use the paper as the mathematical specification and perform a clean-room Rust implementation.

### SCoRE

Tian Bai and Ying Jin.  
**Conformal Selective Prediction with General Risk Control.** 2026.  
arXiv:2603.24704  
https://arxiv.org/abs/2603.24704

Primary implementation targets:

- Definition 3.1: risk-adjusted e-values;
- MDR and TDR definitions;
- SDR definition;
- Algorithm 1: SCoRE-MDR;
- Algorithm 2: SCoRE-SDR;
- eBH-based selection;
- weighted extensions under covariate shift.

Reference implementation:  
https://github.com/Tian-Bai/SCoRE

The SCoRE repository is MIT-licensed. It may be used as a behavioral oracle and for independent cross-checking. Prefer an idiomatic Rust reimplementation from the paper rather than a line-by-line translation. Preserve required copyright and license notices if any code is actually adapted.

---

## 3. Scope

### In scope

- bounded scalar losses;
- monotone one-dimensional CRC baseline;
- growing calibration sets;
- high-probability anytime-valid bounds;
- non-monotonic losses;
- multidimensional parameter vectors;
- symmetric optimization algorithms;
- explicit stability evidence;
- selective deployment and abstention;
- MDR, TDR, and SDR;
- generalized or risk-adjusted e-values;
- eBH selection;
- known importance weights;
- estimated-weight diagnostics clearly labeled as asymptotic or empirical;
- deterministic, auditable certificates;
- Rust-native APIs with optional serialization and parallelism.

### Not in scope for the first stable release

- training machine-learning models;
- automatic estimation of density ratios from raw features;
- generic nonlinear optimization frameworks;
- automatic proofs of optimizer stability;
- time-series dependence without a dedicated theorem;
- guarantees under arbitrary concept drift;
- causal claims;
- medical-device or regulatory certification;
- a blanket claim that every module is “distribution-free”;
- Python or WASM bindings before the Rust API stabilizes;
- direct dependencies on `masstrust`, `quietset`, `lineprior`, `veridict`, or `renkin`.

---

## 4. Guarantee taxonomy

Never collapse distinct guarantees into a single boolean such as `is_valid`.

Use an explicit taxonomy similar to:

```rust
pub enum GuaranteeKind {
    /// E[R(theta_hat)] <= alpha under the declared assumptions.
    ExpectedRisk,

    /// With probability at least 1 - delta, risk is <= alpha for every
    /// calibration time covered by the certificate.
    AnytimeHighProbability,

    /// E[L * deploy] <= alpha.
    MarginalDeploymentRisk,

    /// Expected total deployed risk is <= alpha * m.
    TotalDeploymentRisk,

    /// Expected average risk among deployed items is <= alpha.
    SelectiveDeploymentRisk,

    /// Guarantee depends on a limiting argument, such as consistent
    /// estimated importance weights.
    Asymptotic,

    /// No theorem-backed guarantee; diagnostic only.
    EmpiricalOnly,
}
```

A certificate must carry its assumptions:

```rust
pub struct Assumptions {
    pub exchangeability: ExchangeabilityAssumption,
    pub bounded_loss: ClosedInterval,
    pub monotonicity: MonotonicityAssumption,
    pub right_continuity: bool,
    pub symmetry: SymmetryAssumption,
    pub stability: StabilityEvidence,
    pub shift: ShiftAssumption,
}
```

Do not encode an unverified mathematical assumption as `true` merely because the caller selected an option. Distinguish:

- caller-declared assumptions;
- properties checked by the library;
- properties proven by construction;
- properties not checkable from observed data.

---

## 5. Initial repository layout

Start as a single crate. Do not create a large workspace until a real second deliverable exists.

```text
risksieve/
├── Cargo.toml
├── AGENTS.md
├── README.md
├── CHANGELOG.md
├── CITATION.cff
├── LICENSE-MIT
├── LICENSE-APACHE
├── THIRD_PARTY_NOTICES.md
├── docs/
│   ├── guarantees.md
│   ├── assumptions.md
│   ├── references.md
│   └── validation.md
├── src/
│   ├── lib.rs
│   ├── error.rs
│   ├── probability.rs
│   ├── loss.rs
│   ├── guarantee.rs
│   ├── certificate.rs
│   ├── crc/
│   │   ├── mod.rs
│   │   └── monotone.rs
│   ├── anytime/
│   │   ├── mod.rs
│   │   ├── boundary.rs
│   │   ├── calibration.rs
│   │   └── shifted.rs
│   ├── nonmonotone/
│   │   ├── mod.rs
│   │   ├── stability.rs
│   │   ├── discretized.rs
│   │   ├── lipschitz.rs
│   │   └── erm.rs
│   ├── selective/
│   │   ├── mod.rs
│   │   ├── evalue.rs
│   │   ├── mdr.rs
│   │   ├── sdr.rs
│   │   └── ebh.rs
│   ├── shift/
│   │   ├── mod.rs
│   │   └── importance.rs
│   └── numerics/
│       ├── mod.rs
│       ├── summation.rs
│       ├── search.rs
│       └── validation.rs
├── examples/
│   ├── anytime_binary_loss.rs
│   ├── selective_abstention.rs
│   ├── masstrust_style_gate.rs
│   └── renkin_route_selection.rs
└── tests/
    ├── paper_crc.rs
    ├── paper_anytime.rs
    ├── paper_nonmonotone.rs
    ├── paper_score_mdr.rs
    ├── paper_score_sdr.rs
    ├── permutation_invariance.rs
    ├── numerical_edges.rs
    └── statistical_validity.rs
```

If bindings are added later, prefer separate crates such as `risksieve-py` or `risksieve-wasm`. The core crate must not depend on binding frameworks.

---

## 6. Core API principles

### 6.1 Validate probability-like values

Do not pass unchecked `f64` values for alpha, delta, gamma, probabilities, or non-negative weights.

```rust
pub struct OpenUnitInterval(f64);
pub struct ClosedUnitInterval(f64);
pub struct NonNegative(f64);
```

Constructors must reject:

- NaN;
- positive or negative infinity;
- out-of-range values;
- negative zero where its distinction could leak into ordering or serialization.

### 6.2 Losses are bounded by contract and checked at runtime

A suggested abstraction is:

```rust
pub trait BoundedLoss<Observation, Parameter> {
    fn bounds(&self) -> ClosedInterval;

    fn evaluate(
        &self,
        observation: &Observation,
        parameter: &Parameter,
    ) -> Result<f64, RiskSieveError>;
}
```

Every returned loss must be checked against the declared interval. Do not silently clamp an invalid loss. Return a structured error containing the observed value and expected bounds.

Provide simple built-ins:

- `ZeroOneLoss`;
- `AbsoluteErrorLoss` with explicit scaling or cap;
- `SelectiveLoss`;
- `WeightedLoss`;
- `MultigroupLoss`.

Avoid domain-specific losses in the core crate.

### 6.3 Certificates are first-class outputs

Controller methods should return a certificate, not only a threshold.

```rust
pub struct RiskCertificate<Parameter> {
    pub parameter: Parameter,
    pub target_risk: f64,
    pub certified_upper_bound: f64,
    pub guarantee: GuaranteeKind,
    pub assumptions: Assumptions,
    pub calibration_size: usize,
    pub diagnostics: Diagnostics,
}
```

Diagnostics may contain:

- empirical risk;
- correction term;
- effective sample size;
- selected count;
- abstention rate;
- weight range;
- stability beta;
- whether a running minimum was applied;
- whether the returned set is intentionally uninformative.

A diagnostic must never be presented as part of the theorem-backed guarantee unless the cited theorem uses it.

### 6.4 Proven and estimated stability are different types

For non-monotonic CRC, do not allow a bootstrap estimate to masquerade as a proven stability constant.

```rust
pub enum StabilityEvidence {
    Analytic {
        beta: NonNegative,
        reference: String,
    },
    UserSupplied {
        beta: NonNegative,
        justification: String,
    },
    Estimated {
        estimate: NonNegative,
        method: StabilityEstimationMethod,
        confidence_interval: Option<(f64, f64)>,
    },
    Unknown,
}
```

Rules:

- `Analytic` may produce a theorem-backed certificate when all other assumptions hold.
- `UserSupplied` must be labeled as relying on an external claim.
- `Estimated` produces an `EmpiricalOnly` or explicitly experimental result unless a theorem justifies the estimator.
- `Unknown` must not produce a non-monotonic risk certificate.

### 6.5 Known and estimated importance weights are different types

```rust
pub enum ImportanceWeightSource {
    KnownDensityRatio,
    Estimated {
        method: String,
        training_data_separate_from_calibration: bool,
    },
}
```

Known weights may support the finite-sample theorem implemented from the paper. Estimated weights must be labeled according to the actual result being claimed, commonly asymptotic or empirical.

---

## 7. Implementation sequence

Do not implement all theories in one pull request. Each milestone must leave the crate usable and internally coherent.

### Milestone 0 — project skeleton and mathematical vocabulary

Deliver:

- crate skeleton;
- typed alpha/delta/gamma values;
- bounded-loss trait;
- assumptions and guarantee enums;
- certificate type;
- error taxonomy;
- reference documentation;
- dual-license files;
- CI and formatting.

No statistical method should be exposed before its assumptions can be represented.

### Milestone 1 — classical monotone CRC baseline

Implement the foundational fixed-sample expected-risk controller.

Required:

- bounded monotone loss;
- one-dimensional ordered parameter;
- explicit finite-sample correction;
- deterministic tie behavior;
- uninformative-result representation;
- direct tests against hand-computed examples;
- module documentation citing the foundational CRC paper.

This baseline is the control implementation against which later modules are compared.

### Milestone 2 — anytime-valid monotone CRC

Implement:

- the boundary functions used by Theorem 4.1;
- computation of the minimum eligible calibration size;
- incremental empirical-risk state;
- the required non-increasing threshold sequence or running minimum;
- explicit behavior when the correction exceeds the risk target;
- a certificate with target alpha and failure probability delta;
- optional shifted version only after the unshifted method is validated.

The state object must support cumulative updates without silently discarding observations.

Suggested API shape:

```rust
let mut controller = AnytimeController::builder()
    .target_risk(alpha)?
    .failure_probability(delta)?
    .loss_bound(1.0)?
    .build()?;

let certificate = controller.update(observation)?;
```

Do not claim validity under online model retraining. The 2026 anytime-valid paper explicitly leaves online model updates as future work.

### Milestone 3 — non-monotonic CRC

Implement in increasing order of generality:

1. one-dimensional discretized bounded loss;
2. continuous Lipschitz loss with explicit constants;
3. multidimensional parameter support;
4. regularized ERM adapters;
5. optional stability estimation as diagnostics.

The optimizer interface must be permutation-invariant or explicitly state that symmetry is the caller's responsibility.

Do not call a generic optimizer “stable” because repeated runs appear similar. Stability requires evidence tied to the implemented theorem.

### Milestone 4 — SCoRE MDR

Implement:

- risk-adjusted e-value abstraction;
- SCoRE-MDR;
- direct deployment decision;
- explicit gamma parameter;
- efficient computation shortcut only after a transparent reference implementation passes tests;
- MDR and implied TDR reporting.

The score function predicts risk or uncertainty but does not itself need to be calibrated or accurate for the finite-sample theorem. Documentation must nevertheless explain that a poor score can severely reduce selection power.

### Milestone 5 — SCoRE SDR

Implement:

- batch test-item handling;
- SCoRE-SDR e-values;
- eBH;
- zero-selection behavior;
- deterministic ordering and tie handling;
- selected-set certificate;
- realized-risk evaluation helpers that require labels and are clearly post-hoc.

Never replace the denominator `max(1, selected_count)` with a different convention.

### Milestone 6 — distribution shift

Implement shifted methods only after the corresponding exchangeable versions are stable.

Required:

- non-negative finite weight validation;
- diagnostics for sum of weights, sum of squared weights, effective sample size, minimum, and maximum;
- known-versus-estimated weight source;
- tests for constant weights reducing to the unweighted method;
- explicit failure for all-zero or numerically degenerate weights;
- no automatic density-ratio learning in the core crate.

### Milestone 7 — downstream adapters and examples

Add examples rather than direct dependencies:

- `masstrust`: abstain from candidate annotation when selective risk is not certified;
- `quietset`: use stability or confidence outputs as an external score, not as a guaranteed loss;
- `lineprior`: consume out-of-fold risk predictions as SCoRE scores;
- `veridict`: share probability and certificate vocabulary, but do not conflate sequential hypothesis testing with anytime CRC;
- `renkin`: select or abstain from routes using bounded route-quality loss.

Each example must state which data are training, calibration, and test data. Reusing the same samples across these roles without a supporting theorem is forbidden.

---

## 8. Numerical requirements

Statistical correctness includes numerical correctness.

### Mandatory rules

- Reject NaN and infinity at API boundaries.
- Use compensated or pairwise summation for accumulated losses and weights.
- Use stable `log`, `log2`, and square-root calculations.
- Check every square-root argument for small negative roundoff.
- Use deterministic total ordering for floating-point sorting.
- Specify tie-breaking in documentation and tests.
- Avoid subtracting nearly equal large values where an algebraically equivalent stable form exists.
- Never silently saturate an e-value, correction, or weight.
- Detect overflow before multiplication where practical.
- Use `usize` carefully when formulas require conversion to `f64`; document the largest exactly representable sample count.
- Keep reference implementations simple and auditable before optimizing.

### Performance targets

Prefer:

- `O(n log n)` or better for threshold selection;
- incremental `O(1)` state updates where the theorem permits;
- no unnecessary allocation in repeated updates;
- optional parallelism behind a `rayon` feature;
- no global thread pool configuration;
- no `unsafe` in the initial releases.

Performance changes require criterion benchmarks and must preserve exact deterministic outputs unless a documented tolerance is unavoidable.

---

## 9. Testing strategy

### 9.1 Deterministic unit tests

Every public mathematical primitive requires tests for:

- alpha near zero and one;
- delta near zero and one;
- empty calibration data;
- one observation;
- all-zero losses;
- all-maximal losses;
- ties;
- discontinuous losses;
- uninformative prediction sets;
- zero selected items;
- one selected item;
- all selected items;
- constant importance weights;
- extreme but finite importance weights;
- invalid weights;
- invalid loss outputs;
- repeated updates;
- serialization round trips when `serde` is enabled.

### 9.2 Paper-traceable tests

Name tests after the source and theorem or algorithm:

```text
anytime_theorem_4_1_boundary_matches_reference
anytime_corollary_4_2_binary_loss
nonmonotone_theorem_1_with_zero_stability
score_algorithm_1_mdr_matches_reference
score_algorithm_2_sdr_matches_reference
```

Each test must link to:

- paper identifier;
- theorem, proposition, definition, or algorithm number;
- any interpretation required by the implementation.

Do not copy numeric outputs from a notebook without recording the input and method used to obtain them.

### 9.3 Property tests

Use `proptest` for invariants such as:

- permutation invariance of symmetric procedures;
- e-values are non-negative;
- certificates never report a bound below zero or above the declared loss maximum unless the mathematical quantity permits it;
- constant importance weights reduce to the unweighted result;
- adding a calibration observation does not corrupt stored sufficient statistics;
- the anytime threshold sequence follows the monotonicity/running-minimum rule;
- serialization preserves certificate semantics.

Do not invent monotonicity properties for non-monotonic controllers.

### 9.4 Statistical validity tests

Monte Carlo tests are necessary but are not proofs.

Maintain two tiers:

1. fast deterministic CI tests with fixed seeds and modest repetitions;
2. slower ignored or scheduled tests with enough repetitions to detect material undercoverage.

Use binomial or concentration intervals around observed violation rates. Avoid brittle assertions such as `observed <= alpha` in a small simulation.

Record:

- RNG algorithm;
- seed;
- repetitions;
- data-generating process;
- target alpha and delta;
- acceptance criterion;
- software version.

### 9.5 Cross-language oracle tests

- SCoRE's MIT-licensed Python implementation may be used as a test oracle.
- The foundational MIT-licensed CRC repository may be used as a test oracle.
- The non-monotonic repository must not be copied from until its license file is clarified; independently generated fixtures are acceptable.
- Python must not become a runtime dependency of the Rust crate.
- Checked-in oracle fixtures must include provenance and generation scripts.

### 9.6 Regression tests

Every discovered correctness bug requires:

1. a minimal failing test;
2. a fix;
3. a short comment describing which guarantee could have been misstated;
4. a CHANGELOG entry when public behavior changes.

---

## 10. Documentation and citation policy

Yes, the papers must be cited explicitly.

### Required files

- `README.md`: scientific basis and scope;
- `docs/references.md`: complete bibliography and implementation mapping;
- `CITATION.cff`: how to cite `risksieve` itself;
- `THIRD_PARTY_NOTICES.md`: code or fixtures derived from external implementations;
- rustdoc on every theorem-backed public method.

### Citation granularity

At module level, cite the paper.

At implementation level, cite the exact theorem, proposition, definition, algorithm, or equation when the code follows it.

Example:

```rust
/// Computes the anytime correction from Hultberg, Zachariah, and Ribeiro
/// (2026), Theorem 4.1, arXiv:2602.04364.
///
/// This function assumes a loss bounded in `[0, B]`.
pub fn anytime_correction(...) -> Result<f64, RiskSieveError> {
    // ...
}
```

### Wording rules

Use:

- “implements the procedure in …”
- “under the assumptions listed in …”
- “the returned certificate represents …”
- “empirically validated against …”

Avoid:

- “mathematically proven by Rust” unless formal verification exists;
- “safe AI” without qualification;
- “distribution-free” without immediately naming the exchangeability or shift assumptions;
- “works under drift” when only covariate shift with valid weights is covered;
- “exact” when estimated weights or estimated stability are used;
- “guaranteed” for empirical diagnostics.

---

## 11. Licensing and clean-room implementation

### Project license

Use:

```text
MIT OR Apache-2.0
```

Include both full license texts and the SPDX expression in `Cargo.toml`.

### Research papers

Citing a paper and independently implementing its mathematical method is different from copying its prose, figures, notebooks, or source code.

Rules:

- derive Rust code from the mathematical specification;
- write original documentation;
- do not reproduce paper figures without checking the paper license and attribution requirements;
- do not paste long passages from papers;
- preserve citations even when no source code is reused.

### Reference repositories

- SCoRE: MIT; adaptation is allowed subject to the license and attribution requirements.
- Foundational CRC reference repository: MIT.
- Non-monotonic CRC repository: README claims MIT, but the missing root license creates ambiguity. Treat it as all-rights-reserved for code-reuse purposes until clarified.
- Anytime-valid CRC: no official implementation was identified in the paper during the initial review; implement independently from the paper.

Whenever external code is adapted:

- record the original file and commit;
- record the license;
- add the required notice;
- explain the transformation in `THIRD_PARTY_NOTICES.md`;
- do not remove original copyright notices.

When only behavioral outputs are used as test fixtures, record provenance but do not imply that the Rust implementation is a derivative translation.

This section is an engineering policy, not legal advice.

---

## 12. Dependency policy

The core crate should be small and auditable.

Acceptable initial dependencies may include:

- `thiserror`;
- `serde` behind a feature;
- `proptest` as a dev-dependency;
- `criterion` as a dev-dependency;
- `rayon` behind an optional feature.

Before adding a numerical or optimization dependency, document:

- why the standard library is insufficient;
- whether the dependency affects deterministic behavior;
- its MSRV;
- license compatibility;
- maintenance status;
- WASM compatibility if relevant.

Do not add a full machine-learning framework to implement a few scalar formulas.

Commit `Cargo.lock` if the repository contains binaries or reproducibility tooling; otherwise follow the normal library policy and document the decision.

Run dependency and license checks in CI.

---

## 13. Error handling

Use a structured error enum. Do not panic on user data.

Expected variants include:

```rust
pub enum RiskSieveError {
    InvalidProbability { name: &'static str, value: f64 },
    NonFiniteValue { name: &'static str, value: f64 },
    LossOutOfBounds { value: f64, lower: f64, upper: f64 },
    EmptyCalibrationSet,
    InvalidImportanceWeight { index: usize, value: f64 },
    DegenerateWeights,
    MissingStabilityEvidence,
    AssumptionMismatch { detail: String },
    NoFeasibleParameter,
    NumericalFailure { operation: &'static str },
}
```

`NoFeasibleParameter` and an intentionally uninformative prediction set are not always the same thing. Represent the paper-specified uninformative result as a valid outcome when appropriate.

---

## 14. Agent workflow

Before changing code:

1. read this file;
2. read the relevant paper section;
3. read `docs/guarantees.md` and `docs/assumptions.md`;
4. identify the exact guarantee being implemented;
5. identify assumptions that are checkable, caller-declared, or uncheckable;
6. propose the smallest coherent change.

During implementation:

- work on one theoretical slice per pull request;
- avoid unrelated refactors;
- keep a transparent reference implementation until optimized code is cross-checked;
- add tests before or with the implementation;
- update rustdoc and reference mapping;
- preserve deterministic behavior;
- do not weaken validation to make an example pass;
- do not convert theorem conditions into undocumented defaults.

After implementation:

1. run formatting, linting, tests, documentation tests, and dependency checks;
2. compare against independent fixtures where available;
3. state what guarantee is now supported;
4. state what is still unsupported;
5. list every assumption;
6. state whether any external code was adapted;
7. update `CHANGELOG.md`.

---

## 15. Pull-request requirements

Every pull request implementing statistical functionality must include:

- **Source:** paper and exact theorem/algorithm;
- **Guarantee:** precise mathematical quantity controlled;
- **Assumptions:** complete list;
- **API:** example input and returned certificate;
- **Validation:** deterministic tests, property tests, and oracle tests;
- **Numerics:** known edge cases;
- **Complexity:** time and memory complexity;
- **Licensing:** whether external code or fixtures were used;
- **Limitations:** unsupported settings and non-guaranteed modes.

A PR must not be merged if reviewers cannot distinguish the theorem-backed output from diagnostics.

---

## 16. Definition of done

A module is complete only when all are true:

- public API is typed and documented;
- invalid numeric inputs are rejected;
- assumptions are represented in returned metadata;
- exact paper references are present;
- deterministic tests cover edge cases;
- property tests cover structural invariants;
- statistical simulations show no obvious contradiction;
- reference outputs are cross-checked where possible;
- no unsupported guarantee is implied;
- performance is measured for expected workloads;
- license and attribution review is complete;
- examples compile;
- rustdoc contains at least one realistic example;
- `cargo fmt`, `clippy`, tests, and docs are green.

Passing tests alone is not sufficient if the API can misstate the guarantee.

---

## 17. Standard commands

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --doc
cargo bench --no-run
cargo deny check
```

Run slower statistical validation explicitly:

```bash
cargo test --test statistical_validity -- --ignored --nocapture
```

If Miri-compatible tests are present:

```bash
cargo +nightly miri test
```

Do not require nightly Rust for normal library use.

---

## 18. First implementation backlog

Create issues in this order:

1. project skeleton, licenses, CI, references;
2. validated scalar types;
3. bounded-loss abstraction;
4. guarantee and assumption metadata;
5. classical monotone CRC baseline;
6. deterministic threshold search and tie policy;
7. anytime boundary primitives;
8. growing-calibration controller;
9. anytime paper fixtures and simulations;
10. non-monotonic stability evidence types;
11. discretized non-monotonic controller;
12. multidimensional optimizer interface;
13. SCoRE risk-adjusted e-values;
14. SCoRE-MDR;
15. eBH;
16. SCoRE-SDR;
17. importance-weight types and diagnostics;
18. shifted anytime CRC;
19. weighted SCoRE;
20. downstream integration examples.

The first release should prefer a smaller set of correctly labeled guarantees over a broad API with ambiguous claims.
