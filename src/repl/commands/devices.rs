use crate::channels::dm::DmPolicyEnforcer;

use super::CommandResult;
use crate::config::SynapseConfig;

pub fn cmd_pair(arg: &str, config: &SynapseConfig) -> CommandResult {
    let sub = arg.split_whitespace().next().unwrap_or("");
    let rest: Vec<&str> = arg.split_whitespace().skip(1).collect();

    match sub {
        "" => {
            // Generate QR + setup code
            let mut bootstrap = crate::gateway::nodes::BootstrapStore::new();
            let token = bootstrap.issue();
            let port = config.serve.as_ref().and_then(|s| s.port).unwrap_or(3000);
            let url = format!("ws://localhost:{}/ws", port);
            let code = crate::gateway::nodes::bootstrap::encode_setup_code(&url, &token);
            if let Some(qr) = crate::gateway::nodes::bootstrap::generate_qr_text(&code) {
                eprintln!("{qr}");
            }
            eprintln!("Setup code: {code}");
            eprintln!("Gateway:    {url}");
            eprintln!("Expires in 10 minutes.");
        }
        "list" => {
            let mut store = crate::gateway::nodes::PairingStore::new();
            let pending = store.list_pending();
            let paired = store.list_paired();
            if pending.is_empty() && paired.is_empty() {
                eprintln!("No devices.");
            } else {
                if !pending.is_empty() {
                    eprintln!("Pending:");
                    for r in &pending {
                        eprintln!(
                            "  {} - {} ({})",
                            r.request_id,
                            &r.node_name,
                            r.platform.as_deref().unwrap_or("-")
                        );
                    }
                }
                if !paired.is_empty() {
                    eprintln!("Paired:");
                    for n in &paired {
                        eprintln!(
                            "  {} - {} ({})",
                            n.node_id,
                            &n.name,
                            n.platform.as_deref().unwrap_or("-")
                        );
                    }
                }
            }
        }
        "approve" => {
            let mut store = crate::gateway::nodes::PairingStore::new();
            if let Some(id) = rest.first().copied() {
                match store.approve(id) {
                    Some(n) => eprintln!("Approved: {}", n.node_id),
                    None => eprintln!("Not found: {id}"),
                }
            } else {
                let pending = store.list_pending();
                if pending.is_empty() {
                    eprintln!("No pending requests.");
                } else {
                    let last_id = pending.last().unwrap().request_id.clone();
                    match store.approve(&last_id) {
                        Some(n) => eprintln!("Approved: {}", n.node_id),
                        None => eprintln!("Not found: {last_id}"),
                    }
                }
            }
        }
        "reject" => {
            if let Some(id) = rest.first() {
                let mut store = crate::gateway::nodes::PairingStore::new();
                if store.reject(id) {
                    eprintln!("Rejected: {id}");
                } else {
                    eprintln!("Not found: {id}");
                }
            } else {
                eprintln!("Usage: /pair reject <request_id>");
            }
        }
        "remove" => {
            if let Some(id) = rest.first() {
                let mut store = crate::gateway::nodes::PairingStore::new();
                if store.remove_paired(id) {
                    eprintln!("Removed: {id}");
                } else {
                    eprintln!("Not found: {id}");
                }
            } else {
                eprintln!("Usage: /pair remove <device_id>");
            }
        }
        _ => eprintln!("Usage: /pair [list|approve|reject|remove]"),
    }
    CommandResult::Continue
}

pub async fn cmd_dm(arg: &str) -> CommandResult {
    let parts: Vec<&str> = arg.split_whitespace().collect();
    let sub = parts.first().copied().unwrap_or("");
    let channel = parts.get(1).copied().unwrap_or("");

    if sub.is_empty() {
        eprintln!("Usage: /dm <action> <channel> [value]");
        return CommandResult::Continue;
    }

    if channel.is_empty() {
        eprintln!("Usage: /dm <action> <channel> [value]");
        return CommandResult::Continue;
    }

    let pairing_dir = dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".synapse")
        .join("pairing");
    let enforcer = crate::channels::dm::FileDmPolicyEnforcer::new(
        pairing_dir,
        crate::channels::dm::DmPolicy::Pairing,
        None,
    );

    match sub {
        "list" => {
            let pending = enforcer.list_pending(channel).await;
            if pending.is_empty() {
                eprintln!("No pending DM requests for {channel}.");
            } else {
                for p in &pending {
                    eprintln!("  {} - {} ({})", p.code, p.sender_id, p.channel);
                }
            }
        }
        "approve" => {
            let code = parts.get(2).copied().unwrap_or("");
            if code.is_empty() {
                eprintln!("Usage: /dm approve <channel> <code>");
            } else {
                match enforcer.approve_code(channel, code).await {
                    Ok(sender) => eprintln!("Approved: {sender}"),
                    Err(e) => eprintln!("Failed: {e}"),
                }
            }
        }
        "allowlist" => {
            let list = enforcer.get_allowlist(channel);
            if list.is_empty() {
                eprintln!("Allowlist for {channel} is empty.");
            } else {
                for id in &list {
                    eprintln!("  {id}");
                }
            }
        }
        "remove" => {
            let sender = parts.get(2).copied().unwrap_or("");
            if sender.is_empty() {
                eprintln!("Usage: /dm remove <channel> <sender_id>");
            } else if enforcer.remove_from_allowlist(channel, sender) {
                eprintln!("Removed: {sender}");
            } else {
                eprintln!("Not found: {sender}");
            }
        }
        _ => eprintln!("Usage: /dm [list|approve|allowlist|remove] <channel>"),
    }
    CommandResult::Continue
}
