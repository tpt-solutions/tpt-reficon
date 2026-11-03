# Introduction

`tpt-reficon` moves two kinds of runtime validation to compile time:

- **Refinement types** — a newtype carries a predicate over its inner value
  (`value > 0 && value <= 100`). Construction either proves the predicate at
  macro-expansion time, or emits a precise runtime check.
- **Function contracts** — `#[contract]` checks `#[requires]` at entry and
  `#[ensures]` on the return value. Checks the parameter domains prove are removed.
- **FFI contracts** (feature `ffi`) — `#[ffi_contract]` proves a `repr(C)` struct
  matches its C header layout, and `ffi_wrappers!` declares checked wrappers
  around C functions. Partial: see the [FFI contracts](ffi-contracts.md) chapter.
- **Fractional borrows** (feature `unstable`) — a prototype that turns a struct of
  integers and `bool` into lock-free shared state with fractional permissions.
  See the [fractional borrows](fractional-borrows.md) chapter.

Everything is decided by an in-house Fourier–Motzkin QF_LRA solver, ported from
[tpt-telos](https://github.com/tpt-solutions/tpt-telos). The solver is a
self-contained Rust module, so the workspace has no external solver dependency.

## Getting started

```toml
[dependencies]
tpt-reficon = "0.1"
```

```rust,edition2021
# extern crate tpt_reficon;
# extern crate tpt_reficon_core;
use tpt_reficon::prelude::*;

#[derive(Refine)]
#[requires(value > 0 && value <= 100)]
pub struct BoundedPercentage(u8);

fn main() {
    let pct = BoundedPercentage::new(42); // panics on violation
    assert_eq!(*pct, 42);                 // Deref to the inner value
    assert!(BoundedPercentage::try_new(101).is_err());
}
```

Each derived type gets these items:

| Item | Behaviour |
| --- | --- |
| `new(value)` | Constructs the type; panics with a span-accurate message on violation. |
| `try_new(value)` | Returns `Result<Self, RefinementError>`. |
| `get(&self)` | Returns a copy of the inner value (inner type must be `Copy`). |
| `into_inner(self)` | Consumes the wrapper and returns the raw value. |
| `Deref<Target = Inner>` | Borrows the inner value. |
| `TryFrom<Inner>` | Same as `try_new`. |

A complete tour lives in `crates/tpt-reficon/examples/refinement_types.rs`; run
it with `cargo run --example refinement_types`.
