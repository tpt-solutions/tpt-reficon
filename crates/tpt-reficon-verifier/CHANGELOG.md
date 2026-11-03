# Changelog

All notable changes to `tpt-reficon-verifier` are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- In-house QF_LRA decision procedure via Fourier–Motzkin elimination, with exact
  checked rational arithmetic. It returns witnesses, and it case-splits over
  `!=` constraints. Overflow, exponential blow-up, and unsupported shapes return
  `Verdict::Unknown`, never a spurious UNSAT.
- Public API: `decide`, `unsat`, `unsat_checked`, `model`, `entails`,
  `counterexample`, `satisfies_model`, and the `Verdict` enum (`Unsat`, `Sat`,
  `Unknown`).
- `ir` module: `Linear`, `Constraint`, `Relation`, `Model`, with `satisfies`,
  `negate`, and `equality_substitute`.
- `layout` module: `StructLayout::compute` (the `repr(C)` algorithm),
  `prove_equivalent` (deterministic match against a foreign layout), and
  `prove_within_bounds` (solver-backed check that fields fit and do not overlap).
- `permission` module (research prototype): `check_configuration`, a
  non-interference check for fractional permissions.
- Dependency-free `cargo bench` micro-benchmarks for the decision procedure:
  bounds, `!=` splitting and its budget, equality chains, multi-variable
  Fourier–Motzkin, and unsatisfiable or degenerate systems.
- `z3-reserved` feature, reserved as an extension point. It is not implemented.
- Property tests: `solver_soundness`, which checks against a brute-force
  rational-grid oracle. It asserts that `unsat` never reports a satisfiable
  system as UNSAT, that returned models are verified witnesses, and that
  `entails` respects countermodels.
- Initial workspace scaffold, adapted from the `tpt-telos` verifier.
- Keywords and categories set per crate, and the crate has its own README.

### Changed
- Keywords and categories are now set per crate, not inherited from the workspace.

### Fixed
- `ir.satisfies`: evaluation overflow now degrades to "satisfied" instead of
  panicking. An adversarial model such as one containing `i128::MAX` could
  previously panic the check that verifies solver witnesses. It cannot turn an
  overflow into a spurious UNSAT.
- `ir.render`: a constant-only linear expression rendered with a leading
  separator (`" - 3"`). It now renders `"-3"`.
- `solver.is_contradiction`: the `Ordering::Greater` arm returned `false`,
  classifying `0 <= -1` as satisfiable. That made the solver report UNSAT as SAT
  for disjoint bounds such as `x >= 2 && x <= 1`.
- `solver.to_inequalities`: flipped rows (`Ge`, `Gt`, `Eq`) now negate the
  right-hand side as well as the coefficients. Unflipped rows keep theirs.
- `solver.bound_expr`: witness reconstruction used the wrong reciprocal
  (`a_v/1` instead of `1/a_v`) and skipped the sign flip on non-variable
  coefficients. That produced invalid witnesses for multi-variable systems and
  false `Unknown` results.
- `solver.reconstruct_witness`: seeding the lower and upper folds with `ZERO`
  corrupted reconstruction when every explicit bound lay on one side of zero.
- `solver.fm_core`: combined rows no longer keep zero-valued coefficients,
  which skewed elimination heuristics and reconstruction.
- Witness reconstruction rejected satisfiable systems such as `x > 0 ∧ x == 1`,
  reporting "could not decide". A strict bound now counts only when it sits
  exactly on the chosen point. Regression test added.

### Security
- Fuzz target `solver_decide` (in the `fuzz/` crate) feeds arbitrary input to
  `decide`. The property is that it never panics and that its verdicts are
  internally consistent.
- Every arithmetic step in the solver is checked. Overflow, blow-up, and
  unsupported shapes degrade to `Unknown`.
- Fourier–Motzkin blow-up is capped, `!=` case-splitting is budgeted, and
  adversarial models cannot force a panic or a spurious UNSAT.

### Attribution
- The Fourier–Motzkin decision procedure, exact integer-ratio arithmetic, and
  memory-layout proof logic are ported and adapted from TPT Solutions'
  `tpt-telos-verifier` and `tpt-telos-uir-bridge`. See the workspace `NOTICE` file.
