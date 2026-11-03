# Changelog

All notable changes to `tpt-reficon-macros` are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- `#[derive(Refine)]` on single-field structs, with `#[requires(...)]`,
  `#[ensures(...)]` (treated as an extra conjoined predicate), and
  `#[refine(variable = "...")]`. It generates `new`, `try_new`, `TryFrom`,
  `get`, and `into_inner`.
- Predicates are parsed into the `tpt-reficon-verifier` constraint IR. An
  unsatisfiable refinement is a compile error with a span-accurate message.
- Zero-cost elision: when the type's domain already implies the predicate, no
  runtime check is emitted.
- Disjunction (`||`) support, lowered to disjunctive normal form. A
  disjunction is a compile error only when every alternative is unsatisfiable,
  and it is elided when the domain proves one alternative. An expansion that
  grows without bound is rejected with an error.
- Exact single-variable integer-feasibility analysis for primitive integer
  inner types. Rational-only refinements such as `value > 5 && value < 6`, or
  `value > 255` on `u8`, are compile errors. The analysis is conservative, and
  it is checked against a brute-force oracle by property tests.
- `#[contract]`: pre- and postconditions on functions with integer parameters
  and results. A violation panics. Predicates the parameter or return-type
  domains prove are removed. Unsatisfiable preconditions are compile errors.
- `#[ffi_contract(bridge = "c" | "go", header = "...", c_name = "...")]`
  (feature `ffi-bindgen`): proves a `#[repr(C)]` struct mirrors a C struct. The
  foreign layout comes from clang through bindgen's layout assertions. Covers
  name, offset, size, alignment, and total size, and emits `const` assertions.
- `ffi_wrappers!` (feature `ffi-bindgen`): safe wrappers around C functions.
  Each signature is checked against the header, and `#[requires]` and
  `#[ensures]` run before the unsafe call.
- `#[fractional_borrow]` (feature `unstable`, prototype): emits `<Name>Shared`
  with one permission-guarded cell per field and `try_read_<field>` /
  `try_write_<field>` accessors.
- Proptest no-panic property over `#[ffi_contract]` input parsing.
- Property tests `runtime_checks`, comparing generated `try_new` verdicts with
  plain-Rust predicate semantics, and `trybuild` UI tests covering passing and
  failing cases (unsatisfiable, unknown variable, nonlinear, multi-field,
  generic, out-of-domain, disjunctive, contract).

### Changed
- Keywords and categories are now set per crate, and the crate has its own
  README.
- Targets the internal constraint IR directly rather than emitting SMT-LIB2.
  The solver was ported in-house and does not read SMT-LIB2. See the
  `tpt-reficon-verifier` docs.

### Fixed
- The tuple-struct field access is `self.0`, not a recursive `Self(...)`
  constructor.
- Removed `core::format!`, which `core` does not provide, in favor of static
  `&'static str` messages. Dropped runtime format captures named after
  macro-internal variables.
- Runtime predicates on primitive integer inner types are evaluated in `i128`,
  not the narrow inner type. Predicates such as `value + 200 < 300` on `u8`
  previously panicked in debug builds and wrapped in release builds.
- With more than one `#[requires]`, each predicate is parenthesized in the
  runtime check. Previously the predicates were joined without parentheses.
- Any refinement on an `i64` panicked during expansion, because the domain's
  lower bound negated `i64::MIN`. The bound is a tautology and is now dropped.

### Security
- `#![warn(missing_docs)]` and `unsafe_code` forbidden. Generated code contains
  no `unsafe`, except the `ffi_wrappers!` expansion, which requires the calling
  crate to allow it.
- All arithmetic in the macro expansion uses checked operations.
- Macro expansion is bounded: an `i64` coefficient range and a hard elimination
  limit prevent a single predicate from stalling the compiler.
