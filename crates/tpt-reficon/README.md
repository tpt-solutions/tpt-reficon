# tpt-reficon

Refinement types, function contracts, FFI contracts, and fractional borrows for
Rust. Predicates are checked at compile time by an in-house QF_LRA
(quantifier-free linear real arithmetic) solver that runs during macro
expansion. Checks the solver can prove redundant are removed, so they cost
nothing at runtime. Every other check is a precise runtime check with a
span-accurate diagnostic.

This is the facade crate. Start here: it re-exports the derive macro, the
attribute macros, and the runtime types behind one `prelude` module. The
implementation lives in [`tpt-reficon-macros`](https://crates.io/crates/tpt-reficon-macros)
(proc macros), [`tpt-reficon-core`](https://crates.io/crates/tpt-reficon-core)
(runtime types), and [`tpt-reficon-verifier`](https://crates.io/crates/tpt-reficon-verifier)
(solver and layout proofs).

## Contents

- [Features](#features)
- [Quick start](#quick-start)
- [Feature flags](#feature-flags)
- [What is proven, what is checked at runtime](#what-is-proven-what-is-checked-at-runtime)
- [Status](#status)
- [Documentation](#documentation)
- [Minimum supported Rust version](#minimum-supported-rust-version)
- [Security](#security)
- [License](#license)

## Features

- **Refinement types.** `#[derive(Refine)]` with `#[requires(...)]` on a
  single-field newtype. Predicates are linear comparisons joined with `&&` and
  `||`. A contradictory refinement is a compile error. A refinement the type's
  domain already implies generates no code.
- **Integer-domain completeness.** For primitive integer inner types, an exact
  single-variable integer-feasibility check runs against the type's domain. So
  `value > 5 && value < 6`, or `value > 255` on `u8`, is rejected at compile
  time rather than producing a constructor that always panics.
- **Checked literals.** `refined!(Type, literal)` fails to compile when the
  literal breaks the refinement.
- **Derive extras.** `#[derive(Refine)]` also implements `From`, `AsRef`,
  `Display`, and `FromStr`. Opt-in `#[refine(serde)]` adds a `Deserialize` that
  validates the value, and `#[refine(proptest)]` adds an `Arbitrary` that yields
  only valid values.
- **Function contracts.** `#[contract]` adds `#[requires]` and `#[ensures]`
  (with `result`) to functions and methods over integer parameters and return
  values. `old(x)` in `#[ensures]` refers to a parameter's value at entry.
- **FFI contracts** (feature `ffi`). `#[ffi_contract]` proves that a
  `#[repr(C)]` struct matches a C or Go struct declared in a header, checking
  field names, offsets, sizes, alignment, and total size. `ffi_wrappers!`
  declares safe wrappers around C functions, checks each signature against the
  header, and runs the contracts before the unsafe call.
- **Fractional borrows** (feature `unstable`, prototype). `#[fractional_borrow]`
  turns a struct of integers and `bool` into lock-free shared state with one
  fractional permission per field.

## Quick start

```toml
[dependencies]
tpt-reficon = "0.1"
```

```rust
use tpt_reficon::prelude::*;

// Refinement type: the inner value must satisfy the predicate.
#[derive(Refine)]
#[requires(value > 0 && value <= 100)]
pub struct BoundedPercentage(u8);

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Runtime-checked constructor: the predicate cannot hold for every u8.
    let pct = BoundedPercentage::try_new(42)?;
    assert_eq!(*pct, 42);

    // Panics on violation, with a message that names the predicate.
    let full = BoundedPercentage::new(100);
    assert_eq!(full.get(), 100);

    Ok(())
}
```

A refinement no value can satisfy fails to compile:

```rust,compile_fail
#[derive(tpt_reficon::Refine)]
#[requires(value > 5 && value < 3)] // error: unsatisfiable refinement
struct Impossible(u8);
```

A refinement the domain already guarantees generates no check:

```rust
#[derive(tpt_reficon::Refine)]
#[requires(value >= 0)] // always true for u64, so no code is emitted
struct NonNegativeU64(u64);
```

A longer walkthrough is in
[`examples/refinement_types.rs`](https://github.com/tpt-solutions/tpt-reficon/blob/master/crates/tpt-reficon/examples/refinement_types.rs).
Run it with `cargo run --example refinement_types`.

## Feature flags

| Flag | Effect |
| --- | --- |
| `ffi` | Enables `#[ffi_contract]` and `ffi_wrappers!`. Needs libclang at build time, because header parsing goes through bindgen. |
| `unstable` | Enables `#[fractional_borrow]` and the lock-free `SharedU64` accessor. The semantics may change between releases. |

The default feature set has neither flag, so `cargo add tpt-reficon` builds
without libclang.

## What is proven, what is checked at runtime

- **Proven at compile time:** unsatisfiable refinements (compile error), and
  refinements the domain already implies (check removed).
- **Checked at runtime:** every other refinement. `new` panics on violation.
  `try_new` and `TryFrom` return a `RefinementError`. The message is `'static`
  and never requires the inner type to implement `Debug`.
- **Decided over ℚ, then tightened over ℤ:** the solver's Fourier–Motzkin
  elimination is exact over the rationals. For primitive integer types the
  derive adds an exact single-variable integer check, so rational-only
  refinements are rejected too.
- **Runtime arithmetic is widened to `i128`.** A predicate such as
  `value + 200 < 300` on `u8` cannot overflow or wrap, so the runtime verdict
  agrees with the solver.

Disjunctions are decided exactly, but elision is per alternative. A tautology
such as `value == 0 || value != 0` is not elided, so it keeps a check that
always succeeds.

The full soundness model is in the
[user guide](https://github.com/tpt-solutions/tpt-reficon/tree/master/docs/user-guide/src/soundness.md).

## Status

| Area | State |
| --- | --- |
| Refinement types (`#[derive(Refine)]`) | Implemented |
| Function contracts (`#[contract]`, integer parameters and results) | Implemented |
| `#[ffi_contract]` for `bridge = "c"` and `bridge = "go"` | Implemented, behind `ffi` |
| `ffi_wrappers!` | Implemented, behind `ffi` |
| `#[fractional_borrow]` | Prototype, behind `unstable` |
| `ensures` with `result.is_ok()` (`spec.txt` form) | Not supported |

`ensures(...)` on a refinement type is treated as an extra `requires`. Post-state
semantics apply only to `#[contract]` functions.

## Documentation

- [User guide](https://github.com/tpt-solutions/tpt-reficon/tree/master/docs/user-guide/src/SUMMARY.md):
  refinement types, predicate syntax, how checks are decided, FFI contracts,
  fractional borrows, soundness.
- [Positioning](https://github.com/tpt-solutions/tpt-reficon/blob/master/docs/positioning.md):
  how this compares with `bindgen`, `cbindgen`, and `proptest`, and when each
  is the right tool.
- Design notes in
  [`docs/design/`](https://github.com/tpt-solutions/tpt-reficon/tree/master/docs/design):
  function contracts, FFI wrappers, Go header ingestion, fractional borrows.
- API reference on [docs.rs](https://docs.rs/tpt-reficon).

## Minimum supported Rust version

1.75.0. CI runs an MSRV build job on that toolchain.

## Security

`unsafe_code` is forbidden in this crate and across the workspace. The
generated code contains no `unsafe`. Report vulnerabilities privately, as
described in
[CONTRIBUTING.md](https://github.com/tpt-solutions/tpt-reficon/blob/master/CONTRIBUTING.md).

## License

Licensed under either of [MIT](https://github.com/tpt-solutions/tpt-reficon/blob/master/LICENSE-MIT)
or [Apache-2.0](https://github.com/tpt-solutions/tpt-reficon/blob/master/LICENSE-APACHE)
at your option. See the
[NOTICE](https://github.com/tpt-solutions/tpt-reficon/blob/master/NOTICE) file
for attribution of the code ported from TPT Solutions' tpt-telos.

Copyright © 2026 TPT Solutions.
