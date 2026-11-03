# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- `#[fractional_borrow]` (feature `unstable`): typed, lock-free shared state for
  a struct of `bool` and integer fields. It emits `<Name>Shared` with one
  permission-guarded cell per field and `try_read_<field>` / `try_write_<field>`
  accessors. The typed layer is `tpt_reficon_core::fractional::Shared<T>`.
- `ffi_wrappers!` (feature `ffi`): declares safe wrappers around C functions.
  Each signature is checked against the header, `#[requires]` and `#[ensures]`
  run through `#[contract]` before the unsafe call, and the call is confined to
  the wrapper. Design in `docs/design/ffi-wrappers.md`. End-to-end tests are in
  the standalone crate `crates/tpt-reficon-ffi-tests`, which the `ffi` CI job runs.
- `#[contract]`: pre- and postconditions on functions with integer parameters
  and results. `#[requires]` is checked at entry, `#[ensures]` on `result` at
  exit, and a violation panics. Predicates the parameter or return-type domains
  prove are compiled out. Unsatisfiable preconditions are compile errors. Design
  in `docs/design/function-contracts.md`.
- `#[ffi_contract](bridge = "c" | "go", header = "...", c_name = "...")]`
  (feature `ffi`, needs libclang): proves a `repr(C)` struct mirrors a C struct
  from a header. The foreign layout is read from clang's values via bindgen's
  layout assertions. Covers name, offset, size, alignment, and total size, with
  `const` size and alignment assertions in the expansion.
- `tpt-reficon-verifier::permission`: solver-backed non-interference check for
  fractional permissions (research prototype for Phase 5).
- Design notes under `docs/design/`: Go header ingestion (Phase 4), fractional
  borrows (Phase 5 literature review and design), and the build-time daemon
  (Phase 6, design only, with a measure-first recommendation).
- Proptest no-panic property over `#[ffi_contract]` input parsing.
- Go `bridge = "go"` integration fixture (`crates/tpt-reficon/tests/go/payment/`):
  a cgo-preamble struct, a checked-in `c-shared` header, and a Go test that
  asserts the same layout.
- `tpt-reficon-core::fractional::SharedU64` (feature `unstable`): lock-free
  fractional-permission accessor for `u64` built on atomics, with no `unsafe`.
- User guide as an mdBook under `docs/user-guide/` (refinement types, predicate
  syntax, how checks are decided, soundness model; FFI contracts and fractional
  borrows marked as partial/prototype).
- README: status table mapping the `spec.txt` section-4 API examples to their
  implementation state (refinement types runnable; FFI contracts and `ensures`
  post-state semantics tracked as not-yet-implemented).
- Positioning doc `docs/positioning.md`: candid comparison with `bindgen`,
  `cbindgen`, and `proptest` — role, guarantees, limitations, and guidance on
  when each tool fits.
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
- 2026-11 soundness review (todo.md Phase 10): repeated variables are merged
  before elimination; `#[contract]` treats `mut` parameters without entry
  premises at exit; a postcondition or precondition no value satisfies is a
  compile error, including integer gaps; `ffi_wrappers!` makes a pointer
  wrapper `unsafe fn`; `ffi_contract` checks each field's type against the
  compile target and tracks the header; `Rat`'s `Ord` no longer panics;
  `parse_decimal` accepts one sign only; the generated constructors are
  `#[track_caller]`.
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
- Solver witness reconstruction rejected satisfiable systems such as
  `x > 0 ∧ x == 1`, reporting "could not decide". A strict bound now counts only
  when it sits exactly on the chosen point. Regression test added.
- Refinement derive: with more than one `#[requires]`, the runtime check joined
  predicates without parentheses. `value < 10` plus `value == 0 || value == 42`
  accepted 42, which violates the first predicate. Each predicate is now
  parenthesized, with a regression test.
- Refinement derive and `#[contract]`: any refinement on an `i64` panicked during
  expansion, because the domain's lower bound negated `i64::MIN`. The bound was
  a tautology, so it is dropped. Regression tests added.

### Changed
- **Breaking (2026-11 review):** `Linear::add`/`sub` are now `checked_add`/
  `checked_sub` and return `Option`. `Linear`'s fields are private: use
  `from_parts`, `terms()`, and `constant()`. `equality_substitute` returns
  `Option`. `prove_within_bounds` and `prove_equivalent` report new
  `LayoutMismatch` variants.
- **Breaking:** `Refine::from_inner_unchecked` is removed, so no public constructor
  skips the predicate. `#[derive(Refine)]` rejects `pub` inner fields and inner
  types that are not bare primitives (aliases and qualified paths are not
  resolved).
- **Breaking:** `u128` values above `i128::MAX` fail the runtime check instead of
  wrapping negative. This refuses some valid values, and never accepts an invalid one.
- **Deviation from `spec.txt`:** the design document describes emitting
  SMT-LIB2 queries to the `tpt-telos` solver. Because the solver was ported
  in-house and does not consume SMT-LIB2, the proc-macro frontend targets the
  internal constraint IR directly (see `tpt-reficon-verifier` docs).
- Dropped the `no-std` crates.io category, since no crate is `no_std`.
- `cargo deny` allow-list gains `BSD-3-Clause` and `ISC` for the optional
  `ffi-bindgen` dependency. CI now runs deny with `--all-features` and has an
  `ffi` job that installs libclang.

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
