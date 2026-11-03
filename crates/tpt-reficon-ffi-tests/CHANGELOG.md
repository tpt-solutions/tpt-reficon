# Changelog

All notable changes to `tpt-reficon-ffi-tests` are documented in this file.

This crate is not published, so entries are for repository history only.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- End-to-end tests for `ffi_wrappers!`. The C side is a Rust function exported
  under the C symbol name, so no C toolchain is needed. The header is the shared
  `payment.h` sample from the facade crate.
- The `ffi` CI job runs this crate, after installing libclang.
- Crate-level README, keywords, and categories.
