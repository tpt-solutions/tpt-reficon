# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- README: status table mapping the `spec.txt` section-4 API examples to their
  implementation state (refinement types runnable; FFI contracts and `ensures`
  post-state semantics tracked as not-yet-implemented).
- Initial workspace scaffold with four crates: `tpt-reficon-verifier`,
  `tpt-reficon-core`, `tpt-reficon-macros`, and the `tpt-reficon` facade.
- `tpt-reficon-verifier`: in-house QF_LRA decision procedure via
  Fourier–Motzkin elimination with exact checked rational arithmetic,
  including witness reconstruction and case-splitting over `!=` constraints.
  Overflow, exponential blow-up, and unsupported shapes degrade to a
  conservative `Unknown` verdict — never a spurious UNSAT.
- `tpt-reficon-verifier`: memory-layout/alignment proof utilities (C `repr(C)`
  struct walk, `prove_equivalent`, solver-backed `prove_within_bounds`)
  adapted from the `tpt-telos-uir-bridge` approach.
- `tpt-reficon-macros`: `#[derive(Refine)]` frontend parsing
  `#[requires(...)]` / `#[ensures(...)]` predicates into the internal
  constraint IR, rejecting unsatisfiable refinements with span-accurate
  compile errors, and *eliding* the runtime check when the type's domain
  implies the predicate. Otherwise it emits a precise runtime check
  (`new` panics, `try_new`/`TryFrom` error) with a `'static` message that
  never requires the inner type to implement `Debug`.
- `tpt-reficon-macros`: disjunction (`||`) support in predicates, lowered to
  disjunctive normal form. A disjunction is a compile error (unsatisfiable)
  only when every alternative is, and it is elided when the type's domain
  proves some alternative (e.g. `value <= 3 || value <= 255` on `u8`). A
  pathological disjunction/conjunction expansion is rejected with a
  span-accurate error rather than expanding without bound.
- `tpt-reficon-macros`: exact single-variable integer-feasibility analysis for
  primitive integer inner types. Refinements that are rational-satisfiable but
  have no integer value in the type's domain — `value > 5 && value < 6`, or
  `value > 255` / `value < 0` on `u8` — are now compile errors rather than
  always-panicking constructors. The analysis is conservative (an extra
  variable, or arithmetic overflow, keeps the runtime check) and is verified
  against a brute-force witness oracle by property tests.
- `tpt-reficon-verifier`: dependency-free `cargo bench` micro-benchmarks for the
  decision procedure (`crates/tpt-reficon-verifier/benches/solver.rs`) — solver
  latency on representative shapes (bounds, `!=` splitting and its budget,
  equality chains, multi-variable Fourier–Motzkin, unsat/degenerate systems).
- `tpt-reficon-core`: `Refine` trait and the `RefinementError` runtime-check
  error type (no allocation, usable in `no_std`-adjacent contexts).
- `tpt-reficon`: facade crate re-exporting `prelude::*` (trait + derive macro
  + error type) matching the spec.txt API sketch.
- `trybuild` UI tests covering elided and runtime-checked refinements and
  compile-fail diagnostics (unsatisfiable, unknown variable, nonlinear,
  multi-field, generic).
- Property-based tests: `solver_soundness` (a brute-force rational-grid oracle
  asserts `unsat` never reports a satisfiable system UNSAT, returned models are
  verified witnesses, and entails-respects-countermodels) and macro
  `runtime_checks` (generated `try_new` verdicts vs. plain-Rust predicate
  semantics).
- Runnable example crate (`crates/tpt-reficon/examples/refinement_types.rs`),
  built and executed in CI.
- `#![warn(missing_docs)]` across all crates.
- GitHub Actions CI: fmt check, clippy `-D warnings`, `cargo test --workspace`,
  examples run, and an MSRV (1.75.0) build job.

### Fixed
- `tpt-reficon-verifier.ir.satisfies`: evaluation overflow now degrades
  conservatively to "satisfied" instead of panicking. An adversarial model
  (e.g. an `i128::MAX` value) could previously panic the constraint check used
  to *verify* solver witnesses; it can never turn an overflow into a spurious
  UNSAT.
- `tpt-reficon-macros`: runtime predicates on primitive integer inner types are
  now evaluated in `i128` rather than in the (narrow) inner type. A predicate
  like `value + 200 < 300` on `u8` previously panicked in debug builds and
  silently wrapped (changing its verdict) in release builds; it now agrees with
  the solver's ℚ reasoning and can never overflow.
- `tpt-reficon-verifier.ir.render`: a constant-only linear expression rendered
  with a leading separator (`" - 3"`, `" + 3"`); it now renders `"-3"`.
- `tpt-reficon-verifier.solver.is_contradiction`: the `Ordering::Greater` arm
  returned `false` (satisfiable) — misclassifying the genuine contradiction
  `0 <= -1` as satisfiable. This made the solver report UNSAT as SAT for
  disjoint bounds (e.g. `x >= 2 && x <= 1`).
- `tpt-reficon-verifier.solver.to_inequalities`: flipped rows (`Ge`/`Gt`/`Eq`)
  now negate the right-hand side as well as the coefficients
  (`-Σaᵢxᵢ <= k`, not `<= -k`), and unflipped rows keep their coefficients.
- `tpt-reficon-verifier.solver.bound_expr`: witness reconstruction used the
  wrong reciprocal (`a_v/1` instead of `1/a_v`) and skipped the sign flip on
  non-variable coefficients, producing invalid witnesses for
  multi-variable systems and false `Unknown` results.
- `tpt-reficon-verifier.solver.reconstruct_witness`: seeding the lower/upper
  folds with `ZERO` corrupted reconstruction when every explicit bound lay on
  one side of zero.
- `tpt-reficon-verifier.solver.fm_core`: combined rows no longer keep
  zero-valued coefficient entries, which previously skewed elimination
  heuristics and reconstruction.
- `tpt-reficon-macros`: corrected the tuple-struct field access
  (`self.0`, not a recursive `Self(...)` constructor), removed
  `core::format!` (not available in `core`) in favor of static
  `&'static str` messages, and dropped runtime format captures named after
  macro-internal variables.

### Changed
- **Deviation from `spec.txt`:** the design document describes emitting
  SMT-LIB2 queries to the `tpt-telos` solver. Because the solver was ported
  in-house and does not consume SMT-LIB2, the proc-macro frontend targets the
  internal constraint IR directly (see `tpt-reficon-verifier` docs).

### Security
- Supply-chain scanning gate: `deny.toml` plus a `cargo-deny` CI job enforce
  RustSec advisory checks, a wildcard-dep ban, and an explicit SPDX license
  allow-list over the full dependency graph (default-deny v2 semantics). Run
  locally with `cargo deny check`; adding a dependency takes its license
  through an explicit review.
- Workspace-internal path dependencies now pin `version = "0.1.0"` (publish
  readiness and required to satisfy the wildcard ban).

### Attribution
- The Fourier–Motzkin decision procedure, exact integer-ratio arithmetic, and
  the memory-layout proof logic are ported/adapted from TPT Solutions'
  internal `tpt-telos-verifier` and `tpt-telos-uir-bridge` components.
  See `NOTICE` for details.
