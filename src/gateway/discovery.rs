use std::time::Duration;

/// Advertise this Synapse gateway on the local network via mDNS.
#[allow(dead_code)]
pub struct MdnsAdvertiser {
    service_name: String,
    port: u16,
}

#[allow(dead_code)]
impl MdnsAdvertiser {
    pub fn new(instance_name: &str, port: u16) -> Self {
        Self {
            service_name: instance_name.to_string(),
            port,
        }
    }

    /// Start advertising. Returns a handle that stops advertising on drop.
    pub async fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        // mdns-sd crate would be used here. For now, placeholder.
        tracing::info!(
            service = %self.service_name,
            port = self.port,
            "mDNS: advertising _synapse._tcp on local network"
        );
        // TODO: use mdns-sd crate when added as dependency
        // let mdns = ServiceDaemon::new()?;
        // let service = ServiceInfo::new("_synapse._tcp.local.", &self.service_name, ...);
        // mdns.register(service)?;
        Ok(())
    }
}

/// Discover Synapse gateways on the local network.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct DiscoveredGateway {
    pub name: String,
    pub host: String,
    pub port: u16,
}

#[allow(dead_code)]
pub async fn discover_gateways(timeout: Duration) -> Vec<DiscoveredGateway> {
    tracing::info!(
        timeout_secs = timeout.as_secs(),
        "mDNS: scanning for _synapse._tcp services"
    );
    // TODO: use mdns-sd crate for actual discovery
    // let mdns = ServiceDaemon::new().ok()?;
    // let receiver = mdns.browse("_synapse._tcp.local.").ok()?;
    // collect for timeout duration
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn advertiser_creates() {
        let adv = MdnsAdvertiser::new("my-synapse", 3000);
        assert_eq!(adv.port, 3000);
    }

    #[tokio::test]
    async fn discover_returns_empty_without_mdns() {
        let results = discover_gateways(Duration::from_millis(100)).await;
        assert!(results.is_empty());
    }
}
