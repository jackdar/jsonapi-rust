//! Proc-macro support for the `jsonapi` crate.
//!
//! This crate is not meant to be used directly. Enable the `derive` feature on
//! the `jsonapi` crate and use `#[derive(JsonApiModel)]` instead.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields, LitStr};

/// Derives [`JsonApiModel`](../jsonapi/model/trait.JsonApiModel.html) for a struct.
///
/// The struct **must** have an `id: String` field (or any type whose `to_string()`
/// returns the desired JSON:API id).
///
/// # Struct-level attribute
///
/// | Attribute | Description |
/// |-----------|-------------|
/// | `#[jsonapi(resource_type = "articles")]` | Sets the JSON:API resource `type`. Defaults to the lowercase struct name if omitted. |
///
/// # Field-level attributes
///
/// | Attribute | Description |
/// |-----------|-------------|
/// | `#[jsonapi(skip)]` | Exclude this field from JSON:API attributes entirely. |
/// | `#[jsonapi(rename = "foo")]` | Emit this field as `"foo"` in JSON:API attributes. |
/// | `#[jsonapi(has_one)]` | Treat this field as a has-one relationship (excluded from attributes). |
/// | `#[jsonapi(has_many)]` | Treat this field as a has-many relationship via [`JsonApiArray`](../jsonapi/array/trait.JsonApiArray.html). |
///
/// Fields with no `#[jsonapi(rename)]` always use the raw Rust field name in JSON:API
/// attributes, regardless of any `#[serde(rename)]` present on the field.
///
/// Attribute fields are emitted in struct declaration order, giving stable JSON output
/// across requests.
///
/// # Example
///
/// ```rust,ignore
/// use serde::{Deserialize, Serialize};
/// use jsonapi::JsonApiModel;
///
/// #[derive(Debug, Serialize, Deserialize, JsonApiModel)]
/// #[jsonapi(resource_type = "articles")]
/// struct Article {
///     id: String,
///     title: String,
///     #[serde(rename = "publishedAt")]
///     published_at: String,       // jsonapi key is "publishedAt" (inherits serde rename)
///     #[jsonapi(rename = "body")]
///     content: String,            // jsonapi key is "body", serde key is "content"
///     #[jsonapi(skip)]
///     internal_cache: String,     // not present in jsonapi output
/// }
/// ```
#[proc_macro_derive(JsonApiModel, attributes(jsonapi))]
pub fn derive_jsonapi_model(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    impl_jsonapi_model(input)
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

fn impl_jsonapi_model(input: DeriveInput) -> Result<TokenStream2, syn::Error> {
    let struct_name = &input.ident;

    let type_name = parse_struct_type(&input.attrs, struct_name)?;

    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => &fields.named,
            _ => {
                return Err(syn::Error::new_spanned(
                    struct_name,
                    "JsonApiModel only supports structs with named fields",
                ))
            }
        },
        _ => {
            return Err(syn::Error::new_spanned(
                struct_name,
                "JsonApiModel can only be derived for structs",
            ))
        }
    };

    let mut has_id = false;
    // (field ident, serde lookup key, jsonapi output key)
    let mut attribute_fields: Vec<(syn::Ident, String, String)> = vec![];
    let mut has_one_fields: Vec<syn::Ident> = vec![];
    let mut has_many_fields: Vec<syn::Ident> = vec![];

    for field in fields {
        let ident = field.ident.as_ref().unwrap();
        let field_name = ident.to_string();

        if field_name == "id" {
            has_id = true;
            continue;
        }

        let serde_name = get_serde_rename(field).unwrap_or_else(|| field_name.clone());
        let jsonapi_attrs = get_jsonapi_field_attrs(field)?;

        if jsonapi_attrs.skip {
            continue;
        }
        if jsonapi_attrs.has_one {
            has_one_fields.push(ident.clone());
            continue;
        }
        if jsonapi_attrs.has_many {
            has_many_fields.push(ident.clone());
            continue;
        }

        let jsonapi_name = jsonapi_attrs.rename.unwrap_or_else(|| field_name.clone());
        attribute_fields.push((ident.clone(), serde_name, jsonapi_name));
    }

    if !has_id {
        return Err(syn::Error::new_spanned(
            struct_name,
            "JsonApiModel requires an `id: String` field",
        ));
    }

    let rel_field_strs: Vec<String> = has_one_fields
        .iter()
        .chain(has_many_fields.iter())
        .map(|i| i.to_string())
        .collect();

    let relationship_fields_impl = if rel_field_strs.is_empty() {
        quote! {
            fn relationship_fields() -> Option<&'static [&'static str]> {
                None
            }
        }
    } else {
        quote! {
            fn relationship_fields() -> Option<&'static [&'static str]> {
                static FIELDS: &'static [&'static str] = &[#(#rel_field_strs),*];
                Some(FIELDS)
            }
        }
    };

    let build_relationships_impl = if has_one_fields.is_empty() && has_many_fields.is_empty() {
        quote! {
            fn build_relationships(&self) -> Option<::jsonapi::api::Relationships> {
                None
            }
        }
    } else {
        let has_one_inserts = has_one_fields.iter().map(|f| {
            let name = f.to_string();
            quote! {
                relationships.insert(#name.into(), Self::build_has_one(&self.#f));
            }
        });
        let has_many_inserts = has_many_fields.iter().map(|f| {
            let name = f.to_string();
            quote! {
                relationships.insert(#name.into(), {
                    let values = &self.#f.get_models();
                    Self::build_has_many(values)
                });
            }
        });
        quote! {
            fn build_relationships(&self) -> Option<::jsonapi::api::Relationships> {
                let mut relationships: ::jsonapi::api::Relationships = Default::default();
                #(#has_one_inserts)*
                #(#has_many_inserts)*
                Some(relationships)
            }
        }
    };

    let build_included_impl = if has_one_fields.is_empty() && has_many_fields.is_empty() {
        quote! {
            fn build_included(&self) -> Option<::jsonapi::api::Resources> {
                None
            }
        }
    } else {
        let has_one_appends = has_one_fields.iter().map(|f| {
            quote! {
                included.append(&mut self.#f.to_resources());
            }
        });
        let has_many_loops = has_many_fields.iter().map(|f| {
            quote! {
                for model in self.#f.get_models() {
                    included.append(&mut model.to_resources());
                }
            }
        });
        quote! {
            fn build_included(&self) -> Option<::jsonapi::api::Resources> {
                let mut included: ::jsonapi::api::Resources = vec![];
                #(#has_one_appends)*
                #(#has_many_loops)*
                Some(included)
            }
        }
    };

    let attr_inserts = attribute_fields
        .iter()
        .map(|(_ident, serde_name, jsonapi_name)| {
            quote! {
                if let Some(val) = _obj.get(#serde_name) {
                    attributes.insert(#jsonapi_name.to_string(), val.clone());
                }
            }
        });

    let expanded = quote! {
        impl ::jsonapi::model::JsonApiModel for #struct_name {
            fn jsonapi_type(&self) -> String {
                #type_name.to_string()
            }

            fn jsonapi_id(&self) -> String {
                self.id.to_string()
            }

            #relationship_fields_impl
            #build_relationships_impl
            #build_included_impl

            fn to_jsonapi_resource(&self) -> (::jsonapi::api::Resource, Option<::jsonapi::api::Resources>) {
                let _serialized = ::serde_json::to_value(self).unwrap();
                let _obj = _serialized.as_object().unwrap();

                let mut attributes: ::jsonapi::api::ResourceAttributes = Default::default();
                #(#attr_inserts)*

                let resource = ::jsonapi::api::Resource {
                    _type: self.jsonapi_type(),
                    id: self.jsonapi_id(),
                    relationships: self.build_relationships(),
                    attributes,
                    ..Default::default()
                };
                (resource, self.build_included())
            }
        }
    };

    Ok(expanded)
}

