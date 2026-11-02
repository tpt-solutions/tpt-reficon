# Fractional borrows (prototype)

> **Status: research prototype.** The non-interference check exists in
> `tpt-reficon-verifier` (`permission` module). The `#[fractional_borrow]` macro
> is not built yet, and the `unstable` feature flag on `tpt-reficon` is reserved
> for it. Semantics may change. Track progress in `todo.md`, Phase 5.

A shared resource carries one unit of permission, split into fractions held by
concurrent accessors. Readers hold any positive fraction. A writer holds the
whole unit, so it cannot coexist with any other holder.

The prototype checks a proposed set of accesses with the solver:

```rust
use tpt_reficon_verifier::permission::{check_configuration, Access};

assert!(check_configuration(&[Access::Read, Access::Read]).is_ok());
assert!(check_configuration(&[Access::Write, Access::Read]).is_err());
```

`Err` means the solver proved that no fraction split grants those accesses at
once.

The design, the literature review (Boyland 2003, Bornat et al. 2005, Iris,
RustBelt, Prusti, Creusot), and the limits are in
[`docs/design/fractional-borrows.md`](../../design/fractional-borrows.md).
