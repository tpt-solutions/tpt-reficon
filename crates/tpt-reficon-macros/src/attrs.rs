//! Parsing of the `#[requires]`, `#[ensures]`, and `#[refine(...)]` helper
//! attributes.

use syn::spanned::Spanned;
use syn::{Attribute, Expr, Ident, LitStr};

/// The configuration a `#[derive(Refine)]` understands.
pub struct RefineConfig {
    /// Predicate expressions (from `requires` and `ensures`), in source order.
    pub predicates: Vec<Expr>,
    /// The variable name used for the inner value inside predicates.
    pub variable: String,
    /// Whether `#[refine(serde)]` asked for a `Deserialize` impl.
    pub serde: bool,
}

/// Parses all helper attributes present on the struct.
pub fn parse_config(
    attrs: &[Attribute],
    default_field_variable: Option<&Ident>,
) -> syn::Result<RefineConfig> {
    let mut predicates = Vec::new();
    let mut variable: Option<String> = None;
    let mut serde = false;

    for attr in attrs {
        if attr.path().is_ident("requires") || attr.path().is_ident("ensures") {
            let expr: Expr = attr.parse_args()?;
            predicates.push(expr);
        } else if attr.path().is_ident("refine") {
            let items = attr.parse_args_with(
                syn::punctuated::Punctuated::<RefineItem, syn::Token![,]>::parse_terminated,
            )?;
            for item in items {
                match item {
                    RefineItem::Variable { value } => {
                        if variable.replace(value.value()).is_some() {
                            return Err(syn::Error::new(
                                attr.span(),
                                "`refine(variable = ...)` may only be specified once",
                            ));
                        }
                    }
                    RefineItem::Serde => {
                        if serde {
                            return Err(syn::Error::new(
                                attr.span(),
                                "`refine(serde)` may only be specified once",
                            ));
                        }
                        serde = true;
                    }
                }
            }
        }
    }

    let variable = variable.unwrap_or_else(|| {
        default_field_variable.map_or_else(|| "value".to_string(), |id| id.to_string())
    });

    Ok(RefineConfig {
        predicates,
        variable,
        serde,
    })
}

/// One item inside `#[refine(...)]`: `variable = "name"` or the bare `serde`.
enum RefineItem {
    Variable { value: LitStr },
    Serde,
}

impl syn::parse::Parse for RefineItem {
    fn parse(input: syn::parse::ParseStream<'_>) -> syn::Result<Self> {
        let name: Ident = input.parse()?;
        if name == "serde" {
            return Ok(RefineItem::Serde);
        }
        if name != "variable" {
            return Err(syn::Error::new(
                name.span(),
                "unknown `refine(...)` key; expected `variable` or `serde`",
            ));
        }
        input.parse::<syn::Token![=]>()?;
        let value: LitStr = input.parse()?;
        Ok(RefineItem::Variable { value })
    }
}
