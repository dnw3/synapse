use super::*;
use crate::gateway::messages::InboundMessage;

impl AgentSession {
    /// Build a routing context from an inbound message.
    pub(super) fn routing_context(msg: &InboundMessage) -> RoutingContext {
        RoutingContext {
            channel: Some(msg.channel.platform.clone()),
            account_id: msg.channel.account_id.clone(),
            peer_kind: Self::chat_type_to_peer_kind(&msg.chat.chat_type),
            peer_id: msg.sender.id.clone(),
            sender_id: msg.sender.id.clone(),
            guild_id: msg.channel.guild_id.clone(),
            team_id: msg.channel.team_id.clone(),
            roles: msg.chat.roles.clone(),
            message: Some(msg.content.clone()),
        }
    }

    /// Resolve the routing for this message via the binding router.
    pub(super) fn resolve_route(&self, msg: &InboundMessage) -> ResolvedRoute {
        if let Some(ref router) = self.router {
            let ctx = Self::routing_context(msg);
            match router.resolve(&ctx) {
                crate::router::RouteResult::Single(resolved) => {
                    let agent_id = resolved.def.id.clone();
                    tracing::info!(
                        agent = %agent_id,
                        binding = ?resolved.binding.map(|b| &b.agent),
                        "routed to agent"
                    );
                    ResolvedRoute::Single(ResolvedAgentInfo {
                        id: agent_id,
                        model_override: resolved.def.model.clone(),
                        prompt_override: resolved.def.system_prompt.clone(),
                        def: Some(resolved.def.clone()),
                    })
                }
                crate::router::RouteResult::Broadcast { group, agents } => {
                    tracing::info!(
                        broadcast_group = %group.name,
                        strategy = ?group.strategy,
                        agent_count = agents.len(),
                        "broadcast match"
                    );
                    let infos: Vec<_> = agents
                        .iter()
                        .map(|r| ResolvedAgentInfo {
                            id: r.def.id.clone(),
                            model_override: r.def.model.clone(),
                            prompt_override: r.def.system_prompt.clone(),
                            def: Some(r.def.clone()),
                        })
                        .collect();
                    ResolvedRoute::Broadcast {
                        group_name: group.name.clone(),
                        strategy: group.strategy.clone(),
                        agents: infos,
                        timeout_secs: group.timeout_secs,
                    }
                }
            }
        } else {
            ResolvedRoute::Single(ResolvedAgentInfo {
                id: "default".into(),
                model_override: None,
                prompt_override: None,
                def: None,
            })
        }
    }

    /// Load delivery state from session metadata store.
    pub(super) async fn load_delivery_state(&self, session_key: &str) -> SessionDeliveryState {
        let store = self.session_mgr.store();
        let ns = &["delivery_state"];
        match store.get(ns, session_key).await {
            Ok(Some(item)) => serde_json::from_value(item.value).unwrap_or_default(),
            _ => SessionDeliveryState::default(),
        }
    }

    /// Save delivery state to session metadata store.
    pub(super) async fn save_delivery_state(
        &self,
        session_key: &str,
        state: &SessionDeliveryState,
    ) {
        let store = self.session_mgr.store();
        let ns = &["delivery_state"];
        if let Ok(value) = serde_json::to_value(state) {
            let _ = store.put(ns, session_key, value).await;
        }
    }

    /// Resolve or create a persistent session ID for a given session key.
    ///
    /// Uses deterministic session keys so the same person/chat always maps to the
    /// same session.  Metadata (channel, chat_type, display_name) is written on
    /// creation and `updated_at` is bumped on every access.
    pub(super) async fn resolve_session(
        &self,
        session_key: &str,
        msg: &InboundMessage,
    ) -> Result<String, AgentError> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        // 1. Fast path: check in-memory cache
        {
            let map = self.session_map.read().await;
            if let Some(sid) = map.get(session_key) {
                // Bump updated_at in the background (best-effort)
                if let Ok(Some(mut info)) = self.session_mgr.get_session(sid).await {
                    info.updated_at = now;
                    let _ = self.session_mgr.update_session(&info).await;
                }
                return Ok(sid.clone());
            }
        }

        // 2. Search existing sessions by session_key field
        if let Ok(sessions) = self.session_mgr.list_sessions().await {
            if let Some(mut info) = sessions
                .into_iter()
                .find(|s| s.session_key.as_deref() == Some(session_key))
            {
                let sid = info.session_id.clone();
                // Bump updated_at
                info.updated_at = now;
                let _ = self.session_mgr.update_session(&info).await;
                // Cache
                let mut map = self.session_map.write().await;
                map.insert(session_key.to_string(), sid.clone());
                tracing::info!(session_key = %session_key, session_id = %sid, "resolved existing session by key");
                return Ok(sid);
            }
        }

