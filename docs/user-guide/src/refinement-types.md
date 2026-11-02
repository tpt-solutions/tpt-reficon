# Refinement types

Apply `#[derive(Refine)]` to a tuple struct with one field, or to a struct with
one named field:

```rust
use tpt_reficon::prelude::*;

#[derive(Refine)]
#[requires(value <= 255)]
pub struct Byte(u8);

#[derive(Refine)]
#[refine(variable = "x")]
#[requires(x >= -10 && x <= 10)]
pub struct SmallOffset {
    x: i16,
}
```

- `#[requires(...)]` attaches a predicate. Several attributes are conjoined.
- The predicate variable defaults to `value` for a tuple struct, and to the
  field name for a named-field struct. Override it with `#[refine(variable = "...")]`.
- Predicates may use `&&`, `||`, comparisons, and linear arithmetic over the
  inner value.

The derive rejects shapes it cannot reason about (generic structs, multiple
fields, unknown variables, nonlinear terms) with a compile error that points at
the offending token.

## What the derive does with a predicate

For each predicate the derive runs the solver and picks one of three outcomes:

1. **Unsatisfiable** — no value can satisfy the refinement. This is a **compile
   error**, for example `#[requires(value > 5 && value < 3)]`.
2. **Implied by the domain** — the inner type's range already guarantees the
   predicate, for example `value <= 255` on `u8`. `new` and `try_new` emit no
   check, and `try_new` always returns `Ok`.
3. **Neither** — the derive emits an exact runtime check in `new` and `try_new`.

See [How checks are decided](checks.md) for the rules behind each outcome.
