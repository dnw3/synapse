//! Multi-agent routing — bindings-based routing with priority-chain matching.
//!
//! Replaces the old score-based `AgentRouter` with OpenClaw-aligned `BindingRouter`.
//!
//! Priority chain (highest → lowest):
//!   peer match → guild+roles → guild → team → account → channel → default

use std::collections::HashMap;

use crate::config::{AgentBroadcastGroup, AgentDef, AgentsConfig, Binding, PeerKind};

// ---------------------------------------------------------------------------
// Routing context
// ---------------------------------------------------------------------------

/// Context for routing a message to an agent.
#[allow(dead_code)]
#[derive(Debug, Default)]
pub struct RoutingContext {
    /// Channel name (e.g. "lark", "discord", "slack").
    pub channel: Option<String>,
    /// Account ID within the channel (for multi-account setups).
    pub account_id: Option<String>,
    /// Whether this is a DM or group.
    pub peer_kind: Option<PeerKind>,
    /// Platform-specific peer ID (chat_id for groups, sender_id for DMs).
    pub peer_id: Option<String>,
    /// Sender ID (user identity).
    pub sender_id: Option<String>,
    /// Discord guild ID.
    pub guild_id: Option<String>,
    /// Slack team/workspace ID.
    pub team_id: Option<String>,
    /// User's roles (Discord).
    pub roles: Vec<String>,
    /// Message text (for pattern matching, reserved for future use).
    pub message: Option<String>,
}

// ---------------------------------------------------------------------------
// Route result
// ---------------------------------------------------------------------------

/// Result of routing a message.
#[derive(Debug)]
pub enum RouteResult<'a> {
    /// Route to a single agent.
    Single(ResolvedAgent<'a>),
    /// Route to multiple agents (broadcast).
    Broadcast {
        group: &'a AgentBroadcastGroup,
        agents: Vec<ResolvedAgent<'a>>,
    },
}

/// A resolved agent with its definition and the binding that matched.
#[derive(Debug)]
pub struct ResolvedAgent<'a> {
    /// Agent definition.
    pub def: &'a AgentDef,
    /// The binding that matched (None for default fallback).
    pub binding: Option<&'a Binding>,
}

// ---------------------------------------------------------------------------
// Binding router
// ---------------------------------------------------------------------------

/// Bindings-based router with priority-chain matching.
pub struct BindingRouter {
    /// Agent definitions keyed by ID.
    agents: HashMap<String, AgentDef>,
    /// Default agent ID.
    default_agent: String,
    /// Route bindings (in config order).
    bindings: Vec<Binding>,
    /// Agent broadcast groups.
    broadcasts: Vec<AgentBroadcastGroup>,
}

impl BindingRouter {
    /// Create a router from config.
    pub fn new(
        agents_config: &AgentsConfig,
        bindings: &[Binding],
        broadcasts: &[AgentBroadcastGroup],
    ) -> Self {
        let mut agents = HashMap::new();
        for def in &agents_config.list {
            agents.insert(def.id.clone(), def.clone());
        }
        Self {
            agents,
            default_agent: agents_config.default.clone(),
            bindings: bindings.to_vec(),
            broadcasts: broadcasts.to_vec(),
        }
    }

    /// Resolve the routing for a message context.
    pub fn resolve(&self, ctx: &RoutingContext) -> RouteResult<'_> {
        // 1. Check broadcast groups first
        if let Some(bg) = self.match_broadcast(ctx) {
            let agents: Vec<_> = bg
                .agents
                .iter()
                .filter_map(|id| {
                    self.agents
                        .get(id)
                        .map(|def| ResolvedAgent { def, binding: None })
                })
                .collect();
            if !agents.is_empty() {
                return RouteResult::Broadcast { group: bg, agents };
            }
        }

        // 2. Find best matching binding by specificity
        let matched = self
            .bindings
            .iter()
            .filter(|b| self.binding_matches(b, ctx))
            .max_by_key(|b| self.binding_specificity(b));

