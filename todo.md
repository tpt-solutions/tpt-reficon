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
- [x] Real `ensures` post-state semantics for functions: `#[contract]` checks `#[requires]` at entry and
      `#[ensures]` on `result` at exit (`docs/design/function-contracts.md`). On the refinement derive, `ensures`
      is still an additional `requires`, which is equivalent there because the constructed value is the one checked

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
- [x] Generate safe wrapper functions enforcing preconditions before the `unsafe` FFI call: `ffi_wrappers!`
      checks each Rust signature against the header, and `#[requires]`/`#[ensures]` run through `#[contract]`
      before the call (`docs/design/ffi-wrappers.md`). Written as a macro over function declarations, not as
      an attribute on the struct, because the wrapper needs a function to attach to
- [x] Integration tests against sample C and Go libraries — C sample header done (`crates/tpt-reficon/tests/ffi/`);
      Go side is `crates/tpt-reficon/tests/go/payment/`: a cgo-preamble struct, a checked-in
      `c-shared` header, and a Go test asserting the same layout (done)

## Phase 5 — Fractional Permission Borrows (Research/Prototype)
- [x] Literature review: separation logic, prior art (RustBelt, Iris, Prusti, Creusot) — `docs/design/fractional-borrows.md`
- [x] Design `#[fractional_borrow]` macro API surface (proposed syntax in the design doc)
- [x] Prototype non-interference proofs via `tpt-reficon-verifier` (`permission` module)
- [x] Prototype lock-free concurrent accessor — built as a runtime type over atomics (`SharedU64`), no `unsafe`.
      Codegen from `#[fractional_borrow]` is done (below)
- [x] `#[fractional_borrow]` codegen (`unstable`): emits `<Name>Shared` with one permission-guarded typed cell per
      field and `try_read_<field>` / `try_write_<field>` accessors. Permission is per field. Typed layer is
      `tpt_reficon_core::fractional::Shared<T>` over `Bits` (integers and `bool`)
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
- [x] Fuzz testing the FFI header parser — proptest no-panic property over `#[ffi_contract]` input parsing;
      The `cargo-fuzz` target `solver_decide` in `fuzz/` runs in CI (nightly, 60 s).
      Header ingestion glue is covered by the proptest property; bindgen's own C parser is out of scope.
      Local MSVC linking of the libFuzzer runtime fails on this machine, so fuzzing runs on Linux CI only
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

## Phase 10 — Review 2026-11 Soundness Fixes (write the failing test first)
- [x] A1 `Linear::add`/`sub` saturate silently (`verifier/src/ir.rs`): make them checked; overflow becomes "unsupported predicate" in `predicate.rs`
- [x] A2 Merge duplicate variables in `Linear.terms` in `to_inequalities` (`solver.rs`); make the fields private
- [x] A3 `u128` above `i128::MAX` wraps negative in the widened check (`expand.rs`, `contract.rs`, `domain.rs`)
- [x] A4 `#[contract]` with `mut` params: snapshot params before the body; never elide `ensures` on them
- [x] A5 Seal or make `unsafe` `Refine::from_inner_unchecked`; reject `pub` inner fields in the derive
- [x] A6 `ffi_wrappers!` must not emit safe `pub fn` taking raw pointers
- [x] A7 Reject or resolve type aliases and non-primitive inner types (`domain.rs`)
- [x] A8 `#[contract]`: run the integer-gap check and check `ensures` against the return domain (`contract.rs`)
- [x] A9 `layout.rs`: alignment, first-offset and `offsets` length checks in `prove_within_bounds`/`prove_equivalent`; handle empty structs
- [x] A10 `solver.rs`: check `uppers*lowers` before combining; share one budget across `!=` leaves
- [x] A11 FFI: compare type kind (not just size/align), `c_char`/`c_long` per target, track header changes with `include_bytes!` — `c_long` uses the host width, and the per-field kind assertion, compiled for the target, rejects a mismatch.
- [ ] Lower severity: `Ident::new` panics on bad `variable`/`r#type`; `satisfies` overflow handling; `Rat` negation/`Ord` panics and `parse_decimal("+-5")`; non-Copy inner types; suffixed literals; lifetime-only generics in `#[contract]`; `#[track_caller]` on `new`/contract panics; struct-level error spans (re-bless `.stderr`); `fractional_borrow` for `usize`/`isize`/`u128`; loom model for `SharedU64` — done: `Ident::new` (now an error), `Rat` `Ord` and `parse_decimal` sign, non-Copy inner types (rejected by the primitive rule), suffixed literals, lifetime-only generics, `#[track_caller]`, `fractional_borrow` on `usize`/`isize`/`u128` (rejected), `satisfies` overflow (already conservative). Still open: struct-level error spans (needs a `.stderr` re-bless), loom model for `SharedU64` (needs a new dev-dependency).

