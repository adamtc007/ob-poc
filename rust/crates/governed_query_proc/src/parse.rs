//! Parsing of `#[governed_query(...)]` attribute arguments.

use syn::parse::{Parse, ParseStream};
use syn::{Expr, Lit, Token};

/// Parsed arguments from `#[governed_query(verb = "cbu.create", ...)]`.
#[derive(Debug)]
pub struct GovernedQueryArgs {
    /// Required: the verb FQN (e.g., "cbu.create")
    pub verb: String,
    /// Optional: referenced attribute FQNs
    pub attrs: Vec<String>,
    /// Optional: explicit PII authorization
    pub allow_pii: bool,
    /// Optional: skip Principal parameter check
    pub skip_principal_check: bool,
}

impl Parse for GovernedQueryArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut verb: Option<String> = None;
        let mut attrs: Vec<String> = Vec::new();
        let mut allow_pii = false;
        let mut skip_principal_check = false;

        while !input.is_empty() {
            let key: syn::Ident = input.parse()?;
            input.parse::<Token![=]>()?;

            match key.to_string().as_str() {
                "verb" => {
                    let lit: Lit = input.parse()?;
                    if let Lit::Str(s) = lit {
                        verb = Some(s.value());
                    } else {
                        return Err(syn::Error::new_spanned(
                            lit,
                            "governed_query: `verb` must be a string literal",
                        ));
                    }
                }
                "attrs" => {
                    // Parse as array: ["attr1", "attr2"]
                    let content;
                    syn::bracketed!(content in input);
                    while !content.is_empty() {
                        let lit: Lit = content.parse()?;
                        if let Lit::Str(s) = lit {
                            attrs.push(s.value());
                        } else {
                            return Err(syn::Error::new_spanned(
                                lit,
                                "governed_query: `attrs` entries must be string literals",
                            ));
                        }
                        if !content.is_empty() {
                            content.parse::<Token![,]>()?;
                        }
                    }
                }
                "allow_pii" => {
                    let lit: Expr = input.parse()?;
                    if let Expr::Lit(expr_lit) = &lit {
                        if let Lit::Bool(b) = &expr_lit.lit {
                            allow_pii = b.value;
                        } else {
                            return Err(syn::Error::new_spanned(
                                lit,
                                "governed_query: `allow_pii` must be a boolean",
                            ));
                        }
                    } else {
                        return Err(syn::Error::new_spanned(
                            lit,
                            "governed_query: `allow_pii` must be a boolean literal",
                        ));
                    }
                }
                "skip_principal_check" => {
                    let lit: Expr = input.parse()?;
                    if let Expr::Lit(expr_lit) = &lit {
                        if let Lit::Bool(b) = &expr_lit.lit {
                            skip_principal_check = b.value;
                        } else {
                            return Err(syn::Error::new_spanned(
                                lit,
                                "governed_query: `skip_principal_check` must be a boolean",
                            ));
                        }
                    } else {
                        return Err(syn::Error::new_spanned(
                            lit,
                            "governed_query: `skip_principal_check` must be a boolean literal",
                        ));
                    }
                }
                other => {
                    return Err(syn::Error::new(
                        key.span(),
                        format!("governed_query: unknown attribute `{other}`"),
                    ));
                }
            }

            // Optional trailing comma
            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
        }

        let verb = verb.ok_or_else(|| {
            syn::Error::new(
                proc_macro2::Span::call_site(),
                "governed_query: `verb` argument is required",
            )
        })?;

        Ok(GovernedQueryArgs {
            verb,
            attrs,
            allow_pii,
            skip_principal_check,
        })
    }
}
