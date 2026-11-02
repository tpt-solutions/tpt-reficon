# Function contracts (`#[contract]`)

Status: implemented for integer parameters and integer returns. This is the
design the code follows, and the decisions it records.

## Surface

```rust
use tpt_reficon::prelude::*;

#[contract]                              // must be the outermost attribute
#[requires(divisor != 0)]
#[requires(amount >= 0 && amount <= limit)]
#[ensures(result >= 0)]
fn scaled(amount: i64, limit: i64, divisor: i64) -> i64 {
    amount * limit / divisor
}
```

- `#[requires(...)]` predicates may mention integer parameters.
- `#[ensures(...)]` predicates may mention integer parameters and `result`, which
  is the return value. The return type must be an integer type.
- Predicates use the same grammar as `#[derive(Refine)]`: comparisons, `&&`,
  `||`, `+`, `-`, integer literals.

## Decisions

1. **Attribute shape.** `requires` and `ensures` cannot be standalone proc
   attributes, because the name would collide with the `Refine` derive's helper
   attributes. `#[contract]` consumes them from the item, so it must be the
   outermost attribute. A misplaced `#[requires]` produces rustc's "cannot find
   attribute" error.
2. **Failure mode: panic.** A violated contract panics with the predicate text,
   the function name, and the kind of contract (`precondition` or
   `postcondition`). The signature stays the user's. This matches `new` on a
   refined type. A `Result`-returning variant is a later decision.
3. **Preconditions are checked at entry.** Checks run before the body. A
   precondition is elided only when the parameters' type domains prove it, the
   same rule `#[derive(Refine)]` uses. Otherwise the check stays in the code.
4. **Postconditions are checked at exit.** The body runs in a closure so that
   `return` statements and the tail expression both produce the value that is
   checked. A postcondition is elided only when the return type's domain together
   with the preconditions proves it. Body-level reasoning is out of scope, so
   every other postcondition stays a runtime check.
5. **Compile-time rejection.** A contract is an error when no arguments can
   satisfy its preconditions. The check is the solver's ℚ decision with the
   parameter domains included. Unsatisfiability over ℚ implies unsatisfiability
   over ℤ, so the error is sound. The reverse does not hold, so a contract that
   is only rationally satisfiable (`x > 5 && x < 6` on an integer parameter) is
   not rejected; it fails at runtime instead. The single-variable integer analysis
   in `integer.rs` is not applied to multi-parameter contracts.
6. **Overflow-free checks.** Predicates are evaluated with integer parameters and
   the result widened to `i128`, the same rule the refinement derive uses.
7. **Limits.** No generics, `async`, `const`, `unsafe`, `extern`, methods, or
   variadics. Parameters named `result` conflict with `#[ensures]`, and an
   error reports that.

## Soundness of elision

Premises for a precondition are the parameter domains. Premises for a
postcondition are the parameter domains, the return-type domain on `result`, and
the preconditions, which hold at exit because entry checked them or elided
them. Entailment for a disjunctive premise holds when every premise branch
entails some conclusion branch, which the `predicate` module already decides
over ℚ. Entailment over ℚ implies entailment over ℤ.

## Out of scope for now

- Contracts on methods and `self`.
- Non-integer parameters or results inside predicates.
- Reasoning about the body. The solver never sees the function body.
- Call-site checking of preconditions.
