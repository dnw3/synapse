use synaptic::secrets::SecretRegistry;

#[tokio::main]
async fn main() {
    println!("=== SecretRegistry Demo ===\n");

    let registry = SecretRegistry::new();

    // Register secrets
    registry.register("api_key", "sk-proj-abc123xyz789");
    registry.register("db_password", "super_secret_pw!");
    registry.register_with_mask("ssn", "123-45-6789", "[SSN HIDDEN]");

    // Demonstrate mask_output: replace secret values with masks
    let model_output = "I found the API key sk-proj-abc123xyz789 in the config. \
                        The database password is super_secret_pw! and the SSN is 123-45-6789.";

    println!("--- Original model output ---");
    println!("{}\n", model_output);

    let masked = registry.mask_output(model_output);
    println!("--- After mask_output ---");
    println!("{}\n", masked);

    // Demonstrate inject: replace {{secret:name}} placeholders with real values
    let template = "Authorization: Bearer {{secret:api_key}}\nDB_PASS={{secret:db_password}}";

    println!("--- Template before inject ---");
    println!("{}\n", template);

    let injected = registry.inject(template).unwrap();
    println!("--- After inject ---");
    println!("{}\n", injected);

    // Demonstrate missing secret error
    let bad_template = "Token: {{secret:nonexistent}}";
    match registry.inject(bad_template) {
        Ok(_) => println!("Unexpected success"),
        Err(e) => println!("--- Expected error for missing secret ---\n{}\n", e),
    }

    // Remove a secret and verify it no longer masks
    registry.remove("api_key");
    let after_remove = registry.mask_output("Key is sk-proj-abc123xyz789");
    println!("--- After removing api_key secret ---");
    println!("{}", after_remove);

    println!("\nDone.");
}
