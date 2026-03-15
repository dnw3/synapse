use sha2::{Digest, Sha256};

use crate::gateway::nodes::bootstrap::generate_pairing_token;
use crate::gateway::nodes::PairingStore;

pub struct DevicesArgs {
    pub request_id: Option<String>,
    pub device_id: Option<String>,
    pub name: Option<String>,
}

pub fn run(action: &str, args: &DevicesArgs) {
    let mut store = PairingStore::new();

    match action {
        "list" => {
            let pending = store.list_pending();
            let paired = store.list_paired();

            if pending.is_empty() && paired.is_empty() {
                println!("No devices.");
                return;
            }

            if !pending.is_empty() {
                println!("Pending requests:");
                println!("{:<36}  {:<20}  {:<10}", "REQUEST ID", "NAME", "PLATFORM");
                for req in &pending {
                    println!(
                        "{:<36}  {:<20}  {:<10}",
                        req.request_id,
                        &req.node_name,
                        req.platform.as_deref().unwrap_or("-"),
                    );
                }
                println!();
            }

            if !paired.is_empty() {
                println!("Paired devices:");
                println!("{:<36}  {:<20}  {:<10}", "NODE ID", "NAME", "PLATFORM");
                for node in &paired {
                    println!(
                        "{:<36}  {:<20}  {:<10}",
                        node.node_id,
                        &node.name,
                        node.platform.as_deref().unwrap_or("-"),
                    );
                }
            }
        }
        "approve" => {
            let request_id = match args.request_id.as_deref() {
                Some(id) => id.to_string(),
                None => {
                    let pending = store.list_pending();
                    if pending.is_empty() {
                        eprintln!("No pending requests.");
                        return;
                    }
                    pending.last().unwrap().request_id.clone()
                }
            };
            match store.approve(&request_id) {
                Some(node) => println!("Approved: {} ({})", node.node_id, &node.name,),
                None => eprintln!("Request not found: {request_id}"),
            }
        }
        "reject" => {
            let id = args
                .request_id
                .as_deref()
                .expect("request_id required for reject");
            if store.reject(id) {
                println!("Rejected: {id}");
            } else {
                eprintln!("Request not found: {id}");
            }
        }
        "remove" => {
            let id = args
                .device_id
                .as_deref()
                .expect("device_id required for remove");
            if store.remove_paired(id) {
                println!("Removed: {id}");
            } else {
                eprintln!("Device not found: {id}");
            }
        }
        "rename" => {
            let id = args
                .device_id
                .as_deref()
                .expect("--device required for rename");
            let name = args.name.as_deref().expect("--name required for rename");
            if store.rename(id, name) {
                println!("Renamed {id} to {name}");
            } else {
                eprintln!("Device not found: {id}");
            }
        }
        "rotate" => {
            let id = args
                .device_id
                .as_deref()
                .expect("--device required for rotate");
            let new_token = generate_pairing_token();
            let hash = format!("{:x}", Sha256::digest(new_token.as_bytes()));
            if store.update_token_hash(id, &hash) {
                println!("Token rotated for {id}");
                println!("New token: {new_token}");
            } else {
                eprintln!("Device not found: {id}");
            }
        }
        "revoke" => {
            let id = args
                .device_id
                .as_deref()
                .expect("--device required for revoke");
            if store.update_token_hash(id, "") {
                println!("Token revoked for {id}");
            } else {
                eprintln!("Device not found: {id}");
            }
        }
        _ => {
            eprintln!(
                "Unknown action: {action}. Use: list, approve, reject, remove, rename, rotate, revoke"
            );
        }
    }
}
