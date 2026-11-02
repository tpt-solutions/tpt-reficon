# Fractional borrows (Phase 5 design and literature review)

Status: research prototype. The solver-backed non-interference check lives in
`crates/tpt-reficon-verifier/src/permission.rs`. The `#[fractional_borrow]`
macro and accessor codegen are not built yet.

## Literature review

- **Fractional permissions** (Boyland, 2003). A location's permission is a
  real number in `(0, 1]`. Reads hold any positive fraction; a write holds
  the full `1`. Splitting and joining preserve the total. This is the model the
  prototype encodes.
- **Permission accounting** (Bornat, Calcagno, O'Hearn, Parkinson, 2005).
  Separation-logic treatment of fractional permissions, which gives the
  soundness argument for concurrent use.
- **Iris** (Jung et al.). A higher-order concurrent separation logic framework
  with ghost state and invariants. It is the most general of the frameworks here,
  and is a proof assistant rather than a tool for Rust code.
- **RustBelt** (Jung et al., 2018). Iris-based semantic proof that Rust's
  ownership and borrowing rules are sound, including the `unsafe` abstractions
  built on them. It is the reference for what a Rust-specific model must cover.
- **Prusti** (ETH Zürich). Deductive verifier for Rust built on Viper; it
  supports permissions and specifications directly in Rust code.
- **Creusot** (Denis, Jourdan, Marché). Deductive verifier for Rust that
  translates to Why3.

What this project takes from them: the fraction algebra (Boyland) and a
solver-discharged check (not a proof assistant). What it does not take:
ghost state and higher-order invariants. Those need a proof assistant, which is
out of scope.

## Design

Surface syntax (proposed, not implemented):

```rust
#[fractional_borrow]
pub struct Shared {
    counter: u64,
}
```

Each access produces a `Frac<'a, Shared, Q>` handle. A `Read` handle holds a
positive fraction and may be cloned by splitting. A `Write` handle holds the
full unit. Dropping a handle returns its fraction to the pool.

The non-interference obligation is the one `permission::check_configuration`
checks: for any set of live handles, a fraction assignment exists with
`Σ q_i ≤ 1`, `q_i > 0`, and `q_w = 1` for each writer.

## Prototype scope and limits

- Implemented: the configuration check, with tests for reader-only, single
  writer, writer-plus-reader, and two-writer configurations. It is exact over ℚ.
- Not implemented: the accessor codegen. A lock-free accessor needs an
  `UnsafeCell` or equivalent. The workspace forbids `unsafe_code`, so the
  codegen is blocked on a decision about whether to relax that rule for generated
  code. Recommendation: keep the rule, and restrict the first accessor prototype to
  `Copy` integer fields backed by `std` atomics, which needs no `unsafe`.
- Not modelled: the ordering of handle creation and drop across threads. The
  check is a static configuration check, not a runtime protocol proof.