        match matched {
            Some(b) => {
                let def = self
                    .agents
                    .get(&b.agent)
                    .or_else(|| self.agents.get(&self.default_agent));
                match def {
                    Some(def) => RouteResult::Single(ResolvedAgent {
                        def,
                        binding: Some(b),
                    }),
                    None => self.default_route(),
                }
            }
            None => self.default_route(),
        }
    }

    /// Get the default agent definition.
    pub fn default_agent(&self) -> Option<&AgentDef> {
        self.agents.get(&self.default_agent)
    }

    /// Get an agent definition by ID.
    pub fn get_agent(&self, id: &str) -> Option<&AgentDef> {
        self.agents.get(id)
    }

    /// List all agent definitions.
    pub fn agents(&self) -> &HashMap<String, AgentDef> {
        &self.agents
    }

    /// List all bindings.
    pub fn bindings(&self) -> &[Binding] {
        &self.bindings
    }

    /// List all broadcast groups.
    pub fn broadcasts(&self) -> &[AgentBroadcastGroup] {
        &self.broadcasts
    }

    // -----------------------------------------------------------------------
    // Internal
    // -----------------------------------------------------------------------

    fn default_route(&self) -> RouteResult<'_> {
        let def = self
            .agents
            .get(&self.default_agent)
            .expect("default agent must exist");
        RouteResult::Single(ResolvedAgent { def, binding: None })
    }

    /// Check if a binding matches the given context.
    fn binding_matches(&self, b: &Binding, ctx: &RoutingContext) -> bool {
        // Channel constraint
        if let Some(ref ch) = b.channel {
            match &ctx.channel {
                Some(ctx_ch) if ctx_ch == ch => {}
                _ => return false,
            }
        }

        // Account ID constraint
        if let Some(ref acct) = b.account_id {
            match &ctx.account_id {
                Some(ctx_acct) if ctx_acct == acct => {}
                _ => return false,
            }
        }

        // Peer constraint
        if let Some(ref peer) = b.peer {
            let kind_matches = ctx.peer_kind.as_ref().is_some_and(|k| *k == peer.kind);
            let id_matches = ctx.peer_id.as_ref().is_some_and(|id| *id == peer.id);
            if !kind_matches || !id_matches {
                return false;
            }
        }

        // Guild constraint (Discord)
        if let Some(ref guild) = b.guild_id {
            match &ctx.guild_id {
                Some(ctx_guild) if ctx_guild == guild => {}
                _ => return false,
            }
        }

        // Team constraint (Slack)
        if let Some(ref team) = b.team_id {
            match &ctx.team_id {
                Some(ctx_team) if ctx_team == team => {}
                _ => return false,
            }
        }

        // Roles constraint (AND logic — user must have all listed roles)
        if !b.roles.is_empty() {
            for role in &b.roles {
                if !ctx.roles.contains(role) {
                    return false;
                }
            }
        }

        true
    }

    /// Compute binding specificity score for tie-breaking.
    ///
    /// Priority: peer(100) > guild+roles(80) > guild(60) > team(50)
    ///           > account(30) > channel(10)
    fn binding_specificity(&self, b: &Binding) -> u32 {
        let mut score = 0u32;
        if b.peer.is_some() {
            score += 100;
        }
        if b.guild_id.is_some() {
            score += 60;
        }
        if !b.roles.is_empty() {
            score += 20;
        }
        if b.team_id.is_some() {
            score += 50;
        }
        if b.account_id.is_some() {
            score += 30;
        }
        if b.channel.is_some() {
            score += 10;
        }
        score
    }

    /// Check if a broadcast group matches the context.
    fn match_broadcast(&self, ctx: &RoutingContext) -> Option<&AgentBroadcastGroup> {
        self.broadcasts.iter().find(|bg| {
            // Channel constraint
            if let Some(ref ch) = bg.channel {
                if ctx.channel.as_deref() != Some(ch.as_str()) {
                    return false;
                }
            }
            // Peer ID constraint
            if let Some(ref pid) = bg.peer_id {
                if ctx.peer_id.as_deref() != Some(pid.as_str()) {
                    return false;
                }
            }
            true
        })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{
        AgentBroadcastGroup, AgentDef, AgentsConfig, Binding, BroadcastStrategy, DmSessionScope,
        PeerKind, PeerMatch,
    };

    fn make_agents() -> AgentsConfig {
        AgentsConfig {
            default: "home".into(),
            list: vec![
                AgentDef {
                    id: "home".into(),
                    description: None,
                    model: Some("sonnet".into()),
                    system_prompt: None,
                    workspace: None,
                    dm_scope: DmSessionScope::PerChannelPeer,
                    group_session_scope: None,
                    tool_allow: vec![],
                    tool_deny: vec![],
                    skills_dir: None,
                },
                AgentDef {
                    id: "work".into(),
                    description: None,
                    model: Some("gpt-4o".into()),
                    system_prompt: None,
                    workspace: None,
                    dm_scope: DmSessionScope::PerPeer,
                    group_session_scope: None,
                    tool_allow: vec![],
                    tool_deny: vec![],
                    skills_dir: None,
                },
            ],
        }
    }

    #[test]
    fn default_fallback() {
        let agents = make_agents();
        let router = BindingRouter::new(&agents, &[], &[]);
        let ctx = RoutingContext::default();
        match router.resolve(&ctx) {
            RouteResult::Single(r) => {
                assert_eq!(r.def.id, "home");
                assert!(r.binding.is_none());
            }
            _ => panic!("expected Single"),
        }
    }

    #[test]
    fn channel_binding() {
        let agents = make_agents();
        let bindings = vec![Binding {
            agent: "work".into(),
            channel: Some("slack".into()),
            ..Default::default()
        }];
        let router = BindingRouter::new(&agents, &bindings, &[]);

        // Match
        let ctx = RoutingContext {
            channel: Some("slack".into()),
            ..Default::default()
        };
        match router.resolve(&ctx) {
            RouteResult::Single(r) => assert_eq!(r.def.id, "work"),
            _ => panic!("expected Single"),
        }

        // No match → default
        let ctx2 = RoutingContext {
            channel: Some("discord".into()),
            ..Default::default()
        };
        match router.resolve(&ctx2) {
            RouteResult::Single(r) => assert_eq!(r.def.id, "home"),
            _ => panic!("expected Single"),
        }
    }

    #[test]
    fn peer_beats_channel() {
        let agents = make_agents();
        let bindings = vec![
            Binding {
                agent: "home".into(),
                channel: Some("discord".into()),
                ..Default::default()
            },
            Binding {
                agent: "work".into(),
                channel: Some("discord".into()),
                peer: Some(PeerMatch {
                    kind: PeerKind::Direct,
                    id: "alice".into(),
                }),
                ..Default::default()
            },
        ];
        let router = BindingRouter::new(&agents, &bindings, &[]);

        let ctx = RoutingContext {
            channel: Some("discord".into()),
            peer_kind: Some(PeerKind::Direct),
            peer_id: Some("alice".into()),
            ..Default::default()
        };
        match router.resolve(&ctx) {
            RouteResult::Single(r) => {
                assert_eq!(r.def.id, "work");
                assert!(r.binding.is_some());
            }
            _ => panic!("expected Single"),
        }
    }

    #[test]
    fn account_binding() {
        let agents = make_agents();
        let bindings = vec![
            Binding {
                agent: "home".into(),
                channel: Some("lark".into()),
                account_id: Some("personal".into()),
                ..Default::default()
            },
            Binding {
                agent: "work".into(),
                channel: Some("lark".into()),
                account_id: Some("biz".into()),
                ..Default::default()
            },
        ];
        let router = BindingRouter::new(&agents, &bindings, &[]);

        let ctx = RoutingContext {
            channel: Some("lark".into()),
            account_id: Some("biz".into()),
            ..Default::default()
        };
        match router.resolve(&ctx) {
            RouteResult::Single(r) => assert_eq!(r.def.id, "work"),
            _ => panic!("expected Single"),
        }
    }

    #[test]
    fn guild_roles_binding() {
        let agents = make_agents();
        let bindings = vec![
            Binding {
                agent: "home".into(),
                channel: Some("discord".into()),
                guild_id: Some("g1".into()),
                ..Default::default()
            },
            Binding {
                agent: "work".into(),
                channel: Some("discord".into()),
                guild_id: Some("g1".into()),
                roles: vec!["dev".into()],
                ..Default::default()
            },
        ];
        let router = BindingRouter::new(&agents, &bindings, &[]);

        // User with dev role → work (guild+roles beats guild)
        let ctx = RoutingContext {
            channel: Some("discord".into()),
            guild_id: Some("g1".into()),
            roles: vec!["dev".into(), "admin".into()],
            ..Default::default()
        };
        match router.resolve(&ctx) {
            RouteResult::Single(r) => assert_eq!(r.def.id, "work"),
            _ => panic!("expected Single"),
        }

        // User without dev role → home (guild-only match)
        let ctx2 = RoutingContext {
            channel: Some("discord".into()),
            guild_id: Some("g1".into()),
            roles: vec!["viewer".into()],
            ..Default::default()
        };
        match router.resolve(&ctx2) {
            RouteResult::Single(r) => assert_eq!(r.def.id, "home"),
            _ => panic!("expected Single"),
        }
    }

    #[test]
    fn broadcast_match() {
        let agents = make_agents();
        let broadcasts = vec![AgentBroadcastGroup {
            name: "review".into(),
            description: None,
            channel: Some("lark".into()),
            peer_id: Some("oc_review_group".into()),
            agents: vec!["home".into(), "work".into()],
            strategy: BroadcastStrategy::Parallel,
            timeout_secs: 60,
        }];
        let router = BindingRouter::new(&agents, &[], &broadcasts);

        let ctx = RoutingContext {
            channel: Some("lark".into()),
            peer_id: Some("oc_review_group".into()),
            ..Default::default()
        };
        match router.resolve(&ctx) {
            RouteResult::Broadcast { group, agents } => {
                assert_eq!(group.name, "review");
                assert_eq!(agents.len(), 2);
                assert_eq!(agents[0].def.id, "home");
                assert_eq!(agents[1].def.id, "work");
            }
            _ => panic!("expected Broadcast"),
        }

        // Different peer → no broadcast, falls to default
        let ctx2 = RoutingContext {
            channel: Some("lark".into()),
            peer_id: Some("oc_other".into()),
            ..Default::default()
        };
        match router.resolve(&ctx2) {
            RouteResult::Single(r) => assert_eq!(r.def.id, "home"),
            _ => panic!("expected Single"),
        }
    }
}
