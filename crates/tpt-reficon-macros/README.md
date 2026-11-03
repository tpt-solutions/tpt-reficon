# tpt-reficon-macros

The proc-macro frontend for [`tpt-reficon`](https://crates.io/crates/tpt-reficon).
It parses predicates, translates them straight into the
[`tpt-reficon-verifier`](https://crates.io/crates/tpt-reficon-verifier) constraint
IR, and discharges them at macro-expansion time. The expansion either omits a
check the solver or domain proves, or emits a runtime check.

Depend on the `tpt-reficon` facade rather than this crate. The facade
re-exports every macro here and sets the feature flags for you. Depend on this
crate directly only if you need the macros without the facade.

## Contents

- [Macros](#macros)
- [How a predicate is handled](#how-a-predicate-is-handled)
- [Feature flags](#feature-flags)
- [Diagnostics](#diagnostics)
- [Minimum supported Rust version](#minimum-supported-rust-version)
- [License](#license)

## Macros

### `#[derive(Refine)]`

Derives a refinement type on a single-field struct, named or tuple. Attributes:

- `#[requires(<expr>)]` (repeatable): a predicate the inner value must satisfy.
  Predicates are conjoined. Each is parenthesized in the generated check.
- `#[ensures(<expr>)]`: accepted as an extra conjoined predicate.
- `#[refine(variable = "name")]`: names the predicate variable for the inner
  value. It defaults to the field name, or to `value` for tuple structs.

Predicate grammar: linear comparisons (`<`, `<=`, `>`, `>=`, `==`, `!=`) over
the variable, combined with `&&` and `||`. Disjunctions are lowered to disjunctive
normal form. A pathological expansion is rejected with a span-accurate error
rather than expanding without bound.

The derive generates `new`, `try_new`, `TryFrom`, `get`, and `into_inner`.
Both `new` and the `TryFrom` path check the predicate at runtime, unless the
check was elided.

### `#[contract]`

Adds pre- and postconditions to a function. Apply it as the outermost attribute.
Inside it, `#[requires(...)]` may mention integer parameters, and `#[ensures(...)]`
may also mention `result`, the return value. A violated condition panics.
Conditions proven by the parameter or return-type domains are removed.
Unsatisfiable preconditions are compile errors.

### `#[ffi_contract]` (feature `ffi-bindgen`)

Proves that a `#[repr(C)]` struct matches a struct in a C or Go header.

- `bridge = "c"` or `bridge = "go"`. For Go, the struct must be declared as a C
  struct in the cgo preamble.
- `header = "..."`: the header path, relative to `CARGO_MANIFEST_DIR`.
- `c_name = "..."`: optional, when the C name differs from the Rust name.

Layout comes from clang through bindgen's layout assertions. The check covers
field name, offset, size, alignment, and total size. The expansion includes
`const` size and alignment assertions. Requires libclang at build time.

### `ffi_wrappers!` (feature `ffi-bindgen`)

Declares safe wrappers around C functions in a header. Each Rust signature is
checked against the header. The `#[requires]` and `#[ensures]` contracts run
before the unsafe call, and the call is confined to the wrapper. The crate that
uses it must allow `unsafe_code`, since the expansion contains an `unsafe`
block.

### `#[fractional_borrow]` (feature `unstable`, prototype)

Turns a struct of `bool` and integer fields into typed shared state. It emits a
`<Name>Shared` type with one permission-guarded cell per field, plus
`try_read_<field>` and `try_write_<field>` accessors. The runtime types are in
[`tpt-reficon-core`](https://crates.io/crates/tpt-reficon-core).

## How a predicate is handled

1. The predicate is parsed into the constraint IR. Anything nonlinear or
   unsupported is a compile error that names the offending expression.
2. The solver checks satisfiability over ℚ. An unsatisfiable predicate is a
   compile error. For primitive integer inner types, an exact
   single-variable integer-feasibility check also runs against the domain,
   so rational-only refinements are caught.
3. The domain check: if the type's domain already implies the predicate, the
   runtime check is dropped entirely.
4. Otherwise the expansion contains a runtime check. Integer predicates are
   evaluated in `i128`, so they cannot overflow or wrap in the narrow inner type.

Solver results are never spurious. Overflow, blow-up, and unsupported shapes
produce "could not prove", which falls back to the runtime check.

## Feature flags

| Flag | Effect |
| --- | --- |
| `ffi-bindgen` | Enables `#[ffi_contract]` and `ffi_wrappers!`. Pulls in `bindgen`, which needs libclang at build time. |
| `unstable` | Enables `#[fractional_borrow]`. The semantics may change. |

## Diagnostics

Compile errors point at the predicate's span. Runtime failures carry a
`'static` message naming the predicate and type, so the check never allocates
and never needs the inner type to implement `Debug`. The UI tests under
`tests/ui/` pin the exact diagnostics for both the passing and failing cases.

## Minimum supported Rust version

1.75.0.

## License

Licensed under either of [MIT](https://github.com/tpt-solutions/tpt-reficon/blob/master/LICENSE-MIT)
or [Apache-2.0](https://github.com/tpt-solutions/tpt-reficon/blob/master/LICENSE-APACHE)
at your option.
