/// Tests for the `#[derive(JsonApiModel)]` proc macro and IndexMap-backed
/// attribute ordering. Requires the `derive` feature.
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate jsonapi;

use jsonapi::JsonApiModel;
use jsonapi::model::*;
use jsonapi::array::JsonApiArray;

// ---------------------------------------------------------------------------
// Test structs
// ---------------------------------------------------------------------------

/// Basic struct — no relationships, no directives.
/// The JSON:API type defaults to the lowercase struct name ("article").
#[derive(Debug, PartialEq, Serialize, Deserialize, JsonApiModel)]
struct Article {
    id: String,
    title: String,
    body: String,
    views: u32,
}

/// Struct with an explicit type override.
#[derive(Debug, PartialEq, Serialize, Deserialize, JsonApiModel)]
#[jsonapi(type = "blog-posts")]
struct BlogPost {
    id: String,
    title: String,
}

/// Struct exercising #[jsonapi(skip)] and #[jsonapi(rename)].
#[derive(Debug, PartialEq, Serialize, Deserialize, JsonApiModel)]
#[jsonapi(type = "people")]
struct Person {
    id: String,
    name: String,
    #[jsonapi(rename = "emailAddress")]
    email: String,
    #[jsonapi(skip)]
    password_hash: String,
}

/// Struct where a serde rename should flow through to jsonapi with no
/// explicit #[jsonapi(rename)] needed.
#[derive(Debug, PartialEq, Serialize, Deserialize, JsonApiModel)]
#[jsonapi(type = "events")]
struct Event {
    id: String,
    #[serde(rename = "eventName")]
    name: String,
    #[serde(rename = "startedAt")]
    started_at: String,
}

/// Struct with both a serde rename AND a jsonapi rename — jsonapi wins.
#[derive(Debug, PartialEq, Serialize, Deserialize, JsonApiModel)]
#[jsonapi(type = "products")]
struct Product {
    id: String,
    #[serde(rename = "productName")]
    #[jsonapi(rename = "name")]
    title: String,
}

/// Supporting types for relationship tests.
#[derive(Debug, PartialEq, Serialize, Deserialize, JsonApiModel)]
struct Tag {
    id: String,
    label: String,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, JsonApiModel)]
#[jsonapi(type = "authors")]
struct Author {
    id: String,
    name: String,
}

/// Struct with has_one and has_many relationships.
#[derive(Debug, PartialEq, Serialize, Deserialize, JsonApiModel)]
#[jsonapi(type = "posts")]
struct Post {
    id: String,
    title: String,
    #[jsonapi(has_one)]
    author: Author,
    #[jsonapi(has_many)]
    tags: Vec<Tag>,
}

// ---------------------------------------------------------------------------
// jsonapi_type / jsonapi_id
// ---------------------------------------------------------------------------

#[test]
fn derive_default_type_is_lowercase_struct_name() {
    let a = Article { id: "1".into(), title: "t".into(), body: "b".into(), views: 0 };
    assert_eq!(a.jsonapi_type(), "article");
}

#[test]
fn derive_explicit_type_override() {
    let p = BlogPost { id: "1".into(), title: "t".into() };
    assert_eq!(p.jsonapi_type(), "blog-posts");
}

#[test]
fn derive_jsonapi_id() {
    let a = Article { id: "42".into(), title: "t".into(), body: "b".into(), views: 0 };
    assert_eq!(a.jsonapi_id(), "42");
}

// ---------------------------------------------------------------------------
// Attribute presence and absence
// ---------------------------------------------------------------------------

#[test]
fn derive_skip_excludes_field_from_attributes() {
    let p = Person {
        id: "1".into(),
        name: "Alice".into(),
        email: "a@example.com".into(),
        password_hash: "secret".into(),
    };
    let (resource, _) = p.to_jsonapi_resource();
    assert!(
        resource.get_attribute("password_hash").is_none(),
        "skipped field must not appear in attributes"
    );
    assert!(
        resource.get_attribute("name").is_some(),
        "non-skipped field must appear in attributes"
    );
}

#[test]
fn derive_jsonapi_rename_changes_attribute_key() {
    let p = Person {
        id: "1".into(),
        name: "Alice".into(),
        email: "a@example.com".into(),
        password_hash: "secret".into(),
    };
    let (resource, _) = p.to_jsonapi_resource();
    assert!(
        resource.get_attribute("emailAddress").is_some(),
        "#[jsonapi(rename)] key must appear"
    );
    assert!(
        resource.get_attribute("email").is_none(),
        "original field name must not appear when renamed"
    );
    assert_eq!(
        resource.get_attribute("emailAddress").unwrap(),
        "a@example.com"
    );
}

#[test]
fn derive_serde_rename_does_not_affect_jsonapi_key() {
    let e = Event {
        id: "1".into(),
        name: "RustConf".into(),
        started_at: "2024-09-12".into(),
    };
    let (resource, _) = e.to_jsonapi_resource();
    // serde renames to "eventName" / "startedAt", but without #[jsonapi(rename)]
    // the raw field name is used in jsonapi attributes
    assert!(
        resource.get_attribute("name").is_some(),
        "raw field name must be used as jsonapi key even when serde has renamed it"
    );
    assert!(
        resource.get_attribute("eventName").is_none(),
        "serde rename must not bleed into jsonapi key"
    );
    assert!(
        resource.get_attribute("started_at").is_some(),
        "raw field name must be used as jsonapi key even when serde has renamed it"
    );
    assert!(
        resource.get_attribute("startedAt").is_none(),
        "serde rename must not bleed into jsonapi key"
    );
}