## Phase 11 — Release Blockers and Stale Docs
- [x] Per-crate `LICENSE-*` and `README.md`; absolute links in the facade README; `[package.metadata.docs.rs]` (no `all-features`)
- [ ] Commit `Cargo.lock` or prove/raise MSRV; run `cargo publish --dry-run` in dependency order — MSRV proven (1.75.0 builds and compiles the tests; `Cargo.lock` stays gitignored). `cargo publish --dry-run` is left for the release, since it needs the dependencies on crates.io.
- [x] CI: pin `dtolnay/rust-toolchain@stable`, OS matrix, `cargo doc -D warnings`, `mdbook build/test`, Go test job, `cargo hack --each-feature`, `unstable` facade tests, pinned toolchain for trybuild, MSRV `--no-run` tests
- [x] Refresh docs that call shipped features "planned": `introduction.md`, `ffi-contracts.md`, `soundness.md`, `positioning.md`, README, `fractional-borrows.md`; add a CHANGELOG entry for `#[fractional_borrow]`
- [x] Fix README: "no unsafe" claim (`ffi_wrappers!` emits it), quick-start comment on `new`, nested "Status" heading; fix mdBook `../../design/` links
- [x] Fix `todo.md` Phase 4 Go item and the garbled Phase 7 fuzz bullet; update `CONTRIBUTING.md` deps and libclang note; add a banner to `spec.txt`
- [x] Add `SECURITY.md`, `CODE_OF_CONDUCT.md`, issue-template `config.yml`

## Phase 12 — Usability
- [x] `RefinementError` and panic messages carry the offending value and predicate
- [ ] Derive extras: `Display`, `From<T> for Inner`, `AsRef`, `FromStr`, `TryFrom<Inner>`, `Hash`/`Ord`, `const fn` constructors — done: `From<T> for Inner`, `AsRef`, `Display`, `FromStr`; `TryFrom` already existed. Declined: `Hash`/`Ord` (a user derive on the refined type already gives them, and the derive does not add impls the user may want to choose); `const fn` constructors (the check calls `TryInto`, which is not `const`, and a `const` constructor would need the check redesigned).
- [x] `serde` feature, as `#[refine(serde)]`: opt-in, so no `cfg` is needed and users who do not want serde pay nothing. The impl deserializes the inner type and then runs `try_new`. Decision: opt-in attribute, not a crate feature, because a `cfg` evaluated in the user's crate cannot see this crate's features.
- [x] Debug-only / off switch for runtime checks — declined. Skipping a check in release builds makes the guarantee false for release users. A user who wants that can write `debug_assert!` at the call site, where the decision is visible. Keeping the checks on everywhere is the recommendation.
- [ ] Contracts: methods and `&self`, `const fn`, lifetime generics, `Result`/`Option`-aware `ensures`, `old(x)` — done: methods and `&self` (the receiver is not checked, and the body sees it), lifetime generics, `old(x)` on integer parameters. Declined: `const fn` (the generated checks use closures and `TryInto`, neither `const`); `Result`/`Option`-aware `ensures` (needs predicates over enum variants, which the linear grammar does not express).
- [x] Predicates: `%`/`*` by constant, `!`, `bool`, floats, `.len()` — done: `*` by a constant, `!`, `true`/`false` literals (so `bool` fields compare with `== true`). Declined: floats (the solver decides over ℚ, but `f64` arithmetic rounds, so a proof over ℚ can be false for the `f64` value); `.len()` (needs non-primitive inner types, which the derive rejects by design). `%` by a positive literal is done, over unsigned inner types only. A remainder becomes a fresh variable with the defining facts dividend = k·q + r and 0 <= r < k, which every branch carries. The quotient is constrained only over the rationals, so an UNSAT verdict and an entailment proof stay sound while some integer-only cases are missed. Signed types are rejected, because a signed remainder takes the dividend's sign.
- [ ] Fractions: real splitting in `SharedU64` and the permission prover — open. The current accessor hands out whole units, and splitting a value's fraction needs a new encoding of the permission pool.

