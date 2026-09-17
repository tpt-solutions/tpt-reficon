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
must audit every time the header changes. The planned `#[ffi_contract]` path is
a *verification layer on top of* (or instead of) that auditing:

- `prove_equivalent`/`prove_within_bounds` already ship in
  `tpt-reficon-verifier` (ported from `tpt-telos-uir-bridge`) to compare
  `repr(C)` layouts.
- The Phase 4 macro will parse the header, derive the expected layout, prove
  the struct matches, and generate safe call sites enforcing preconditions.

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
| Converts headers to Rust? | ✅ | ❌ | ❌ | planned (header → layout proof) |
| Converts Rust to headers? | ❌ | ✅ | ❌ | ❌ |
| Verifies layout/ABI? | no | no | no | yes (planned for FFI) |
| Emits `unsafe` code? | yes | n/a | no | **no** (safe wrappers; `unsafe_code` is `forbid`) |
| Runtime test generator? | no | no | yes | no |
| Static *proof* of predicates? | no | no | no | yes (QF_LRA subset) |
| Failure mode | goes wrong silently if header lies | goes wrong silently if code drifts | finds counterexamples (probabilistic) | **compile error or elision; never a wrong verdict** |
| Maturity | mature | mature | mature | 0.1.0, pre-publish |

## Honest limitations

- **Only linear predicates.** `2*value*value == 8` is out of scope. The
  predicate grammar is linear comparisons combined with `&&` / `||` on `i64`
  coefficients. `proptest` remains the tool for everything nonlinear.
- **Single post-state model is not yet real:** `#[ensures(...)]` is currently a
  synonym for an extra `#[requires(...)]` (see `README.md` soundness notes).
- **FFI contracts and fractional borrows are not shipped yet** (Phases 4–5 in
  `todo.md`). The verifier's layout utilities are available today.
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