#[test]
fn derive_jsonapi_rename_beats_serde_rename() {
    let p = Product { id: "1".into(), title: "Widget".into() };
    let (resource, _) = p.to_jsonapi_resource();
    // serde name is "productName", jsonapi rename is "name" — jsonapi wins
    assert!(
        resource.get_attribute("name").is_some(),
        "#[jsonapi(rename)] must take precedence over #[serde(rename)]"
    );
    assert!(resource.get_attribute("productName").is_none());
    assert!(resource.get_attribute("title").is_none());
}

// ---------------------------------------------------------------------------
// Attribute ordering
// ---------------------------------------------------------------------------

#[test]
fn derive_attribute_order_matches_struct_declaration() {
    let a = Article {
        id: "1".into(),
        title: "Order Test".into(),
        body: "content".into(),
        views: 7,
    };
    let (resource, _) = a.to_jsonapi_resource();
    let keys: Vec<&str> = resource.attributes.keys().map(String::as_str).collect();
    assert_eq!(
        keys,
        vec!["title", "body", "views"],
        "attributes must appear in struct field declaration order"
    );
}

#[test]
fn indexmap_attribute_order_is_stable_across_serializations() {
    let a = Article {
        id: "1".into(),
        title: "Stability".into(),
        body: "body".into(),
        views: 3,
    };
    let first_keys: Vec<String> = a.to_jsonapi_resource().0.attributes.keys().cloned().collect();
    let second_keys: Vec<String> = a.to_jsonapi_resource().0.attributes.keys().cloned().collect();
    assert_eq!(first_keys, second_keys);
}

// ---------------------------------------------------------------------------
// Relationships
// ---------------------------------------------------------------------------

#[test]
fn derive_has_one_builds_relationship() {
    let post = Post {
        id: "1".into(),
        title: "Hello".into(),
        author: Author { id: "99".into(), name: "Alice".into() },
        tags: vec![],
    };
    let (resource, _) = post.to_jsonapi_resource();

    // author must be a relationship, not an attribute
    assert!(resource.get_attribute("author").is_none());
    let rel = resource.get_relationship("author").expect("author relationship must exist");
    match &rel.data {
        Some(jsonapi::api::IdentifierData::Single(id)) => {
            assert_eq!(id.id, "99");
            assert_eq!(id._type, "authors");
        }
        _ => panic!("expected a single relationship identifier"),
    }
}

#[test]
fn derive_has_many_builds_relationship() {
    let post = Post {
        id: "1".into(),
        title: "Hello".into(),
        author: Author { id: "1".into(), name: "Bob".into() },
        tags: vec![
            Tag { id: "10".into(), label: "rust".into() },
            Tag { id: "11".into(), label: "webdev".into() },
        ],
    };
    let (resource, _) = post.to_jsonapi_resource();

    assert!(resource.get_attribute("tags").is_none());
    let rel = resource.get_relationship("tags").expect("tags relationship must exist");
    match &rel.data {
        Some(jsonapi::api::IdentifierData::Multiple(ids)) => {
            assert_eq!(ids.len(), 2);
            assert_eq!(ids[0].id, "10");
            assert_eq!(ids[1].id, "11");
        }
        _ => panic!("expected multiple relationship identifiers"),
    }
}

#[test]
fn derive_relationship_fields_returns_relationship_names() {
    let fields = Post::relationship_fields().expect("Post must have relationship fields");
    assert!(fields.contains(&"author"));
    assert!(fields.contains(&"tags"));
}

// ---------------------------------------------------------------------------
// Round-trip structs
//
// For a full to_jsonapi_document -> from_jsonapi_document round-trip to work:
//   - A `#[jsonapi(rename = "x")]` field must also have `#[serde(rename = "x")]`
//     because from_jsonapi_document deserialises through serde.
//   - A `#[jsonapi(skip)]` field must have `#[serde(default)]` (or be Option<T>)
//     so serde can reconstruct the struct when the key is absent.
// ---------------------------------------------------------------------------

/// A struct designed for safe round-trips with both rename and skip.
#[derive(Debug, PartialEq, Serialize, Deserialize, JsonApiModel)]
#[jsonapi(type = "users")]
struct User {
    id: String,
    name: String,
    /// Both attributes needed: serde handles deserialisation, jsonapi handles output key.
    #[serde(rename = "emailAddress")]
    #[jsonapi(rename = "emailAddress")]
    email: String,
    /// Skipped in jsonapi; serde uses default so the field can be reconstructed.
    #[jsonapi(skip)]
    #[serde(default)]
    password_hash: String,
}

// ---------------------------------------------------------------------------
// Round-trip: to_jsonapi_document / from_jsonapi_document
// ---------------------------------------------------------------------------

#[test]
fn derive_round_trip_simple_struct() {
    let original = Article {
        id: "5".into(),
        title: "Round Trip".into(),
        body: "content".into(),
        views: 100,
    };
    let doc = original.to_jsonapi_document();
    assert!(doc.is_valid());

    let json = serde_json::to_string(&doc).unwrap();
    let doc_back: DocumentData = serde_json::from_str(&json).unwrap();
    let recovered = Article::from_jsonapi_document(&doc_back).unwrap();
    assert_eq!(original, recovered);
}

#[test]
fn derive_round_trip_with_skip_and_rename() {
    let original = User {
        id: "7".into(),
        name: "Carol".into(),
        email: "carol@example.com".into(),
        password_hash: "secret".into(),
    };
    let doc = original.to_jsonapi_document();
    let json = serde_json::to_string(&doc).unwrap();
    let doc_back: DocumentData = serde_json::from_str(&json).unwrap();
    let recovered = User::from_jsonapi_document(&doc_back).unwrap();

    assert_eq!(recovered.name, original.name);
    assert_eq!(recovered.email, original.email);
    // password_hash is not emitted into JSON:API so it falls back to Default ("")
    assert_eq!(recovered.password_hash, String::default());
}
