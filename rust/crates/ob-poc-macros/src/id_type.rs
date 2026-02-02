//! Implementation of #[derive(IdType)] macro for UUID newtypes

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields};

pub fn derive_id_type_impl(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    // Parse attributes: #[id(prefix = "...", new_v4)]
    let (prefix, generate_new) = parse_id_attrs(&input.attrs);

    // Validate: must be tuple struct with single field
    let inner_type = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {
                &fields.unnamed.first().unwrap().ty
            }
            _ => {
                return syn::Error::new_spanned(
                    &input,
                    "IdType requires a tuple struct with exactly one field: struct MyId(Uuid)",
                )
                .to_compile_error()
                .into();
            }
        },
        _ => {
            return syn::Error::new_spanned(&input, "IdType only works on tuple structs")
                .to_compile_error()
                .into();
        }
    };

    // Generate new() + Default if requested
    let new_impl = if generate_new {
        quote! {
            impl #name {
                pub fn new() -> Self { Self(::uuid::Uuid::now_v7()) }
            }

            impl Default for #name {
                fn default() -> Self { Self::new() }
            }
        }
    } else {
        quote! {}
    };

    // Display format depends on prefix
    let display_impl = if let Some(ref pfx) = prefix {
        quote! {
            impl ::std::fmt::Display for #name {
                fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                    write!(f, "{}_{}", #pfx, self.0)
                }
            }
        }
    } else {
        quote! {
            impl ::std::fmt::Display for #name {
                fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                    write!(f, "{}", self.0)
                }
            }
        }
    };

    // FromStr handles prefix stripping
    let from_str_impl = if let Some(ref pfx) = prefix {
        let pfx_underscore = format!("{}_", pfx);
        quote! {
            impl ::std::str::FromStr for #name {
                type Err = ::uuid::Error;
                fn from_str(s: &str) -> Result<Self, Self::Err> {
                    let uuid_str = s.strip_prefix(#pfx_underscore).unwrap_or(s);
                    Ok(Self(::uuid::Uuid::parse_str(uuid_str)?))
                }
            }
        }
    } else {
        quote! {
            impl ::std::str::FromStr for #name {
                type Err = ::uuid::Error;
                fn from_str(s: &str) -> Result<Self, Self::Err> {
                    Ok(Self(::uuid::Uuid::parse_str(s)?))
                }
            }
        }
    };

    let expanded = quote! {
        impl #name {
            pub fn from_uuid(id: #inner_type) -> Self { Self(id) }
            // Return by value (Uuid is Copy) for API compatibility
            pub fn as_uuid(&self) -> #inner_type { self.0 }
        }

        #new_impl
        #display_impl
        #from_str_impl

        impl From<#inner_type> for #name {
            fn from(id: #inner_type) -> Self { Self(id) }
        }

        impl From<#name> for #inner_type {
            fn from(id: #name) -> Self { id.0 }
        }

        impl Clone for #name {
            fn clone(&self) -> Self { Self(self.0) }
        }

        impl Copy for #name {}

        impl PartialEq for #name {
            fn eq(&self, other: &Self) -> bool { self.0 == other.0 }
        }

        impl Eq for #name {}

        impl ::std::hash::Hash for #name {
            fn hash<H: ::std::hash::Hasher>(&self, state: &mut H) {
                self.0.hash(state);
            }
        }

        impl ::std::fmt::Debug for #name {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                write!(f, "{}({})", stringify!(#name), self.0)
            }
        }

        impl ::serde::Serialize for #name {
            fn serialize<S: ::serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.serialize_str(&self.to_string())
            }
        }

        impl<'de> ::serde::Deserialize<'de> for #name {
            fn deserialize<D: ::serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                // Fully-qualified call to avoid trait-not-in-scope error
                let s = <::std::string::String as ::serde::Deserialize>::deserialize(deserializer)?;
                s.parse().map_err(::serde::de::Error::custom)
            }
        }

        // SQLx traits - only when database feature is enabled
        // Use UFCS for trait method calls to avoid resolution issues
        #[cfg(feature = "database")]
        impl ::sqlx::Type<::sqlx::Postgres> for #name {
            fn type_info() -> ::sqlx::postgres::PgTypeInfo {
                <#inner_type as ::sqlx::Type<::sqlx::Postgres>>::type_info()
            }
        }

        #[cfg(feature = "database")]
        impl<'q> ::sqlx::Encode<'q, ::sqlx::Postgres> for #name {
            fn encode_by_ref(
                &self,
                buf: &mut ::sqlx::postgres::PgArgumentBuffer
            ) -> ::std::result::Result<::sqlx::encode::IsNull, ::sqlx::error::BoxDynError> {
                <#inner_type as ::sqlx::Encode<'q, ::sqlx::Postgres>>::encode_by_ref(&self.0, buf)
            }
        }

        #[cfg(feature = "database")]
        impl<'r> ::sqlx::Decode<'r, ::sqlx::Postgres> for #name {
            fn decode(
                value: ::sqlx::postgres::PgValueRef<'r>
            ) -> Result<Self, ::sqlx::error::BoxDynError> {
                Ok(Self(<#inner_type as ::sqlx::Decode<'r, ::sqlx::Postgres>>::decode(value)?))
            }
        }
    };

    TokenStream::from(expanded)
}

fn parse_id_attrs(attrs: &[syn::Attribute]) -> (Option<String>, bool) {
    let mut prefix = None;
    let mut new_v4 = false;

    for attr in attrs {
        if attr.path().is_ident("id") {
            let _ = attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("prefix") {
                    let value: syn::LitStr = meta.value()?.parse()?;
                    prefix = Some(value.value());
                } else if meta.path.is_ident("new_v4") {
                    new_v4 = true;
                }
                Ok(())
            });
        }
    }

    (prefix, new_v4)
}
