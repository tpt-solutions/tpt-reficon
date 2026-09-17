<div align="center">

# tpt-reficon

**Refinement types, FFI contracts, and fractional borrows for Rust — verified
at compile time by an in-house QF_LRA solver.**

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

### Roadmap

Not yet implemented — see [`todo.md`](todo.md) for the plan:

- **Contract-based FFI** — `#[ffi_contract(bridge = "c", header = "...")]` to
  parse a C header, compute its ABI layout, and *prove* your `repr(C)` struct
  mirrors it byte-for-byte before you ever write `unsafe`.
- **Fractional borrows (experimental)** — `#[fractional_borrow]` to split
  access permissions across threads with non-interference validated by the
  solver. Behind the `unstable` feature flag.

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

    // This call site is proven at compile time... but only for literals the
    // macro can see; `new` panics on violation with a span-accurate message.
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
| `unstable` | `tpt-reficon` | Reserved for experimental features (e.g. fractional borrows); semantics may change |
| `z3-reserved` | `tpt-reficon-verifier` | Reserved extension point for a future Z3 parity backend |

## Soundness notes

- **Decision procedure is over ℚ (rationals), not ℤ.** For refinement types on
  integer primitives this is *sound* (if no rational solution exists, no
  integer one does either) but not *complete*: a refinement that is
  rational-satisfiable yet integer-empty (e.g. `2*value == 1` on `u8`) is not
  caught at compile time and instead behaves as an always-panicking runtime
  check. When unsure, write predicates whose rational and integer solutions
  coincide (e.g. `value < 100`).
- **Runtime checks are widened to `i128`.** Predicates on integer primitives
  must agree with the solver's reasoning, so the derive evaluates them in
  `i128` — never in the narrow inner type, where arithmetic could panic in
  debug builds or wrap (changing the verdict) in release builds.
- **`ensures(...)` is currently treated as an additional `requires`.** The
  post-state semantics of real `ensures` clauses are not yet modelled.
- **Overflow is never a verdict.** Every arithmetic step in the solver is
  checked; overflow, exponential blow-up, and unsupported shapes degrade to
  "could not prove" (`Unknown`), never to a spurious answer.

## Security posture

- `unsafe_code` is **forbidden** across the workspace
  (`[workspace.lints.rust] unsafe_code = "forbid"`), and verified by clippy in
  CI. There is no `unsafe` in any crate — including generated code.
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
