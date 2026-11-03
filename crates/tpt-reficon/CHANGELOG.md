# Changelog

All notable changes to `tpt-reficon` are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Facade crate re-exporting `prelude::*`: the `Refine` trait, `RefinementError`,
  `#[derive(Refine)]`, `#[contract]`, `#[ffi_contract]`, and `ffi_wrappers!`.
- `tpt_reficon::core` re-export of the runtime crate, so macro-generated code
  resolves under any dependency rename.
- `#[fractional_borrow]` and its typed `Shared<T>` layer, behind the `unstable`
  feature.
- `ffi` feature, enabling `#[ffi_contract]` (for `bridge = "c"` and
  `bridge = "go"`) and `ffi_wrappers!`. Needs libclang at build time.
- `unstable` feature, forwarding to `tpt-reficon-core` and `tpt-reficon-macros`.
- Runnable example `examples/refinement_types.rs`, built and executed in CI.
- Integration tests for function contracts, FFI contracts, fractional borrows,
  refinement regressions, and the Go `bridge = "go"` fixture.
- `#![warn(missing_docs)]` and `#![forbid(unsafe_code)]`.
- Per-crate README, with the feature matrix and the soundness model.

### Changed
- Dropped the `no-std` crates.io category, because no crate is `no_std`.
- Keywords and categories are now set per crate rather than inherited from the
  workspace. Its README is crate-specific and no longer points at the
  repository root.

### Fixed
- Refinement checks on `i64` no longer panic during expansion. The domain's
  lower bound negated `i64::MIN`, and the bound was a tautology, so it is dropped.
  Regression tests cover this.
- With more than one `#[requires]`, the runtime check now parenthesizes each
  predicate. Previously `value < 10` plus `value == 0 || value == 42` accepted
  `42`.

### Security
- Workspace-wide `cargo deny` gate covers RustSec advisories, wildcard
  dependencies, and an SPDX license allow-list. The facade's dependency graph
  is covered by it.
- Workspace-internal path dependencies pin `version = "0.1.0"`, as required by
  the wildcard-dependency ban.

### Attribution
- The solver and layout logic the facade relies on are ported from TPT
  Solutions' `tpt-telos` components. See the workspace `NOTICE` file.
