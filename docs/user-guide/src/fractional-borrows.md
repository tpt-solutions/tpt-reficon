# Fractional borrows (planned)

> **Status: research prototype, not implemented.** The `unstable` feature flag on
> `tpt-reficon` is reserved for this work, and its semantics may change. Track
> progress in `todo.md`, Phase 5.

The planned `#[fractional_borrow]` macro will split access permissions across
threads. Non-interference between the fractions will be checked by the solver,
and accessor code will be generated for lock-free use.

Prior art includes RustBelt, Iris, Prusti, and Creusot. None of them is used
directly by this project.
