# Contributing to tpt-reficon

Thank you for your interest in contributing! This project verifies Rust programs
with an in-house QF_LRA solver, so correctness of the verifier itself is the
highest-priority invariant of this repository.

## Development setup

```sh
git clone https://github.com/tpt-solutions/tpt-reficon
cd tpt-reficon
cargo build --workspace --all-features
cargo test --workspace --all-features
```

Minimum supported Rust version (MSRV) is **1.75.0**. CI enforces it; please do
not introduce dependencies or language features requiring a newer toolchain
without updating the MSRV policy deliberately.

## Project layout

| Crate | Purpose |
| --- | --- |
| `tpt-reficon-verifier` | QF_LRA constraint IR, Fourier–Motzkin decision procedure, layout prover, caching |
| `tpt-reficon-macros` | Proc-macro frontend (`derive(Refine)`, `#[requires]`, `#[ensures]`, `#[ffi_contract]`, `#[fractional_borrow]`) |
| `tpt-reficon-core` | Runtime support types used by generated code |
| `tpt-reficon` | Facade crate re-exporting everything via `prelude::*` |

## Ground rules

1. **Solver soundness first.** Any change to the Fourier–Motzkin elimination,
   rational arithmetic, or witness reconstruction must keep every existing
   property test passing. New optimizations require new tests, including an
   unsatisfiable case that would break if the optimization were wrong.
2. **No runtime panics from user input.** The proc-macro frontend must return
   span-accurate `syn::Error`s for malformed predicates, never panic.
3. **Zero-cost means proven.** A runtime check may only be elided when the
   verifier has *proved* the predicate implied by the type domain. When in
   doubt, emit the check.
4. **Keep dependencies minimal.** The workspace currently depends only on
   `proc-macro2`, `quote`, and `syn`. Justify any addition in your PR.
5. **Format and lint:** run `cargo fmt --all` and `cargo clippy --workspace
   --all-targets -- -D warnings` before pushing.

## Commit / PR conventions

- Conventional Commits prefixes are appreciated (`feat:`, `fix:`, `docs:`,
  `perf:`, `test:`, `chore:`).
- Every PR should state which todo.md phase item(s) it advances.
- Update `CHANGELOG.md` under **Unreleased** for user-visible changes.

## Reporting bugs

Open a GitHub issue using the bug report template. For verifier soundness
bugs, include the smallest constraint set (or predicate) that reproduces the
wrong verdict — these are treated as high priority.

## Licensing

By contributing you agree your work is dual-licensed under MIT OR Apache-2.0.
