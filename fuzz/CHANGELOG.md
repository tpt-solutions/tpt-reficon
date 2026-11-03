# Changelog

All notable changes to `tpt-reficon-fuzz` are documented in this file.

This crate is not published, so entries are for repository history only.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- `solver_decide` fuzz target. It feeds arbitrary bytes to
  `tpt-reficon-verifier::decide` as linear constraints over `x`, `y`, and `z`.
  The property is that `decide` never panics and that its verdicts are
  internally consistent.
- Crate-level README, keywords, and categories.
