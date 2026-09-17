# tpt-reficon — Project TODO

Refinement types, FFI contracts, and fractional borrows for Rust, verified by a QF_LRA solver
ported in-house from [tpt-telos](https://github.com/tpt-solutions/tpt-telos).

License: dual MIT OR Apache-2.0. Copyright TPT Solutions.

## Phase 0 — Repository & Governance Setup
- [x] Init git repo (`.gitignore` for Rust/Cargo)
- [x] `Cargo.toml` workspace manifest (resolver = "2"), pick MSRV and Rust edition
- [x] `LICENSE-MIT` and `LICENSE-APACHE` (copyright: TPT Solutions), set `license = "MIT OR Apache-2.0"` convention for all crates
- [x] `README.md` (project pitch, from spec.txt overview + API example)
- [x] `CONTRIBUTING.md`
- [x] `CHANGELOG.md` (Keep a Changelog format)
- [x] `rustfmt.toml` / `.clippy.toml` conventions
- [x] GitHub Actions CI: fmt check, clippy, `cargo test --workspace`, MSRV check
- [x] Issue/PR templates

## Phase 1 — Verifier Core (ported from `tpt-telos-verifier` / `tpt-telos-uir-bridge`)
- [x] Study `tpt-telos-verifier` (Fourier–Motzkin QF_LRA solver) and `tpt-telos-uir-bridge`
      (memory-layout proofs) source at github.com/tpt-solutions/tpt-telos
- [x] Create `crates/tpt-reficon-verifier` crate
- [x] Port/adapt the Fourier–Motzkin elimination decision procedure for QF_LRA constraints
- [x] Design internal constraint IR (replaces spec.txt's "SMT-LIB2 Generator" concept, since the
      ported solver is not SMT-LIB2-based) — document this deviation from spec.txt
- [x] Port memory-layout/alignment proof logic from `tpt-telos-uir-bridge` (needed later for FFI contracts)
- [x] Decide on optional `z3` feature flag parity (nonlinear arithmetic fallback)
- [x] Attribution: note ported-code origin in `CHANGELOG.md`/`NOTICE`
- [x] Unit tests: satisfiable / unsatisfiable linear constraint sets, edge cases

## Phase 2 — Proc-Macro Frontend (`tpt-reficon-macros`)
- [x] Create `crates/tpt-reficon-macros` crate (proc-macro2, syn, quote)
- [x] Parse `#[derive(Refine)]` on tuple/newtype structs
- [x] Parse `#[requires(...)]` / `#[ensures(...)]` attribute predicates
- [x] Extract predicate expressions to the internal constraint IR (Phase 1)
- [x] Zero-cost elision: call verifier at macro-expansion time; elide runtime check if proved, else emit runtime `assert!`/`Result` check
- [x] Diagnostics: span-accurate compile errors for unprovable/malformed predicates
- [x] `trybuild` tests: expansion snapshots + compile-fail cases

### Frontend hardening (review 2026-09)
- [x] Runtime checks on integer primitives evaluate in `i128` — solver-consistent,
      no debug panics or release wrap for narrow types (e.g. `value + 200 < 300` on `u8`)
- [ ] Support `||` (disjunction) in predicates: lower to DNF branches, case-split
      through the solver, branch-wise elision/entailment (e.g. `value == 0 || value == 1`)
- [ ] Integer-domain (ℤ) completeness: catch predicates with no *integral* solution
      (e.g. `2*value == 1` on `u8`) at compile time instead of an always-panicking
      runtime check (today the solver decides over ℚ; see README "Soundness notes")
- [ ] Real `ensures` post-state semantics (post-condition on the constructed value);
      today `ensures` is treated as an additional `requires`

## Phase 3 — Core Runtime Crate (`tpt-reficon-core`)
- [x] Create `crates/tpt-reficon-core` crate
- [x] `Refine` trait + runtime-check error types
- [x] Shared types/helpers used by macro-generated code
- [x] `crates/tpt-reficon` facade crate re-exporting `prelude::*` (macros + core) — matches spec.txt API example

## Phase 4 — Contract-Based FFI Boundaries
- [ ] C header ingestion via `bindgen` AST
- [ ] Investigate Go header/cgo ingestion strategy (Go has no native C headers — evaluate cgo-generated headers vs. a Go-side codegen helper)
- [ ] `#[ffi_contract(bridge = "...", header = "...")]` macro implementation
- [ ] Memory layout/alignment verification: compare `repr(C)` struct vs. parsed foreign layout using ported `tpt-telos-uir-bridge` logic (Phase 1)
- [ ] Generate safe wrapper functions enforcing preconditions before the `unsafe` FFI call
- [ ] Integration tests against sample C and Go libraries

## Phase 5 — Fractional Permission Borrows (Research/Prototype)
- [ ] Literature review: separation logic, prior art (RustBelt, Iris, Prusti, Creusot)
- [ ] Design `#[fractional_borrow]` macro API surface
- [ ] Prototype non-interference proofs via `tpt-reficon-verifier`
- [ ] Prototype lock-free concurrent accessor codegen
- [ ] Ship behind an `unstable`/experimental feature flag

## Phase 6 — Build-Time Daemon & Caching
- [ ] Design build-time daemon process to cache verifier results across compilations
- [ ] IPC mechanism between proc-macro (compiler process) and daemon
- [ ] Cache invalidation strategy (keyed on predicate AST + constraint IR hash)
- [ ] Fallback to in-process solving when daemon is unavailable/disabled

## Phase 7 — Testing & Quality
- [x] Property-based tests (proptest) for solver + macro-generated checks
- [x] Adversarial-model hardening: `satisfies`/eval overflow is conservative and
      never panics (review 2026-09, committed as `a83e72c`)
- [ ] Compile-time overhead benchmarking (macro expansion + verifier latency)
- [ ] Fuzz testing the FFI header parser
- [x] Full workspace clippy/fmt/MSRV gates enforced in CI (ties back to Phase 0 CI), plus example build/run
- [ ] CI supply-chain scanning (`cargo audit` / `cargo deny`): vulnerability,
      license, and dep-version policy gates in CI (adoption/security)

## Phase 8 — Documentation
- [x] `#![warn(missing_docs)]` enforced on every public item across all crates
      (gate landed with review 2026-09, committed as `a83e72c`)
- [ ] User guide (mdBook or docs.rs long-form docs): refinement types, FFI contracts, fractional borrows
- [x] Runnable tour example (`crates/tpt-reficon/examples/refinement_types.rs`) wired
      into CI, referenced from README
- [ ] Expand README with runnable examples from spec.txt section 4
- [ ] Positioning doc: comparison vs. `bindgen`, `cbindgen`, `proptest`

## Phase 9 — Release & Publishing
- [ ] Finalize crate metadata (description, keywords, categories, repository, license) for all crates
- [ ] Publish to crates.io in dependency order: `tpt-reficon-verifier` → `tpt-reficon-core` → `tpt-reficon-macros` → `tpt-reficon`
- [ ] Tag `v0.1.0`, write GitHub release notes
- [ ] Public announcement / ecosystem outreach
