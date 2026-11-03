//! Type-domain entailment for zero-cost elision.
//!
//! A runtime check is elided only when the verifier *proves* the refined
//! type's underlying domain already implies the predicate. This module turns
//! the inner type into a set of domain constraints over the refined value
//! variable; if the domain entails every predicate constraint, the derived
//! constructors skip the check entirely.
//!
//! Domain constraints are deliberately *weaker* where exact bounds do not fit
//! the solver's `i64` coefficient range (e.g. `u128`, platform `usize`):
//! a weaker premise can only reduce elision, never cause a check to be
//! omitted unsoundly.

use syn::Type;

use tpt_reficon_verifier::ir::{Constraint, Linear, Relation};

/// The primitive scalar types the derive understands, by their bare name.
const PRIMITIVES: &[&str] = &[
    "bool", "u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32", "i64", "i128", "isize",
    "f32", "f64",
];

/// The primitive named by `ty`, if `ty` is written as a bare primitive name.
///
/// Only a single-segment path counts. A qualified path such as `m::u8` or
/// `std::primitive::u8` is not resolved, and neither is a type alias: an alias
/// named `Byte` may stand for anything, so it gets no domain and no integer
/// widening. Callers reject non-primitive inner types outright.
///
/// Known limit: a module-level `type u8 = ...;` shadows the primitive and this
/// cannot see it, because a proc macro has no name resolution.
pub fn primitive_name(ty: &Type) -> Option<&'static str> {
    let Type::Path(p) = ty else {
        return None;
    };
    if p.qself.is_some() || p.path.leading_colon.is_some() || p.path.segments.len() != 1 {
        return None;
    }
    let seg = &p.path.segments[0];
    if !seg.arguments.is_empty() {
        return None;
    }
    let ident = seg.ident.to_string();
    PRIMITIVES.iter().copied().find(|prim| *prim == ident)
}

/// The expression `var + bound` with the bound negated: the row `var (rel) bound`.
fn bound_expr(var: &str, bound: i64) -> Linear {
    Linear::from_parts(vec![(var.to_string(), 1)], -bound).expect("domain bound fits in i64")
}

/// Computes the domain constraints for `ty` over the variable `var`.
///
/// Unknown types yield an empty domain (no elision, always a runtime check).
pub fn domain_constraints(ty: &Type, var: &str) -> Vec<Constraint> {
    let Some(name) = primitive_name(ty) else {
        return Vec::new();
    };
    let le = |bound: i64| Constraint(bound_expr(var, bound), Relation::Le);
    let ge = |bound: i64| Constraint(bound_expr(var, bound), Relation::Ge);
    match name {
        "bool" => vec![ge(0), le(1)],
        "u8" => vec![ge(0), le(i64::from(u8::MAX))],
        "u16" => vec![ge(0), le(i64::from(u16::MAX))],
        "u32" => vec![ge(0), le(i64::from(u32::MAX))],
        // u64/u128/usize upper bounds exceed the i64 coefficient range:
        // keep only the non-negative half. Sound (weaker premises).
        "u64" | "u128" | "usize" => vec![ge(0)],
        "i8" => vec![ge(i64::from(i8::MIN)), le(i64::from(i8::MAX))],
        "i16" => vec![ge(i64::from(i16::MIN)), le(i64::from(i16::MAX))],
        "i32" => vec![ge(i64::from(i32::MIN)), le(i64::from(i32::MAX))],
        // `ge(i64::MIN)` would negate `i64::MIN` (overflow). Every i64 already
        // satisfies `>= i64::MIN`, so the lower bound is dropped without loss.
        "i64" => vec![le(i64::MAX)],
        "i128" | "isize" => Vec::new(),
        "f32" | "f64" => Vec::new(),
        _ => Vec::new(),
    }
}

/// True when `ty` is an unsigned primitive integer. `%` is sound only for these: a
/// signed remainder takes the sign of the dividend, which the remainder encoding
/// does not model.
pub fn is_unsigned(ty: &Type) -> bool {
    matches!(
        primitive_name(ty),
        Some("u8" | "u16" | "u32" | "u64" | "u128" | "usize")
    )
}

/// True when `ty` is one of Rust's primitive integer types.
///
/// Only these may have their runtime predicate widened to `i128` (see
/// `expand.rs`); widening a float or a custom type would change the
/// predicate's arithmetic, so those keep evaluating in the inner type.
pub fn is_supported_integer(ty: &Type) -> bool {
    matches!(
        primitive_name(ty),
        Some(
            "u8" | "u16"
                | "u32"
                | "u64"
                | "u128"
                | "usize"
                | "i8"
                | "i16"
                | "i32"
                | "i64"
                | "i128"
                | "isize"
        )
    )
}

#[cfg(test)]
mod tests {
    use syn::parse_quote;

    use super::*;

    #[test]
    fn u8_domain() {
        let ty: Type = parse_quote!(u8);
        let dom = domain_constraints(&ty, "value");
        assert_eq!(dom.len(), 2);
    }

    #[test]
    fn u64_only_lower_bound() {
        let ty: Type = parse_quote!(u64);
        let dom = domain_constraints(&ty, "value");
        assert_eq!(dom.len(), 1);
        assert_eq!(dom[0].1, Relation::Ge);
    }

    #[test]
    fn unknown_type_empty() {
        let ty: Type = parse_quote!(custom::Crate::Thing);
        assert!(domain_constraints(&ty, "value").is_empty());
    }

    #[test]
    fn only_bare_primitive_names_resolve() {
        assert_eq!(primitive_name(&parse_quote!(u8)), Some("u8"));
        assert_eq!(primitive_name(&parse_quote!(f64)), Some("f64"));
        // A qualified path or a generic could be a user type of the same name.
        assert_eq!(primitive_name(&parse_quote!(std::primitive::u8)), None);
        assert_eq!(primitive_name(&parse_quote!(Vec<u8>)), None);
        assert_eq!(primitive_name(&parse_quote!(Byte)), None);
    }

    #[test]
    fn alias_gets_no_domain_and_no_widening() {
        let ty: Type = parse_quote!(Byte);
        assert!(domain_constraints(&ty, "value").is_empty());
        assert!(!is_supported_integer(&ty));
    }
}
