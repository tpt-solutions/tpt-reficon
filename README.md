<div align="center">

# tpt-reficon

**Refinement types, FFI contracts, and fractional borrows for Rust — verified
at compile time by an in-house QF_LRA solver.**

[![CI](https://github.com/tpt-solutions/tpt-reficon/actions/workflows/ci.yml/badge.svg)](https://github.com/tpt-solutions/tpt-reficon/actions/workflows/ci.yml)

Dual-licensed under MIT OR Apache-2.0.
Copyright © 2026 TPT Solutions.

</div>

---

`tpt-reficon` shifts runtime validation and manual FFI auditing into
compile-time guarantees. Predicates attached to your types are discharged by a
self-contained Fourier–Motzkin elimination engine (ported from TPT Solutions'
[tpt-telos](https://github.com/tpt-solutions/tpt-telos) verifier) at macro
expansion time. Checks that the solver *proves* redundant are elided — zero
runtime cost. Everything else gets a precise runtime check.

## Highlights

- **Refinement types** — `#[derive(Refine)]` + `#[requires(...)]`: contradictory
  refinements are compile errors; refinements implied by the type's domain run
  at zero cost; all others get an exact runtime check with great diagnostics.

## Status

- **Contract-based FFI (partial, `ffi` feature)** — `#[ffi_contract(bridge = "c",
  header = "...")]` reads a `repr(C)` struct's layout from a C header (through
  bindgen, using clang's own layout values) and proves your struct mirrors it.
  `ffi_wrappers!` declares safe wrappers around C functions: each signature is
  checked against the header, and preconditions run before the unsafe call.
- **Fractional borrows (prototype, `unstable` feature)** — `#[fractional_borrow]`
  turns a struct of integers and `bool` into lock-free shared state with one
  fractional permission per field. The solver-backed non-interference check lives
  in `tpt-reficon-verifier`.
  Design and literature review: [`docs/design/fractional-borrows.md`](docs/design/fractional-borrows.md).

See [`todo.md`](todo.md) for the full plan.

## Quick start

```toml
[dependencies]
tpt-reficon = "0.1"
```

```rust
use tpt_reficon::prelude::*;

// Refinement type: values are mathematically bounded at compile time.
#[derive(Refine)]
#[requires(value > 0 && value <= 100)]
pub struct BoundedPercentage(u8);

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Runtime-checked constructor (the predicate cannot hold for every u8).
    let pct = BoundedPercentage::try_new(42)?;
    assert_eq!(*pct, 42);

    // `new` panics on violation. The check is compiled out only when the
    // type's domain already implies the predicate, which `u8` does not here.
    let full = BoundedPercentage::new(100);
    assert_eq!(full.get(), 100);

    Ok(())
}
```

A longer walkthrough lives in
[`crates/tpt-reficon/examples/refinement_types.rs`](crates/tpt-reficon/examples/refinement_types.rs) —
run it with `cargo run --example refinement_types`.

A refinement that no value can satisfy fails **at compile time**:

```rust,compile_fail
#[derive(tpt_reficon::Refine)]
#[requires(value > 5 && value < 3)] // error: unsatisfiable refinement
struct Impossible(u8);
```

And a check the type already guarantees is compiled out entirely:

```rust
#[derive(tpt_reficon::Refine)]
#[requires(value >= 0)] // always true for u64 — zero code emitted
struct NonNegativeU64(u64);
```

## Where the design-document examples stand

[`spec.txt`](spec.txt) section 4 sketches three API examples. Their current
state:

| `spec.txt` §4 example | Status |
| --- | --- |
| `#[derive(Refine)]` + `#[requires(value > 0 && value <= 100)]` on `struct BoundedPercentage(u8)` | **Implemented** — see the quick start above and the runnable tour in [`examples/refinement_types.rs`](crates/tpt-reficon/examples/refinement_types.rs). |
| `#[ffi_contract(bridge = "go", header = "payment_processor.h")]` on a foreign-facing struct | **Partial** — struct layout proofs work with the `ffi` feature for `bridge = "c"` and `bridge = "go"` (the Go struct must be declared in the cgo preamble; see [`docs/design/go-header-ingestion.md`](docs/design/go-header-ingestion.md)). Safe wrappers are available through `ffi_wrappers!`. |
| `#[ensures(result.is_ok() -> …)]` post-conditions on functions | **Implemented for integer functions** via `#[contract]`: `#[requires]` checks at entry, `#[ensures]` checks `result` at exit, and a violation panics. Predicates the domains prove are compiled out. The `spec.txt` form with `result.is_ok()` is not supported. See [`docs/design/function-contracts.md`](docs/design/function-contracts.md). |

## Crates

| Crate | Purpose |
| --- | --- |
| [`tpt-reficon`](crates/tpt-reficon) | Facade + prelude (start here) |
| [`tpt-reficon-macros`](crates/tpt-reficon-macros) | Proc-macro frontend |
| [`tpt-reficon-core`](crates/tpt-reficon-core) | Runtime support types |
| [`tpt-reficon-verifier`](crates/tpt-reficon-verifier) | QF_LRA solver + layout prover (usable standalone) |

## Feature flags

| Flag | Crate | Effect |
| --- | --- | --- |
| `ffi` | `tpt-reficon` | Enables `#[ffi_contract]`, which needs libclang at build time |
| `unstable` | `tpt-reficon` | Reserved for experimental features (e.g. fractional borrows); semantics may change |
| `z3-reserved` | `tpt-reficon-verifier` | Reserved extension point for a future Z3 parity backend |

## Soundness notes

- **The solver decides over ℚ; the derive adds exact ℤ/domain checks for
  single-variable predicates.** The verifier's Fourier–Motzkin engine is exact
  over the rationals. For primitive integer inner types, `#[derive(Refine)]`
  additionally runs an exact single-variable integer-feasibility analysis per
  alternative, intersected with the type's domain, so rational-only refinements
  (`value > 5 && value < 6`, or `value > 255` on `u8`) are **compile errors**
  rather than always-panicking constructors. The verifier's own API remains a
  ℚ decision procedure.
- **Runtime checks are widened to `i128`.** Predicates on integer primitives
  must agree with the solver's reasoning, so the derive evaluates them in
  `i128` — never in the narrow inner type, where arithmetic could panic in
  debug builds or wrap (changing the verdict) in release builds.
- **`ensures(...)` on a refinement type is treated as an additional `requires`.**
  Post-state semantics apply to `#[contract]` functions, not to the derive.
- **Disjunctions (`||`) are decided exactly, but elision is branch-wise.**
  A disjunction is a compile error only when *every* alternative is
  unsatisfiable, and it is elided only when the domain proves *one* alternative.
  A tautology whose alternatives are merely jointly exhaustive (e.g.
  `value == 0 || value != 0`) is therefore not elided and keeps an
  always-succeeding runtime check.
- **Overflow is never a verdict.** Every arithmetic step in the solver is
  checked; overflow, exponential blow-up, and unsupported shapes degrade to
  "could not prove" (`Unknown`), never to a spurious answer.

## Security posture

- `unsafe_code` is **forbidden** across the workspace
  (`[workspace.lints.rust] unsafe_code = "forbid"`), and verified by clippy in
  CI. The one exception is the output of `ffi_wrappers!`: it calls C, so it
  contains an `unsafe` call. That call is confined to a wrapper marked
  `#[allow(unsafe_code)]`, and a wrapper that takes a raw pointer is itself
  `unsafe fn`. A crate that forbids `unsafe_code` cannot use `ffi_wrappers!`.
  The `#[derive(Refine)]` and `#[contract]` output contains no `unsafe`.
- All arithmetic in the decision procedure and the macro expansion is checked
  (`checked_*`); the only runtime `panic!` emitted into generated code is the
  documented `new` constructor's violation panic.
- The solver is defense-in-depth against hostile input: Fourier–Motzkin
  blow-up is capped (any excess becomes `Unknown`), `!=` case-splitting is
  budgeted, and adversarial models cannot force a panic or a spurious UNSAT
  through `satisfies`.
- Macro expansion is bounded: `i64` coefficient range and a hard elimination
  limit mean a single predicate cannot stall the compiler indefinitely.

## Security disclosures

Report vulnerabilities privately to the maintainers (see `CONTRIBUTING.md`);
there is no public issue process for security findings yet.

## License

Licensed under either of [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE) at
your option. See [NOTICE](NOTICE) for ported-code attribution.

## Further reading

- [Positioning doc](docs/positioning.md) — how tpt-reficon compares with
  `bindgen`, `cbindgen`, and `proptest`, and when each is the right tool.
