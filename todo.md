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
- [x] Support `||` (disjunction) in predicates: lowered to DNF, case-split for
      satisfiability (a disjunction is UNSAT only when every alternative is) and
      elided when the domain proves an alternative (e.g. `value == 0 || value == 1`,
      or `value <= 3 || value <= 255` on `u8`); branch cap guards blow-up
- [x] Integer-domain (ℤ) completeness: exact single-variable integer-feasibility
      per alternative, intersected with the type domain — catches `value > 5 && value < 6`
      and `value > 255` on `u8` at compile time (conservative; brute-force-oracle proptest)
- [ ] Real `ensures` post-state semantics (post-condition on the constructed value);
      today `ensures` is treated as an additional `requires`. Decision: deferred until
      function contracts exist; on a refinement type the constructed value is the same
      as the `requires` check, so the current behaviour is equivalent there

## Phase 3 — Core Runtime Crate (`tpt-reficon-core`)
- [x] Create `crates/tpt-reficon-core` crate
- [x] `Refine` trait + runtime-check error types
- [x] Shared types/helpers used by macro-generated code
- [x] `crates/tpt-reficon` facade crate re-exporting `prelude::*` (macros + core) — matches spec.txt API example

## Phase 4 — Contract-Based FFI Boundaries
- [x] C header ingestion via `bindgen` (feature `ffi-bindgen`). Deviation: the foreign layout is read from
      bindgen's layout-test assertions (clang's own `sizeof`/`alignof`/`offsetof`), not by walking the AST
- [x] Investigate Go header/cgo ingestion strategy — decided: C header is the single layout source; no Go-side
      codegen. See `docs/design/go-header-ingestion.md`
- [x] `#[ffi_contract(bridge = "...", header = "...")]` macro implementation (struct-level, `ffi` feature)
- [x] Memory layout/alignment verification: `repr(C)` struct vs. foreign layout via `prove_equivalent` and
      solver-backed `prove_within_bounds`, plus `const` size/align assertions
- [ ] Generate safe wrapper functions enforcing preconditions before the `unsafe` FFI call
- [ ] Integration tests against sample C and Go libraries — C sample header done (`crates/tpt-reficon/tests/ffi/`);
      Go side is `crates/tpt-reficon/tests/go/payment/`: a cgo-preamble struct, a checked-in
      `c-shared` header, and a Go test asserting the same layout (done)

## Phase 5 — Fractional Permission Borrows (Research/Prototype)
- [x] Literature review: separation logic, prior art (RustBelt, Iris, Prusti, Creusot) — `docs/design/fractional-borrows.md`
- [x] Design `#[fractional_borrow]` macro API surface (proposed syntax in the design doc)
- [x] Prototype non-interference proofs via `tpt-reficon-verifier` (`permission` module)
- [x] Prototype lock-free concurrent accessor — built as a runtime type over atomics (`SharedU64`), no `unsafe`.
      Codegen from `#[fractional_borrow]` is still to do
- [x] Ship behind an `unstable`/experimental feature flag: `tpt-reficon-core::fractional` (`SharedU64` accessor)
      is gated on `unstable`, which the facade forwards

## Phase 6 — Build-Time Daemon & Caching
- [x] Design build-time daemon process to cache verifier results across compilations — design only;
      recommendation is to measure expansion cost before building (`docs/design/build-daemon.md`)
- [x] IPC mechanism between proc-macro (compiler process) and daemon (design)
- [x] Cache invalidation strategy (keyed on predicate AST + constraint IR hash; design)
- [x] Fallback to in-process solving: not needed, because no daemon is built. Measured solver cost is 0.05–16 µs
      per predicate (`docs/design/build-daemon.md`)

## Phase 7 — Testing & Quality
- [x] Property-based tests (proptest) for solver + macro-generated checks
- [x] Adversarial-model hardening: `satisfies`/eval overflow is conservative and
      never panics (review 2026-09, committed as `a83e72c`)
- [x] Compile-time overhead benchmarking (verifier latency micro-benchmarks in
      `crates/tpt-reficon-verifier/benches/solver.rs`; macro-expansion overhead
      deferred to a `cargo build --timings`-style harness — noted in the bench doc)
- [ ] Fuzz testing the FFI header parser — partial: proptest no-panic property over `#[ffi_contract]` input parsing;
      a `cargo-fuzz` target is still to do
- [x] Full workspace clippy/fmt/MSRV gates enforced in CI (ties back to Phase 0 CI), plus example build/run
- [x] CI supply-chain scanning (`deny.toml` + `cargo-deny` job): RustSec
      advisories, wildcard ban, and an explicit SPDX license allow-list over
      the whole dependency graph (v2 default-deny semantics); local
      `cargo deny check` documented in CONTRIBUTING.md

## Phase 8 — Documentation
- [x] `#![warn(missing_docs)]` enforced on every public item across all crates
      (gate landed with review 2026-09, committed as `a83e72c`)
- [x] User guide (mdBook, `docs/user-guide/`): refinement types, soundness model; FFI contracts and
      fractional borrows chapters marked as planned until Phases 4–5 land
- [x] Runnable tour example (`crates/tpt-reficon/examples/refinement_types.rs`) wired
      into CI, referenced from README
- [x] Expand README with the API examples from spec.txt section 4: status table
      mapping each example to implementation state — the refinement-type example
      is runnable (quick start + tour), the FFI-contract and `ensures`
      post-state examples are explicitly tracked as not-yet-implemented
- [x] Positioning doc (`docs/positioning.md`): honest comparison vs. `bindgen`,
      `cbindgen`, and `proptest` — role, guarantees, and "when to reach for which"

## Phase 9 — Release & Publishing
- [x] Finalize crate metadata (description, keywords, categories, repository, license, documentation) for all crates
      (dropped the inaccurate `no-std` category: no crate is `no_std`)
- [ ] Publish to crates.io in dependency order: `tpt-reficon-verifier` → `tpt-reficon-core` → `tpt-reficon-macros` → `tpt-reficon`
- [ ] Tag `v0.1.0`, write GitHub release notes
- [ ] Public announcement / ecosystem outreach
