#![cfg(feature = "schemars")]
#![allow(dead_code)]

use schemars::JsonSchema;
use serde::Deserialize;
use synaptic_core::SynapticError;
use synaptic_core::Tool;
use synaptic_macros::tool;

#[derive(Deserialize, JsonSchema)]
struct UserInfo {
    /// User's display name
    name: String,
    /// Age in years
    age: i32,
    email: Option<String>,
}

/// Process user information.
#[tool]
async fn process_user(
    /// The user to process
    user: UserInfo,
    /// Action to perform
    action: String,
) -> Result<String, SynapticError> {
    Ok(format!("{}: {}", user.name, action))
}

#[tokio::test]
async fn custom_type_has_full_schema() {
    let tool = process_user();
    let params = tool.parameters().unwrap();
    let props = params["properties"].as_object().unwrap();

    // "action" is a primitive — still uses hardcoded schema
    assert_eq!(props["action"]["type"], "string");

    // "user" is a custom type — schemars generates full schema
    let user_schema = props["user"].as_object().unwrap();
    assert_eq!(user_schema["type"], "object");
    let user_props = user_schema["properties"].as_object().unwrap();
    assert_eq!(user_props["name"]["type"], "string");
    assert_eq!(user_props["age"]["type"], "integer");
    assert!(user_props.contains_key("email"));
}

#[derive(Deserialize, JsonSchema)]
struct Address {
    street: String,
    city: String,
    zip: String,
}

#[derive(Deserialize, JsonSchema)]
struct Customer {
    name: String,
    address: Address,
}

/// Process a customer order.
#[tool]
async fn process_order(
    /// The customer placing the order
    customer: Customer,
    /// Item to order
    item: String,
) -> Result<String, SynapticError> {
    Ok(format!("{} ordered {}", customer.name, item))
}

#[tokio::test]
async fn nested_custom_type_has_schema() {
    let tool = process_order();
    let params = tool.parameters().unwrap();
    let props = params["properties"].as_object().unwrap();

    // "item" is a primitive
    assert_eq!(props["item"]["type"], "string");

    // "customer" is a custom type with nested Address
    let customer_schema = props["customer"].as_object().unwrap();
    assert_eq!(customer_schema["type"], "object");
    let customer_props = customer_schema["properties"].as_object().unwrap();
    assert_eq!(customer_props["name"]["type"], "string");
    // Address should be present (either inline or via $ref/$defs)
    assert!(customer_props.contains_key("address"));
}

/// Test with optional custom type.
#[tool]
async fn maybe_user(
    /// An optional user
    user: Option<UserInfo>,
) -> Result<String, SynapticError> {
    match user {
        Some(u) => Ok(format!("Got user: {}", u.name)),
        None => Ok("No user".into()),
    }
}

#[tokio::test]
async fn optional_custom_type_has_schema() {
    let tool = maybe_user();
    let params = tool.parameters().unwrap();
    let props = params["properties"].as_object().unwrap();

    // "user" should have schemars-generated schema
    let user_schema = props["user"].as_object().unwrap();
    assert_eq!(user_schema["type"], "object");
    let user_props = user_schema["properties"].as_object().unwrap();
    assert_eq!(user_props["name"]["type"], "string");

    // Optional parameter should not be required
    let required = params["required"].as_array().unwrap();
    assert!(required.is_empty());
}