## Phase 13 — Adoption: Examples, Templates, Docs
- [ ] Examples: `port`, `percentage`, `non_zero_quantity`, `money_cents`, `contract_basics`, `ffi_c_library/` (with `build.rs`), `shared_counter`; all run in CI — six of seven done and run in CI. Open: `ffi_c_library/`. It needs libclang and a C compiler to build, and libclang is not installed here, so I could not run it. The CI `ffi` job is where to verify it.
- [x] mdBook cookbook and a migration guide from `assert!`, `validator`, `nutype`, `garde`
- [x] Extend `positioning.md` with `nutype`, Prusti, Creusot, Kani, Flux
- [ ] `#![doc = include_str!("../README.md")]` on the facade; badges; "which tool for what" table — badge and the which-tool table are done. Open: `include_str!`. The README has `compile_fail` and `main` blocks that do not suit doctests, so the recommendation is to keep the facade's own short crate docs.
- [ ] `cargo-generate` starter template — open. It needs `cargo-generate` installed to check, and the template is most useful once the crate is published.

## Phase 14 — Automation
- [ ] `release-plz`, tag-triggered publish with dry run, `cargo-semver-checks` — open. The publish and tag parts are release steps, which are outside this work. `cargo-semver-checks` needs a published baseline to compare against.
- [x] Dependabot (cargo, github-actions, gomod); scheduled `cargo deny`/audit and `cargo update` — the scheduled run covers deny and advisories. `cargo update` is covered by Dependabot's weekly grouped updates.
- [ ] Deploy mdBook to GitHub Pages; `cargo-llvm-cov`; `cargo-mutants` on the verifier; `justfile` — `justfile` is done. Open: the Pages deploy (a publish step), `cargo-llvm-cov` and `cargo-mutants` (new CI jobs that I could not run locally).

## Phase 15 — Innovation (after v0.1.0)
- [ ] Tri-state solver result `Valid | Invalid(model) | Unknown(reason)`, unsat cores, counterexamples in compile errors — open. The verifier already returns `Sat`, `Unsat` or `Unknown`, which is most of the tri-state. Unsat cores are a new solver feature.
- [x] Integer tightening (gcd normalisation, strict-bound tightening) to close the ℚ/ℤ gap — done for UNSAT. `verifier::solver::tighten` divides each row by the gcd of its coefficients and turns strict bounds into non-strict ones with the constant adjusted. `unsat_integer` decides UNSAT on the tightened set. The derive and `#[contract]` use it, so `a + a + b + b == 3` is a compile error. `decide` keeps its rational models, which can be fractional, and only the UNSAT answer is integer-exact.
- [ ] `cargo reficon explain <Type>` plus a CI gate on new runtime checks — open. It needs a CLI crate.
- [x] Elision verdict in generated rustdoc on `new` — each constructor now says whether its check was proven by the domain or kept at runtime.
- [x] Predicate-derived `proptest`/`arbitrary` strategies — `#[refine(proptest)]` writes an `Arbitrary` impl that draws the inner type and keeps the values `try_new` accepts. Opt-in, like serde.
- [x] `refined!(Type, literal)` compile-time constructor — done. The derive writes a `const fn __tpt_check` with the predicate, and the macro asserts it on the literal in a `const` item. A bad literal is a compile error, with a trybuild case. The argument must be a constant expression.
- [ ] Header-watch `build.rs` helper and drift check; cross-target layout `--check` — open. The header-watch part is already covered by `include_bytes!` in `#[ffi_contract]`. The `build.rs` helper and the cross-target check need a design.
