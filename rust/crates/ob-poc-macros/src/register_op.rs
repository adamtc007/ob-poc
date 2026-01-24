//! Implementation of #[register_custom_op] attribute macro

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{parse_macro_input, Attribute, ItemStruct};

pub fn register_custom_op_impl(input: TokenStream) -> TokenStream {
    let input_struct = parse_macro_input!(input as ItemStruct);

    let struct_name = &input_struct.ident;

    // Validate: must be a unit struct (no fields)
    if !matches!(input_struct.fields, syn::Fields::Unit) {
        return syn::Error::new_spanned(
            &input_struct.fields,
            "#[register_custom_op] only works on unit structs",
        )
        .to_compile_error()
        .into();
    }

    // Extract #[cfg(...)] and #[cfg_attr(...)] attributes
    // These must be applied to ALL generated items
    let cfg_attrs: Vec<&Attribute> = input_struct
        .attrs
        .iter()
        .filter(|a| a.path().is_ident("cfg") || a.path().is_ident("cfg_attr"))
        .collect();

    // Generate deterministic factory function name
    let factory_fn_name = format_ident!("__obpoc_factory_{}", struct_name);

    // Re-emit the ORIGINAL struct unchanged (preserves doc attrs, derives, etc.)
    let original_struct = &input_struct;

    let expanded = quote! {
        // Emit the original struct EXACTLY as parsed (preserves doc attrs, derives, etc.)
        #original_struct

        // Hidden factory function — MUST have same cfg attrs
        #(#cfg_attrs)*
        #[doc(hidden)]
        #[allow(non_snake_case)]
        fn #factory_fn_name() -> ::std::sync::Arc<dyn crate::domain_ops::CustomOperation> {
            ::std::sync::Arc::new(#struct_name)
        }

        // Auto-register with inventory — MUST have same cfg attrs
        #(#cfg_attrs)*
        ::inventory::submit! {
            crate::domain_ops::CustomOpFactory {
                create: #factory_fn_name
            }
        }
    };

    TokenStream::from(expanded)
}
