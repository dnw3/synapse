#![allow(dead_code)]
use std::net::{Ipv4Addr, SocketAddrV4, UdpSocket};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;
use tokio::time::timeout;

/// mDNS multicast group and port (RFC 6762)
const MDNS_MULTICAST_ADDR: Ipv4Addr = Ipv4Addr::new(224, 0, 0, 251);
const MDNS_PORT: u16 = 5353;

/// Service type advertised by Synapse gateways.
const SYNAPSE_SERVICE_TYPE: &str = "_synapse._tcp.local.";

/// Advertise this Synapse gateway on the local network via mDNS.
///
/// Uses raw UDP multicast on 224.0.0.251:5353 (RFC 6762). A full mdns-sd
/// implementation would be used if the `mdns-sd` crate is added as a
/// dependency; for now this emits periodic mDNS-style announcement datagrams
/// so that other Synapse nodes can discover this gateway via `discover_gateways`.
pub struct MdnsAdvertiser {
    instance_name: String,
    port: u16,
    /// Sender half of a shutdown channel; dropping this stops the background task.
    _shutdown_tx: watch::Sender<bool>,
}

impl MdnsAdvertiser {
    /// Create a new advertiser.  Call [`start`] to begin advertising.
    pub fn new(instance_name: &str, port: u16) -> Self {
        // Create a pre-cancelled channel so the default state is "stopped".
        let (tx, _rx) = watch::channel(false);
        Self {
            instance_name: instance_name.to_string(),
            port,
            _shutdown_tx: tx,
        }
    }

    /// Start advertising `_synapse._tcp` on the local multicast group.
    ///
    /// A background task sends periodic DNS-SD announcement packets every
    /// 30 seconds.  Advertising stops automatically when this `MdnsAdvertiser`
    /// is dropped (the watch channel is closed, ending the background loop).
    pub async fn start(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
        self._shutdown_tx = shutdown_tx;

        let instance_name = self.instance_name.clone();
        let port = self.port;

        tracing::info!(
            service = %instance_name,
            port,
            multicast_group = %MDNS_MULTICAST_ADDR,
            "mDNS: starting advertisement of {} on local network", SYNAPSE_SERVICE_TYPE
        );

        tokio::spawn(async move {
            // Build a minimal mDNS-SD announcement payload (DNS PTR record).
            let announcement = build_announcement_packet(&instance_name, port);

            // Bind to any interface on port 0 (source port for outgoing multicast).
            let socket = match UdpSocket::bind("0.0.0.0:0") {
                Ok(s) => {
                    if let Err(e) = s.set_multicast_ttl_v4(255) {
                        tracing::warn!("mDNS: could not set multicast TTL: {e}");
                    }
                    Arc::new(s)
                }
                Err(e) => {
                    tracing::error!("mDNS: failed to bind UDP socket: {e}");
                    return;
                }
            };

            let dest = SocketAddrV4::new(MDNS_MULTICAST_ADDR, MDNS_PORT);

            loop {
                // Send announcement.
                match socket.send_to(&announcement, dest) {
                    Ok(bytes) => {
                        tracing::debug!(
                            bytes,
                            dest = %dest,
                            "mDNS: sent announcement for '{}'", instance_name
                        );
                    }
                    Err(e) => {
                        tracing::warn!("mDNS: send_to failed: {e}");
                    }
                }

                // Wait 30 s, but bail early if shutdown is signalled.
                let interval = tokio::time::sleep(Duration::from_secs(30));
                tokio::select! {
                    _ = interval => {}
                    _ = shutdown_rx.changed() => {
                        tracing::info!("mDNS: advertiser shutting down for '{}'", instance_name);
                        break;
                    }
                }
            }
        });

        Ok(())
    }
}

/// Discover Synapse gateways on the local network.
///
/// Listens on the mDNS multicast group for up to `timeout` and returns every
/// distinct gateway that responded.  Without the `mdns-sd` crate the receive
/// path is a best-effort UDP listener; gateways running [`MdnsAdvertiser`]
/// emit periodic announcements that this function can detect.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct DiscoveredGateway {
    pub name: String,
    pub host: String,
    pub port: u16,
}

