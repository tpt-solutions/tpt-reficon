# Changelog

All notable changes to `tpt-reficon-core` are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- `Refine` trait: an associated `Inner` type, `from_inner_unchecked`, and
  `into_inner`. Implemented by `#[derive(Refine)]`.
- `RefinementError`: the runtime-check error. It holds a `&'static str`, so it
  never allocates, and it never requires the inner type to implement `Debug`.
  Constructors `new` and `with_message`, plus `message()`.
- `prelude` module re-exporting `Refine` and `RefinementError`.
- `fractional` module (feature `unstable`): `SharedU64`, a lock-free
  fractional-permission accessor for `u64`, built on atomics. It uses no
  `unsafe`. Acquisition is a compare-and-swap loop and never blocks.
- `verifier` feature, which pulls in `tpt-reficon-verifier` for macro-debugging
  helpers.
- Unit tests for unchecked construction and error message round-trips.

### Changed
- Keywords and categories are now set per crate, and the crate has its own
  README.

### Security
- `#![forbid(unsafe_code)]`. The workspace-wide `unsafe_code = "forbid"` lint
  also applies.
