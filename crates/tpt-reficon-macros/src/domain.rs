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

/// Computes the domain constraints for `ty` over the variable `var`.
///
/// Unknown types yield an empty domain (no elision, always a runtime check).
pub fn domain_constraints(ty: &Type, var: &str) -> Vec<Constraint> {
    let name = match last_segment_ident(ty) {
        Some(n) => n,
        None => return Vec::new(),
    };
    let le = |bound: i64| {
        Constraint(
            Linear::var(var).add(&Linear::constant_only(-bound)),
            Relation::Le,
        )
    };
    let ge = |bound: i64| {
        Constraint(
            Linear::var(var).add(&Linear::constant_only(-bound)),
            Relation::Ge,
        )
    };
    match name.as_str() {
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

fn last_segment_ident(ty: &Type) -> Option<String> {
    let path = match ty {
        Type::Path(p) if p.qself.is_none() => &p.path,
        _ => return None,
    };
    path.segments.last().map(|s| s.ident.to_string())
}

/// True when `ty` is one of Rust's primitive integer types.
///
/// Only these may have their runtime predicate widened to `i128` (see
/// `expand.rs`); widening a float or a custom type would change the
/// predicate's arithmetic, so those keep evaluating in the inner type.
pub fn is_supported_integer(ty: &Type) -> bool {
    matches!(
        last_segment_ident(ty).as_deref(),
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
}
