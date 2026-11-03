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

## Local setup

- **Rust:** the workspace builds on the MSRV in `Cargo.toml` (1.75.0) and on
  stable. Run the gates CI runs: `cargo fmt --all -- --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`, and
  `cargo test --workspace`.
- **`ffi` feature:** `#[ffi_contract]` and `ffi_wrappers!` run bindgen while the
  crate compiles, so they need libclang. Install LLVM (Windows), `libclang-dev`
  (Debian/Ubuntu), or the Xcode command-line tools (macOS), and set
  `LIBCLANG_PATH` if the library is not on the default path. Without it, the
  bindgen-backed tests panic at expansion time.
- **Supply chain:** run `cargo deny check` before proposing a new dependency.
  The licence allow-list and the advisory policy are in `deny.toml`.
- **Fuzzing:** the `fuzz/` crate uses libFuzzer, which does not link on MSVC.
  Run it on Linux: `cargo fuzz run solver_decide` from `fuzz/`.