        // 3. Legacy fallback: check bot_sessions namespace mapping
        let store = self.session_mgr.store();
        let ns = &["bot_sessions"];
        let peer_kind = Self::chat_type_to_peer_kind(&msg.chat.chat_type);
        let legacy_sid = self.try_legacy_session(store, ns, session_key).await;
        if let Some(sid) = legacy_sid {
            // Migrate: write session_key into SessionInfo so future lookups use field match
            if let Ok(Some(mut info)) = self.session_mgr.get_session(&sid).await {
                info.session_key = Some(session_key.to_string());
                info.channel = Some(msg.channel.platform.clone());
                info.chat_type = Some(Self::peer_kind_to_chat_type(&peer_kind));
                if info.display_name.is_none() {
                    // For web channel, use the request key (e.g. "main") as display name
                    // instead of the ephemeral connection ID ("conn:UUID").
                    // For bot channels, sender.id is the actual user ID (e.g. Lark open_id).
                    info.display_name = if msg.channel.platform == "web" {
                        Some(crate::session::key::to_request_key(session_key).to_string())
                    } else {
                        msg.sender.id.clone()
                    };
                }
                info.updated_at = now;
                let _ = self.session_mgr.update_session(&info).await;
            }
            let mut map = self.session_map.write().await;
            map.insert(session_key.to_string(), sid.clone());
            tracing::info!(session_key = %session_key, session_id = %sid, "migrated legacy bot_sessions mapping");
            return Ok(sid);
        }

        // 4. Create a new session and populate metadata
        let sid = self
            .session_mgr
            .create_session()
            .await
            .map_err(|e| AgentError(format!("failed to create session: {}", e)))?;

        if let Ok(Some(mut info)) = self.session_mgr.get_session(&sid).await {
            info.session_key = Some(session_key.to_string());
            info.channel = Some(msg.channel.platform.clone());
            info.chat_type = Some(Self::peer_kind_to_chat_type(&peer_kind));
            // For web channel, use the request key (e.g. "main") as display name
            // instead of the ephemeral connection ID ("conn:UUID").
            // For bot channels, sender.id is the actual user ID (e.g. Lark open_id).
            info.display_name = if msg.channel.platform == "web" {
                Some(crate::session::key::to_request_key(session_key).to_string())
            } else {
                msg.sender.id.clone()
            };
            info.updated_at = now;
            let _ = self.session_mgr.update_session(&info).await;
        }

        let mut map = self.session_map.write().await;
        map.insert(session_key.to_string(), sid.clone());
        tracing::info!(session_key = %session_key, session_id = %sid, "created new session");
        Ok(sid)
    }

    /// Try to find a session via legacy bot_sessions namespace mapping.
    pub(super) async fn try_legacy_session(
        &self,
        store: &Arc<dyn synaptic::core::Store>,
        ns: &[&str],
        session_key: &str,
    ) -> Option<String> {
        // Try exact key
        if let Ok(Some(item)) = store.get(ns, session_key).await {
            if let Some(sid) = item.value.as_str() {
                if self
                    .session_mgr
                    .get_session(sid)
                    .await
                    .ok()
                    .flatten()
                    .is_some()
                {
                    return Some(sid.to_string());
                }
            }
        }
        // Try stripping "agent:default:" prefix for old-format keys
        if let Some(legacy_key) = session_key.strip_prefix("agent:default:") {
            if let Ok(Some(item)) = store.get(ns, legacy_key).await {
                if let Some(sid) = item.value.as_str() {
                    if self
                        .session_mgr
                        .get_session(sid)
                        .await
                        .ok()
                        .flatten()
                        .is_some()
                    {
                        return Some(sid.to_string());
                    }
                }
            }
        }
        None
    }

    /// Convert PeerKind to a chat_type string for SessionInfo.
    pub(super) fn peer_kind_to_chat_type(peer_kind: &Option<crate::config::PeerKind>) -> String {
        match peer_kind {
            Some(crate::config::PeerKind::Direct) => "direct".to_string(),
            Some(crate::config::PeerKind::Group) => "group".to_string(),
            Some(crate::config::PeerKind::Channel) => "channel".to_string(),
            None => "unknown".to_string(),
        }
    }

    /// Convert InboundMessage chat_type string to PeerKind.
    pub(super) fn chat_type_to_peer_kind(chat_type: &str) -> Option<crate::config::PeerKind> {
        match chat_type {
            "direct" => Some(crate::config::PeerKind::Direct),
            "group" => Some(crate::config::PeerKind::Group),
            "channel" => Some(crate::config::PeerKind::Channel),
            _ => None,
        }
    }
}
