# Contributing to tpt-reficon

Thank you for your interest in tpt-reficon! This project verifies Rust programs
with an in-house QF_LRA solver, so correctness of the verifier itself is the
highest-priority invariant of this repository.

**Contributions are accepted as GitHub issues only.** Please do not open pull
requests; they will be closed without review. Open an issue to report a bug,
propose a feature, or suggest a change, and the maintainers will take it from
there.

## Reporting bugs

Open a GitHub issue using the bug report template. For verifier soundness
bugs, include the smallest constraint set (or predicate) that reproduces the
wrong verdict; these are treated as high priority.

## Feature requests and suggestions

Open an issue describing the problem you are trying to solve, the proposed
behaviour, and any alternatives you considered. Changes that touch the solver
should explain how they preserve soundness.

## Licensing

By opening an issue you agree that your report may be used, modified, and
incorporated into the project under its MIT OR Apache-2.0 license.
