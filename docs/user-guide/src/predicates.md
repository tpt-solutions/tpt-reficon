# Writing predicates

Predicates are ordinary Rust boolean expressions over a single variable: the
inner value, named by `value` or by `#[refine(variable = "...")]`.

## Supported forms

| Form | Example |
| --- | --- |
| Comparison | `value > 0`, `value != 7` |
| Linear arithmetic | `value + 200 < 300`, `2 * value - 3 >= 0` |
| Conjunction | `value > 0 && value <= 100` |
| Disjunction | `value == 0 \|\| value == 42` |

Several `#[requires(...)]` attributes on one type are conjoined, so these two
declarations are equivalent:

```rust
#[requires(value > 0 && value <= 100)]
// is the same as
#[requires(value > 0)]
#[requires(value <= 100)]
```

## Integer arithmetic

Runtime checks on integer primitives are evaluated in `i128`, not in the inner
type. The predicate `value + 200 < 300` on a `u8` therefore never panics in
debug or wraps in release: a `u8` of `150` evaluates to `350 < 300`, which is
false, so it is rejected.

## Disjunctions

A disjunction is lowered to disjunctive normal form and each alternative is
checked separately. The derive applies a branch cap so that a large disjunction
cannot blow up compile time; past the cap, the predicate is not analysed and a
runtime check is emitted.

## Not supported

- Nonlinear terms such as `value * value`. These are rejected with a compile error.
- Predicates that refer to names other than the predicate variable.
- Generic refined types.