fn parse_struct_type(attrs: &[syn::Attribute], ident: &syn::Ident) -> Result<String, syn::Error> {
    for attr in attrs {
        if !attr.path().is_ident("jsonapi") {
            continue;
        }
        let mut found_type: Option<String> = None;
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("resource_type") {
                let value = meta.value()?;
                let s: LitStr = value.parse()?;
                found_type = Some(s.value());
                Ok(())
            } else {
                // ignore unknown struct-level attrs silently
                Ok(())
            }
        })?;
        if let Some(t) = found_type {
            return Ok(t);
        }
    }
    // Default: lowercase struct name
    Ok(ident.to_string().to_lowercase())
}

struct JsonApiFieldAttrs {
    skip: bool,
    rename: Option<String>,
    has_one: bool,
    has_many: bool,
}

fn get_jsonapi_field_attrs(field: &syn::Field) -> Result<JsonApiFieldAttrs, syn::Error> {
    let mut result = JsonApiFieldAttrs {
        skip: false,
        rename: None,
        has_one: false,
        has_many: false,
    };

    for attr in &field.attrs {
        if !attr.path().is_ident("jsonapi") {
            continue;
        }
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("skip") {
                result.skip = true;
            } else if meta.path.is_ident("rename") {
                let value = meta.value()?;
                let s: LitStr = value.parse()?;
                result.rename = Some(s.value());
            } else if meta.path.is_ident("has_one") {
                result.has_one = true;
            } else if meta.path.is_ident("has_many") {
                result.has_many = true;
            } else {
                return Err(meta.error(
                    "unknown jsonapi field attribute; expected skip, rename, has_one, or has_many",
                ));
            }
            Ok(())
        })?;
    }

    Ok(result)
}

fn get_serde_rename(field: &syn::Field) -> Option<String> {
    for attr in &field.attrs {
        if !attr.path().is_ident("serde") {
            continue;
        }
        let mut rename: Option<String> = None;
        let _ = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("rename") {
                let value = meta.value()?;
                let s: LitStr = value.parse()?;
                rename = Some(s.value());
            }
            Ok(())
        });
        if rename.is_some() {
            return rename;
        }
    }
    None
}
