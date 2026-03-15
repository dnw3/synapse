use synaptic::DmPolicy;
use synaptic::DmPolicyEnforcer;

use crate::channels::dm::FileDmPolicyEnforcer;

pub async fn run(action: &str, channel: &str, value: Option<&str>) {
    let pairing_dir = dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".synapse")
        .join("pairing");
    let enforcer = FileDmPolicyEnforcer::new(pairing_dir, DmPolicy::Pairing, None);

    match action {
        "list" => {
            let pending = enforcer.list_pending(channel).await;
            if pending.is_empty() {
                println!("No pending DM pairing requests for {channel}.");
                return;
            }
            println!("Pending DM pairing requests for {channel}:");
            println!("{:<10}  {:<30}  {:<20}", "CODE", "SENDER", "CREATED");
            for p in &pending {
                let created = chrono::DateTime::from_timestamp_millis(p.created_at as i64)
                    .map(|d| d.format("%Y-%m-%d %H:%M").to_string())
                    .unwrap_or_else(|| "-".to_string());
                println!("{:<10}  {:<30}  {:<20}", p.code, p.sender_id, created);
            }
        }
        "approve" => {
            let code = value.expect("code required: synapse pairing approve <channel> <code>");
            match enforcer.approve_code(channel, code).await {
                Ok(sender) => println!("Approved sender {sender} for {channel}"),
                Err(e) => eprintln!("Failed: {e}"),
            }
        }
        "allowlist" => {
            let list = enforcer.get_allowlist(channel);
            if list.is_empty() {
                println!("Allowlist for {channel} is empty.");
            } else {
                println!("Allowlist for {channel}:");
                for id in &list {
                    println!("  {id}");
                }
            }
        }
        "remove" => {
            let sender =
                value.expect("sender_id required: synapse pairing remove <channel> <sender_id>");
            if enforcer.remove_from_allowlist(channel, sender) {
                println!("Removed {sender} from {channel} allowlist");
            } else {
                eprintln!("{sender} not found in {channel} allowlist");
            }
        }
        _ => {
            eprintln!("Unknown action: {action}. Use: list, approve, allowlist, remove");
        }
    }
}
