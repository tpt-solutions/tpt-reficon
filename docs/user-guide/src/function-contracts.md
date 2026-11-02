# Function contracts

`#[contract]` adds pre- and postconditions to a plain function. Integer
parameters and an integer return value are supported.

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

- `#[requires(...)]` may mention parameters. It is checked when the function is
  entered.
- `#[ensures(...)]` may mention parameters and `result`, the return value. It is
  checked when the function exits, whether through `return` or the tail
  expression.
- A violated contract panics. The message names the contract, the function, and
  whether it was a precondition or a postcondition.

## What the compiler decides

- **Unsatisfiable preconditions are errors.** If no arguments satisfy a
  precondition, the function does not compile. For example,
  `#[requires(x > 5 && x < 3)]` is rejected.
- **Proven predicates are free.** A precondition that the parameter types prove,
  such as `x <= 255` on a `u8`, is compiled out. A postcondition that the
  return type proves, such as `result <= 255` on a `u8`, is compiled out too.
- **Postconditions on the body are checked at runtime.** The solver never sees
  the function body, so `#[ensures(result == a + 1)]` stays a check.

## Limits

- Methods and `self` are not supported, nor are generic, `async`, `const`,
  `unsafe`, or `extern` functions.
- Predicates mention integer parameters only. `#[ensures]` needs an integer
  return type.
- A parameter named `result` conflicts with `#[ensures]`.

The design, and the reasons for these choices, are in
[`docs/design/function-contracts.md`](../../design/function-contracts.md).
