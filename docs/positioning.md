# tpt-reficon versus the usual suspects

A candid comparison of `tpt-reficon` with the tools it is most often grouped
with — `bindgen`, `cbindgen`, and `proptest` — so you can decide when it helps
you and when it does not.

## What tpt-reficon is

`tpt-reficon` turns hand-written runtime boilerplate into compile-time
verification for three things:

1. **Refinement types** — `#[derive(Refine)]` + `#[requires(...)]` prove (or
   reject) a value-level predicate over a type's domain, and elide the runtime
   check when it is proven.
2. **Contract-based FFI** — parse a foreign header, *prove* your `repr(C)`
   struct's layout matches it byte-for-byte, and generate a **safe** wrapper
   that enforces preconditions before the `unsafe` call (Phase 4).
3. **Fractional borrows** (experimental) — split read/write permissions across
   threads with solver-backed non-interference proofs (Phase 5).

Under the hood it is an exact, in-house Fourier–Motzkin decision procedure for
linear rational arithmetic (QF_LRA). It is **not** a general-purpose SMT solver,
a theorem prover, or a testing framework.

## The three tools

| Tool | Direction | What it guarantees | Input | Output |
| --- | --- | --- | --- | --- |
| [`bindgen`](https://crates.io/crates/bindgen) | C/C++ → Rust | Nothing about *your* Rust code — it generates `unsafe extern` declarations it believes match the header | C/C++ headers | `unsafe` Rust bindings |
| [`cbindgen`](https://crates.io/crates/cbindgen) | Rust → C/C++ | Nothing about *your* C code — it generates headers it believes match your Rust | Rust source | C/C++ headers |
| [`proptest`](https://crates.io/crates/proptest) | values → runtime | Empirical evidence a property holds on sampled inputs (strategy-driven, shrinking) | a property (`prop_assert!`) | a runtime result with counterexample |
| `tpt-reficon` | contracts → compile time | A *proof* that a contract holds on the type's whole domain (or a compile error / degrades to `Unknown`, never a wrong answer) | linear predicates over your data | proven elision or an explicit runtime check |

## Where tpt-reficon sits

### Refinement types — no current overlap, fills a gap

```rust
#[derive(Refine)]
#[requires(value > 0 && value <= 100)]
pub struct BoundedPercentage(u8); // 0 and u8::MAX are rejected, statically
```

There is no mainstream Rust analogue. `proptest` can *test* such invariants at
runtime; `tpt-reficon` *proves* them at `cfg`-time and removes the check when it
is provably redundant. The two are complementary: keep `proptest` for
properties the solver cannot decide (nonlinear, unbounded, historical state).

### FFI — complements, does not replace, `bindgen`

`bindgen` trusts the header and emits `unsafe`. That `unsafe` is a burden you
must audit every time the header changes. The `#[ffi_contract]` path (feature
`ffi`) is a *verification layer on top of* (or instead of) that auditing:

- `prove_equivalent`/`prove_within_bounds` ship in `tpt-reficon-verifier`
  (ported from `tpt-telos-uir-bridge`) to compare `repr(C)` layouts.
- `#[ffi_contract]` reads the layout from the header through bindgen, proves the
  struct matches it, and checks each field's type against the compile target.
- `ffi_wrappers!` checks each declared signature against the header and
  generates safe call sites that enforce preconditions.

So: `bindgen` converts the header into Rust; we convert the header into a
*layout proof* and a *safe call*. If you already use `bindgen`, tpt-reficon is a
supplement that re-checks ABI compatibility and removes the manual `unsafe`
audit — not a drop-in replacement.

### FFI — none with `cbindgen`

`cbindgen` goes Rust → C. `tpt-reficon`'s contract machinery goes C → Rust.
Reverse directions; they do not compete. A `cbindgen` header consumed by a C
codebase is exactly the kind of interface a future header parser can check.

### Summary

| Question | `bindgen` | `cbindgen` | `proptest` | `tpt-reficon` |
| --- | --- | --- | --- | --- |
| Converts headers to Rust? | ✅ | ❌ | ❌ | ❌ (checks, does not generate types) |
| Converts Rust to headers? | ❌ | ✅ | ❌ | ❌ |
| Verifies layout/ABI? | no | no | no | yes (`ffi` feature) |
| Emits `unsafe` code? | yes | n/a | no | **only** in `ffi_wrappers!`, which calls C; confined and marked |
| Runtime test generator? | no | no | yes | no |
| Static *proof* of predicates? | no | no | no | yes (QF_LRA subset) |
| Failure mode | goes wrong silently if header lies | goes wrong silently if code drifts | finds counterexamples (probabilistic) | **compile error or elision; never a wrong verdict** |
| Maturity | mature | mature | mature | 0.1.0, pre-publish |

### Other verification tools

These are the tools a reader is most likely to compare against. Each answers a
different question, so they complement each other more often than they replace
one another.

- **`nutype`** generates newtypes whose constructors sanitise and validate. The
  check is a runtime call, and nothing is proven. `tpt-reficon` checks the same
  kind of constructor, but a predicate the type's domain already implies is
  removed, and a predicate no value satisfies is a compile error.
- **Prusti** and **Creusot** are deductive verifiers. They take full functional
  specifications written in Rust and discharge them with a separate prover
  (Viper and Why3 respectively). They can express far more than linear
  predicates, and they need more annotations to do it. `tpt-reficon` takes one
  attribute per predicate and needs no prover beyond its own solver.
- **Kani** is a bounded model checker. It explores a harness's behaviours up to
  a bound and reports counterexamples. It can find bugs a refinement cannot
  express, but it says nothing beyond its bound. A refinement holds for every
  value of its type.
- **Flux** is a research refinement-type checker for Rust that also uses an SMT
  solver. It reasons about refinements inside function bodies, where
  `tpt-reficon` checks predicates on a type and at function boundaries. Its
  maturity and coverage differ, so check its current state before choosing it.

## Honest limitations

- **Only linear predicates.** `2*value*value == 8` is out of scope. The
  predicate grammar is linear comparisons combined with `&&` / `||` on `i64`
  coefficients. `proptest` remains the tool for everything nonlinear.
- **No full post-state model:** `#[ensures(...)]` checks the integer return
  value on exit. `old(x)` gives the entry value of an integer parameter.
- **FFI contracts are partial, and fractional borrows are a prototype.** The
  `ffi` feature covers C headers; Go headers are read through the cgo preamble.
  Fractional borrows sit behind the `unstable` feature.
- **`Unknown` is a feature, but it is a leak:** when the solver cannot prove a
  predicate it emits a runtime check rather than eliding it. That is the
  *correct* bias (never a false proof), but it means some checks are retained
  that a smarter solver would have removed.
- **Small team, pre-1.0:** expect API churn until `v0.1.0` tags.

## When to reach for which

- Assert a domain invariant on a value → `tpt-reficon` `#[derive(Refine)]`.
- Explore an invariant you cannot yet express linearly → `proptest`, and later
  consider strengthening it into a refinement.
- Generically bind an existing C/C++ library → `bindgen`.
- Export a stable ABI from Rust → `cbindgen`.
- Ship a C ABI and need to prove your `repr(C)` matches and your unsafe calls
  are guarded → `tpt-reficon` FFI contracts (Phase 4) on top of your existing
  `bindgen` bindings.