#![deny(
    missing_debug_implementations,
    missing_copy_implementations,
    trivial_casts,
    trivial_numeric_casts,
    unsafe_code,
    unstable_features,
    unused_import_braces,
    unused_qualifications
)]
#![doc(html_root_url = "https://docs.rs/jsonapi/")]

//! This is documentation for the `jsonapi` crate.
//! The crate is meant to be used for serializing, deserializing and validating
//! [JSON:API] requests and responses.
//!
//! [JSON:API]: https://jsonapi.org/
//! [serde]: https://serde.rs
//! [JsonApiDocument]: api/struct.JsonApiDocument.html
//! [Resource]: api/struct.Resource.html
//! [jsonapi_model]: macro.jsonapi_model.html
//!
//! ## Examples
//!
//! ### Basic Usage with Macro
//!
//! Using the [`jsonapi_model!`][jsonapi_model] macro a struct can be converted
//! into a [`JsonApiDocument`][JsonApiDocument] or [`Resource`][Resource]. It is
//! required that the struct have an `id` property whose type is `String`. The
//! second argument in the [`jsonapi_model!`][jsonapi_model] macro defines the
//! `type` member as required by the [JSON:API] specification
//!
//! ```rust
//! #[macro_use] extern crate jsonapi;
//! use jsonapi::api::*;
//! use jsonapi::model::*;
//! use serde::{Deserialize, Serialize};
//!
//! #[derive(Debug, PartialEq, Serialize, Deserialize)]
//! struct Flea {
//!     id: String,
//!     name: String,
//! }
//!
//! jsonapi_model!(Flea; "flea");
//!
//! let example_flea = Flea {
//!     id: "123".into(),
//!     name: "Mr.Flea".into(),
//! };
//!
//! // Convert into a `JsonApiDocument`
//! let doc = example_flea.to_jsonapi_document();
//! assert!(doc.is_valid());
//! ```
//!
//! ### Deserializing a JSON:API Document
//!
//! Deserialize a JSON:API document using [serde] by explicitly declaring the
//! variable type in `Result`
//!
//! ```rust
//! use jsonapi::api::Resource;
//!
//! let serialized = r#"{
//!   "id": "1",
//!   "type": "articles",
//!   "attributes": {
//!     "title": "JSON:API paints my bikeshed!",
//!     "body": "The shortest article. Ever."
//!   }
//! }"#;
//! let data: Result<Resource, serde_json::Error> = serde_json::from_str(serialized);
//! assert!(data.is_ok());
//! ```
//!
//! [`JsonApiDocument`][JsonApiDocument] implements `PartialEq` which allows two
//! documents to be compared for equality. If two documents possess the **same
//! contents** the ordering of the attributes and fields within the JSON:API
//! document are irrelevant and their equality will be `true`.
//!
//! ## Testing
//!
//! Run the tests:
//!
//! ```text
//! cargo test
//! ```
//!
//! Run tests with more verbose output:
//!
//! ```text
//! RUST_BACKTRACE=1 cargo test -- --nocapture
//! ```

pub mod api;
pub mod array;
pub mod errors;
pub mod model;
pub mod query;

#[cfg(feature = "derive")]
pub use jsonapi_derive::JsonApiModel;
