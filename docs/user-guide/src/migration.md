# Migrating from other validation approaches

A refinement replaces a runtime validation that you would otherwise write by
hand. This page sketches the common sources and what changes.

## From `assert!` in a constructor

```rust,ignore
// before
pub struct Port(u16);
impl Port {
    pub fn new(v: u16) -> Port {
        assert!(v >= 1, "port must be non-zero");
        Port(v)
    }
}
```

```rust,ignore
// after
#[derive(Refine)]
#[requires(value >= 1)]
pub struct Port(u16);
```

`Port::new` keeps the panic, and the message now names the refinement. Use
`Port::try_new` where you would have returned an error. Bounds that the type
already implies, such as `value <= 65535` on a `u16`, cost nothing.

## From `validator`, `garde`, or similar field-attribute crates

Those crates check a struct's fields when you call `validate()`, so an invalid
value can exist for a while. A refinement makes the invalid value unrepresentable:
you cannot hold a `Port` that is zero. The trade-off is that predicates are
linear, with no regular expressions and no callbacks. Keep the validator crate
for checks that need those.

## From `nutype`

`nutype` generates newtypes with sanitisers and validators. The refinement
derive covers the validation half: the predicate is in the attribute and is
checked in `new` / `try_new`. It does not generate `Display`, `FromStr`, or
serde impls yet (see todo.md, Phase 12).

## Behaviour changes to check

- **`u128` above `i128::MAX`:** the runtime check refuses these values, so a
  predicate that such a value satisfies is still rejected. Watch this if you
  store large `u128` values.
- **Type aliases:** `#[derive(Refine)]` needs a bare primitive inner type
  (`u8`, `i64`, `bool`, `f64`, ...). Write the primitive, not an alias such as
  `type Byte = u8`.
- **Private inner field:** a `pub` inner field is rejected, so no caller can
  build the value without the check.
- **No unchecked constructor:** `from_inner_unchecked` was removed. Use `try_new`.