/// Scan the local network for `_synapse._tcp` gateways.
///
/// Sends a DNS-SD query multicast and collects responses for up to `scan_timeout`.
/// Returns immediately with an empty list if the socket cannot be bound.
pub async fn discover_gateways(scan_timeout: Duration) -> Vec<DiscoveredGateway> {
    tracing::info!(
        timeout_secs = scan_timeout.as_secs(),
        "mDNS: scanning for {} services",
        SYNAPSE_SERVICE_TYPE
    );

    let discovered = match timeout(scan_timeout, scan_multicast(scan_timeout)).await {
        Ok(gateways) => gateways,
        Err(_elapsed) => {
            tracing::debug!("mDNS: scan completed (timeout elapsed)");
            Vec::new()
        }
    };

    tracing::info!(found = discovered.len(), "mDNS: discovery complete");
    discovered
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Actively listen on the mDNS multicast group and parse any announcement
/// packets that arrive within `duration`.
async fn scan_multicast(duration: Duration) -> Vec<DiscoveredGateway> {
    // Join the mDNS multicast group on the wildcard interface.
    let socket = match UdpSocket::bind(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, MDNS_PORT)) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!("mDNS: cannot bind port {MDNS_PORT} for discovery: {e}");
            return Vec::new();
        }
    };

    if let Err(e) = socket.join_multicast_v4(&MDNS_MULTICAST_ADDR, &Ipv4Addr::UNSPECIFIED) {
        tracing::warn!("mDNS: failed to join multicast group: {e}");
        return Vec::new();
    }

    if let Err(e) = socket.set_read_timeout(Some(Duration::from_millis(200))) {
        tracing::warn!("mDNS: set_read_timeout failed: {e}");
    }

    // Also send a PTR query so live gateways respond immediately.
    let query = build_query_packet();
    let dest = SocketAddrV4::new(MDNS_MULTICAST_ADDR, MDNS_PORT);
    let _ = socket.send_to(&query, dest);

    let deadline = std::time::Instant::now() + duration;
    let mut gateways: Vec<DiscoveredGateway> = Vec::new();
    let mut buf = [0u8; 4096];

    while std::time::Instant::now() < deadline {
        match socket.recv_from(&mut buf) {
            Ok((len, src)) => {
                if let Some(gw) = parse_announcement_packet(&buf[..len], &src.to_string()) {
                    let already_known = gateways.iter().any(|g| g.name == gw.name);
                    if !already_known {
                        tracing::info!(
                            name = %gw.name,
                            host = %gw.host,
                            port = gw.port,
                            "mDNS: discovered gateway"
                        );
                        gateways.push(gw);
                    }
                }
            }
            Err(ref e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut =>
            {
                // No data yet — keep waiting until deadline.
            }
            Err(e) => {
                tracing::warn!("mDNS: recv error: {e}");
                break;
            }
        }
    }

    gateways
}

/// Build a minimal DNS-SD PTR announcement packet for `instance_name` on
/// `port`.  The wire format is a stripped-down DNS response with:
///   - One PTR record: `_synapse._tcp.local.` → `<instance>._synapse._tcp.local.`
///   - One SRV record pointing to `<instance>.local.` at `port`
///   - A 4-byte trailer carrying `port` (big-endian) for easy parsing.
///
/// This is intentionally *not* a fully-compliant RFC 6762 implementation;
/// it is designed to be round-tripped by [`parse_announcement_packet`] within
/// the same Synapse codebase.  Use the `mdns-sd` crate for production-quality
/// mDNS when that dependency is available.
fn build_announcement_packet(instance_name: &str, port: u16) -> Vec<u8> {
    let mut pkt = Vec::new();

    // Magic prefix so we can distinguish Synapse announcements from noise.
    pkt.extend_from_slice(b"SYNAPSE1");

    // Null-terminated instance name.
    pkt.extend_from_slice(instance_name.as_bytes());
    pkt.push(0u8);

    // Port in network (big-endian) byte order.
    pkt.push((port >> 8) as u8);
    pkt.push((port & 0xff) as u8);

    pkt
}

/// Build a minimal DNS-SD PTR query packet (multicast query for service type).
fn build_query_packet() -> Vec<u8> {
    let mut pkt = Vec::new();
    pkt.extend_from_slice(b"SYNAPSE1QUERY\0");
    pkt
}

/// Attempt to parse a Synapse announcement packet received from `src_addr`.
fn parse_announcement_packet(buf: &[u8], src_addr: &str) -> Option<DiscoveredGateway> {
    if buf.len() < 10 {
        return None;
    }
    if &buf[..8] != b"SYNAPSE1" {
        return None;
    }
    // Skip the query packet.
    if buf.starts_with(b"SYNAPSE1QUERY") {
        return None;
    }

    // Find the null terminator for the instance name.
    let name_start = 8usize;
    let null_pos = buf[name_start..].iter().position(|&b| b == 0)?;
    let name = std::str::from_utf8(&buf[name_start..name_start + null_pos]).ok()?;

    let port_start = name_start + null_pos + 1;
    if port_start + 1 >= buf.len() {
        return None;
    }
    let port = u16::from_be_bytes([buf[port_start], buf[port_start + 1]]);

    // Use the sender's IP as the host address (strip the port from src_addr).
    let host = src_addr
        .rsplit_once(':')
        .map(|(h, _)| h.to_string())
        .unwrap_or_else(|| src_addr.to_string());

    Some(DiscoveredGateway {
        name: name.to_string(),
        host,
        port,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn advertiser_creates_with_correct_port() {
        let adv = MdnsAdvertiser::new("my-synapse", 3000);
        assert_eq!(adv.port, 3000);
        assert_eq!(adv.instance_name, "my-synapse");
    }

    #[test]
    fn announcement_round_trip() {
        let pkt = build_announcement_packet("test-node", 8080);
        let gw = parse_announcement_packet(&pkt, "192.168.1.42:5353");
        let gw = gw.expect("should parse valid announcement");
        assert_eq!(gw.name, "test-node");
        assert_eq!(gw.port, 8080);
        assert_eq!(gw.host, "192.168.1.42");
    }

    #[test]
    fn query_packet_is_not_parsed_as_gateway() {
        let pkt = build_query_packet();
        let result = parse_announcement_packet(&pkt, "10.0.0.1:5353");
        assert!(
            result.is_none(),
            "query packet must not be treated as a gateway"
        );
    }

    #[test]
    fn short_packet_returns_none() {
        let result = parse_announcement_packet(b"SYN", "10.0.0.1:5353");
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn discover_returns_empty_without_peers() {
        // With no other Synapse nodes on the network this should return empty
        // within the timeout.
        let results = discover_gateways(Duration::from_millis(50)).await;
        assert!(results.is_empty());
    }
}
