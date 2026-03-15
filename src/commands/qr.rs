use crate::config::SynapseConfig;
use crate::gateway::nodes::bootstrap::{encode_setup_code, generate_qr_text};
use crate::gateway::nodes::BootstrapStore;

pub fn run(config: &SynapseConfig, setup_code_only: bool, url_override: Option<String>) {
    let mut store = BootstrapStore::new();
    let token = store.issue();
    let gateway_url = url_override.unwrap_or_else(|| {
        let port = config.serve.as_ref().and_then(|s| s.port).unwrap_or(3000);
        format!("ws://localhost:{port}/ws")
    });
    let setup_code = encode_setup_code(&gateway_url, &token);

    if setup_code_only {
        println!("{setup_code}");
        return;
    }

    if let Some(qr) = generate_qr_text(&setup_code) {
        println!("{qr}");
    }
    println!();
    println!("Setup code: {setup_code}");
    println!("Gateway:    {gateway_url}");
    println!();
    println!("Expires in 10 minutes.");
}
