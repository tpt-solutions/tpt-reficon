# Fractional borrows (prototype)

> **Status: prototype, behind the `unstable` feature.** The non-interference check exists in
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

`#[fractional_borrow]` (feature `unstable`) turns a struct of integers and
`bool` into lock-free shared state:

```rust
use tpt_reficon::prelude::*;

#[fractional_borrow]
#[derive(Debug, Clone, Copy)]
pub struct Counters {
    hits: u64,
    ok: bool,
}

let shared = CountersShared::new(Counters { hits: 0, ok: true });
if let Some(w) = shared.try_write_hits() {
    w.set(w.get() + 1);
}
assert_eq!(shared.try_read_hits().unwrap().get(), 1);
```

Permission is per field. Writers on `hits` do not block readers of `ok`.

The design, the literature review (Boyland 2003, Bornat et al. 2005, Iris,
RustBelt, Prusti, Creusot), and the limits are in
[`docs/design/fractional-borrows.md`](../../design/fractional-borrows.md).
