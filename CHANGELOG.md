# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Initial workspace scaffold: `tpt-reficon-verifier`, `tpt-reficon-core`,
  `tpt-reficon-macros`, and the `tpt-reficon` facade crate.
- `tpt-reficon-verifier`: in-house QF_LRA decision procedure via
  Fourier–Motzkin elimination with exact rational arithmetic, including
  witness reconstruction, plus memory-layout/alignment proof utilities
  adapted from the `tpt-telos-uir-bridge` approach.
- `#[derive(Refine)]` refinement-type frontend with compile-time
  satisfiability checking and zero-cost elision of redundant runtime checks.
- `#[requires(...)]` / `#[ensures(...)]` function contract macros.
- `#[ffi_contract(...)]` macro verifying `repr(C)` struct layouts against
  parsed C headers using the verifier's layout prover.
- Experimental `#[fractional_borrow]` prototype behind the `unstable` feature.
- Build-time verifier caching with optional daemon process and automatic
  in-process fallback.

### Changed
- **Deviation from `spec.txt`:** the design document describes emitting
  SMT-LIB2 queries to the `tpt-telos` solver. Because the solver was ported
  in-house and does not consume SMT-LIB2, the proc-macro frontend targets the
  internal constraint IR directly (see `tpt-reficon-verifier` docs).

### Attribution
- The Fourier–Motzkin decision procedure and the memory-layout proof logic are
  ported/adapted from TPT Solutions' internal `tpt-telos-verifier` and
  `tpt-telos-uir-bridge` components. See `NOTICE` for details.
