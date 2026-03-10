//! Multi-agent routing — configure multiple agents and route by pattern/channel/user
//! with specificity-based scoring.

use std::sync::Arc;

use synaptic::core::ChatModel;

use crate::agent;
use crate::config::{AgentRouteConfig, SynapseConfig};

/// Multi-agent router that dispatches to the appropriate agent based on routing rules.
///
/// Uses specificity scoring to pick the best matching route when multiple routes match.
/// Scoring:
/// - channel exact match: +10
/// - pattern regex match: +5
/// - user match: +20
/// - priority override: replaces computed score
/// - default fallback: score 0
#[allow(dead_code)]
pub struct AgentRouter {
    routes: Vec<(AgentRouteConfig, Arc<dyn ChatModel>)>,
    default_model: Arc<dyn ChatModel>,
}

#[allow(dead_code)]
impl AgentRouter {
    /// Build a router from config.
    pub fn new(
        config: &SynapseConfig,
        default_model: Arc<dyn ChatModel>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let mut routes = Vec::new();

        if let Some(agent_configs) = &config.agent_routes {
            for route in agent_configs {
                let model = if let Some(ref model_name) = route.model {
                    agent::build_model_by_name(config, model_name)?
                } else {
                    default_model.clone()
                };
                routes.push((route.clone(), model));
            }
        }

        Ok(Self {
            routes,
            default_model,
        })
    }

    /// Find the best agent for a message, optionally in a specific channel and from a specific user.
    ///
    /// Returns `(agent_name, model, optional_system_prompt)`.
    pub fn route(
        &self,
        message: &str,
        channel: Option<&str>,
    ) -> (&str, &Arc<dyn ChatModel>, Option<&str>) {
        self.route_with_user(message, channel, None)
    }

    /// Find the best agent with user-aware routing.
    pub fn route_with_user(
        &self,
        message: &str,
        channel: Option<&str>,
        user: Option<&str>,
    ) -> (&str, &Arc<dyn ChatModel>, Option<&str>) {
        let mut best_score: i32 = -1;
        let mut best_idx: Option<usize> = None;

        for (idx, (route, _)) in self.routes.iter().enumerate() {
            let score = self.compute_score(route, message, channel, user);
            if score > best_score {
                best_score = score;
                best_idx = Some(idx);
            }
        }

        if let Some(idx) = best_idx {
            if best_score > 0 {
                let (route, model) = &self.routes[idx];
                return (&route.name, model, route.system_prompt.as_deref());
            }
        }

        // Default agent (score 0 = no match)
        ("default", &self.default_model, None)
    }

    /// Compute the specificity score for a route against a message/channel/user.
    fn compute_score(
        &self,
        route: &AgentRouteConfig,
        message: &str,
        channel: Option<&str>,
        user: Option<&str>,
    ) -> i32 {
        // If the route has a manual priority, use that directly
        if let Some(priority) = route.priority {
            // Still need at least one matching criterion
            let has_channel_match = channel
                .map(|ch| route.channels.contains(&ch.to_string()))
                .unwrap_or(false);
            let has_pattern_match = route.pattern.as_ref().map_or(false, |pattern| {
                regex::Regex::new(pattern)
                    .ok()
                    .map(|re| re.is_match(message))
                    .unwrap_or(false)
            });
            let has_user_match = user
                .map(|u| !route.users.is_empty() && route.users.contains(&u.to_string()))
                .unwrap_or(false);

            if has_channel_match || has_pattern_match || has_user_match
                || (route.channels.is_empty() && route.pattern.is_none() && route.users.is_empty())
            {
                return priority as i32;
            }
            return 0;
        }

        // Compute specificity score
        let mut score: i32 = 0;

        // Channel match: +10
        if let Some(ch) = channel {
            if route.channels.contains(&ch.to_string()) {
                score += 10;
            } else if !route.channels.is_empty() {
                // Route requires specific channels but doesn't match
                return 0;
            }
        } else if !route.channels.is_empty() {
            // Route requires channels but no channel provided
            return 0;
        }

        // User match: +20
        if let Some(u) = user {
            if !route.users.is_empty() {
                if route.users.contains(&u.to_string()) {
                    score += 20;
                } else {
                    // Route requires specific users but doesn't match
                    return 0;
                }
            }
        } else if !route.users.is_empty() {
            // Route requires users but no user provided
            return 0;
        }

        // Pattern match: +5
        if let Some(ref pattern) = route.pattern {
            if let Ok(re) = regex::Regex::new(pattern) {
                if re.is_match(message) {
                    score += 5;
                } else {
                    // Route requires pattern but doesn't match
                    return 0;
                }
            }
        }

        // A route with no criteria is a catch-all with score 1
        if route.channels.is_empty() && route.pattern.is_none() && route.users.is_empty() {
            score = 1;
        }

        score
    }

    /// List all configured routes.
    pub fn routes(&self) -> &[(AgentRouteConfig, Arc<dyn ChatModel>)] {
        &self.routes
    }
}
