/// Convert a client-facing request key to an internal storage key.
/// "main" → "agent:default:main"
/// "lark:direct:user123" → "agent:default:lark:direct:user123"
pub fn to_store_key(agent_id: &str, request_key: &str) -> String {
    format!("agent:{}:{}", agent_id, request_key)
}

/// Extract the client-facing request key from an internal storage key.
/// "agent:default:main" → "main"
/// "agent:default:lark:direct:user123" → "lark:direct:user123"
#[allow(dead_code)]
pub fn to_request_key(store_key: &str) -> &str {
    let mut parts = store_key.splitn(3, ':');
    let _ = parts.next(); // "agent"
    let _ = parts.next(); // agentId
    parts.next().unwrap_or(store_key)
}

/// Extract agent ID from a storage key.
/// "agent:default:main" → "default"
#[allow(dead_code)]
pub fn agent_id_from_store_key(store_key: &str) -> &str {
    store_key.split(':').nth(1).unwrap_or("default")
}

/// Validate a request key (client-facing).
pub fn validate_request_key(key: &str) -> Result<(), String> {
    if key.is_empty() {
        return Err("session key must not be empty".into());
    }
    if key.len() > 256 {
        return Err("session key too long (max 256)".into());
    }
    if key.contains(|c: char| c.is_control()) {
        return Err("session key must not contain control characters".into());
    }
    Ok(())
}

/// Build a session key for a channel DM.
/// "lark", "user123" → "lark:direct:user123"
#[allow(dead_code)]
pub fn channel_dm_key(channel: &str, peer_id: &str) -> String {
    format!("{}:direct:{}", channel, peer_id)
}

/// Build a session key for a channel group.
/// "lark", "chat123" → "lark:group:chat123"
#[allow(dead_code)]
pub fn channel_group_key(channel: &str, group_id: &str) -> String {
    format!("{}:group:{}", channel, group_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_store_key() {
        assert_eq!(to_store_key("default", "main"), "agent:default:main");
        assert_eq!(
            to_store_key("default", "lark:direct:u1"),
            "agent:default:lark:direct:u1"
        );
    }

    #[test]
    fn test_to_request_key() {
        assert_eq!(to_request_key("agent:default:main"), "main");
        assert_eq!(
            to_request_key("agent:default:lark:direct:u1"),
            "lark:direct:u1"
        );
        assert_eq!(to_request_key("not-a-store-key"), "not-a-store-key");
    }

    #[test]
    fn test_agent_id() {
        assert_eq!(agent_id_from_store_key("agent:default:main"), "default");
        assert_eq!(agent_id_from_store_key("agent:mybot:chat"), "mybot");
    }

    #[test]
    fn test_validate() {
        assert!(validate_request_key("main").is_ok());
        assert!(validate_request_key("lark:direct:u1").is_ok());
        assert!(validate_request_key("").is_err());
        assert!(validate_request_key("a\x00b").is_err());
        assert!(validate_request_key(&"x".repeat(300)).is_err());
    }

    #[test]
    fn test_channel_keys() {
        assert_eq!(channel_dm_key("lark", "u1"), "lark:direct:u1");
        assert_eq!(channel_group_key("lark", "g1"), "lark:group:g1");
    }
}
