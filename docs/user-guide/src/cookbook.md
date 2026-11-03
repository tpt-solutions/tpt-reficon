# Cookbook

Short recipes for common refinement and contract patterns. Each one has a
runnable counterpart in `crates/tpt-reficon/examples/`.

## A value that is never zero

Use a refinement and pass the type around, so the divisor needs no check at the
call site. See `non_zero_quantity.rs`.

```rust,edition2021
# extern crate tpt_reficon;
# extern crate tpt_reficon_core;
use tpt_reficon::prelude::*;

#[derive(Refine, Debug, Clone, Copy)]
#[requires(value > 0)]
pub struct NonZeroQuantity(u32);

fn per_part(total: u32, parts: NonZeroQuantity) -> u32 {
    total / *parts
}

assert_eq!(per_part(12, NonZeroQuantity::new(4)), 3);
assert!(NonZeroQuantity::try_new(0).is_err());
```

## Excluding a range

Negation works on comparisons, `&&` and `||`, so an open range is written
directly. `!(a && b)` is checked as `!a || !b`.

```rust,edition2021
# extern crate tpt_reficon;
# extern crate tpt_reficon_core;
use tpt_reficon::prelude::*;

#[derive(Refine, Debug)]
#[requires(!(value > 10 && value < 20))]
pub struct OutsideTeens(u8);

assert!(OutsideTeens::try_new(5).is_ok());
assert!(OutsideTeens::try_new(15).is_err());
```

## Choosing between `new` and `try_new`

`new` panics on a violation, which suits a value that is a bug if wrong. `try_new`
returns `Err`, which suits input from outside the program. Both run the same check.

```rust,edition2021
# extern crate tpt_reficon;
# extern crate tpt_reficon_core;
use tpt_reficon::prelude::*;

#[derive(Refine, Debug)]
#[requires(value >= 1)]
pub struct Port(u16);

fn from_user(input: u16) -> Result<Port, RefinementError> {
    Port::try_new(input)
}

assert!(from_user(0).is_err());
assert_eq!(*from_user(8080).unwrap(), 8080);
```

## Taking the plain value back out

A refined value converts into its inner type with `From`, and `Deref` reads it
in place. Going back in always goes through `try_new` or `new`.

```rust,edition2021
# extern crate tpt_reficon;
# extern crate tpt_reficon_core;
use tpt_reficon::prelude::*;

#[derive(Refine, Debug)]
#[requires(value >= 1)]
pub struct Ticket(u32);

let plain: u32 = Ticket::new(7).into();
assert_eq!(plain, 7);
```

## Checking a function's inputs and output

`#[contract]` checks `requires` on entry and `ensures` on the return value. The
checks the parameter and return domains already prove are removed.

```rust,edition2021
# extern crate tpt_reficon;
# extern crate tpt_reficon_core;
use tpt_reficon::prelude::*;

#[contract]
#[requires(parts > 0)]
#[ensures(result >= 0)]
fn share(total: i64, parts: i64) -> i64 {
    total / parts
}

assert_eq!(share(10, 4), 2);
```

See [function contracts](function-contracts.md) for what is and is not checked.

## Checking a literal at compile time

`refined!(Type, literal)` builds the value and checks the literal during
compilation, so a literal that breaks the refinement fails to compile:

```rust,edition2021
# extern crate tpt_reficon;
# extern crate tpt_reficon_core;
use tpt_reficon::prelude::*;

#[derive(Refine, Debug)]
#[requires(value >= 1)]
pub struct Port(u16);

let port = tpt_reficon::refined!(Port, 8080);
assert_eq!(*port, 8080);
```
