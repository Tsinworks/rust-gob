extern crate proc_macro;
#[macro_use]
extern crate quote;
extern crate serde_derive_internals;
extern crate syn;

use std::borrow::Borrow;

use serde_derive_internals::{ast, Ctxt};
use syn::DeriveInput;

mod derive_enum;
mod derive_struct;

#[proc_macro_derive(GobSerialize, attributes(gob))]
pub fn derive_gob_serialize(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input: DeriveInput = syn::parse(input).unwrap();

    let cx = Ctxt::new();
    let container = ast::Container::from_ast(&cx, &input, serde_derive_internals::Derive::Serialize).unwrap();

    let interpret_as = get_interpret_as(&input.attrs);

    let inner_impl = if let Some(interpret_as_str) = interpret_as {
        if interpret_as_str == "map[interface{}]interface{}" {
            quote!{
                ::gob::Schema::register_type(schema,
                    ::gob::types::Type::build()
                        .map_type(
                            <S::TypeId as ::gob::types::TypeId>::INTERFACE,
                            <S::TypeId as ::gob::types::TypeId>::INTERFACE
                        ))
            }
        } else {
             // Fallback or error?
             // For now we only support map[interface{}]interface{} as requested.
             // If we want to support others, we'd need parsing.
             // Let's error to be safe.
             panic!("Unsupported interpret_as value: {}", interpret_as_str);
        }
    } else {
        match container.data {
            ast::Data::Enum(variants) => derive_enum::derive_enum(variants, &container.attrs),
            ast::Data::Struct(style, fields) => {
                derive_struct::derive_struct(style, fields, &container.attrs)
            }
        }
    };

    let ident = container.ident;
    let (impl_generics, ty_generics, where_clause) = container.generics.split_for_impl();

    let expanded = quote!{
        impl #impl_generics ::gob::GobSerialize for #ident #ty_generics #where_clause {
            fn schema_register<S>(schema: &mut S) -> std::result::Result<S::TypeId, S::Error>
                where S: ::gob::Schema
            {
                #inner_impl
            }
        }
    };

    cx.check().unwrap();

    expanded.into()
}

fn get_interpret_as(attrs: &[syn::Attribute]) -> Option<String> {
    for attr in attrs {
        if attr.path().is_ident("gob") {
            let mut res = None;
            let _ = attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("interpret_as") {
                    let value = meta.value()?;
                    let s: syn::LitStr = value.parse()?;
                    res = Some(s.value());
                    Ok(())
                } else {
                    Ok(())
                }
            });
            if res.is_some() {
                return res;
            }
        }
    }
    None
}

fn variant_field_type_variable(variant_idx: usize, field_idx: usize) -> syn::Ident {
    syn::Ident::new(&format!("type_id_{}_{}", variant_idx, field_idx), proc_macro2::Span::call_site())
}

fn derive_register_field_types<'a, I>(variant_idx: usize, fields: I) -> proc_macro2::TokenStream
where
    I: IntoIterator,
    I::Item: Borrow<ast::Field<'a>>,
{
    let mut expanded = quote!{};
    for (field_idx, field_item) in fields.into_iter().enumerate() {
        let field = field_item.borrow();
        let field_type = &field.ty;
        let type_id_ident = variant_field_type_variable(variant_idx, field_idx);
        expanded.extend(quote!{
            let #type_id_ident =
                <#field_type as ::gob::GobSerialize>::schema_register(schema)?;
        });
    }
    expanded
}

fn derive_field<'a>(variant_idx: usize, field_idx: usize, field: &ast::Field<'a>) -> proc_macro2::TokenStream {
    let type_id_ident = variant_field_type_variable(variant_idx, field_idx);
    let field_name = field.attrs.name().serialize_name();
    quote!{
        .field(#field_name, #type_id_ident)
    }
}

fn derive_element<'a>(variant_idx: usize, element_idx: usize) -> proc_macro2::TokenStream {
    let type_id_ident = variant_field_type_variable(variant_idx, element_idx);
    quote!{
        .element(#type_id_ident)
    }
}