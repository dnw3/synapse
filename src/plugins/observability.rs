//! builtin-observability plugin — registers agent tracing, thinking,
//! loop detection, and cost tracking subscribers into the EventBus.

use std::sync::Arc;

use async_trait::async_trait;
use synaptic::callbacks::CostTrackingCallback;
use synaptic::core::SynapticError;
use synaptic::plugin::{PluginApi, PluginCapability, PluginManifest};

use crate::agent::subscribers::{
    CostTrackingSubscriber, LoopDetectionSubscriber, ThinkingSubscriber, TracingSubscriber,
};
use crate::gateway::usage::UsageTracker;

/// Built-in plugin that registers all observability EventSubscribers.
pub struct ObservabilityPlugin {
    cost_tracker: Arc<CostTrackingCallback>,
    usage_tracker: Arc<UsageTracker>,
}

impl ObservabilityPlugin {
    pub fn new(cost_tracker: Arc<CostTrackingCallback>, usage_tracker: Arc<UsageTracker>) -> Self {
        Self {
            cost_tracker,
            usage_tracker,
        }
    }
}

#[async_trait]
impl synaptic::plugin::Plugin for ObservabilityPlugin {
    fn manifest(&self) -> PluginManifest {
        PluginManifest {
            name: "builtin-observability".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            description: "Agent tracing, thinking, loop detection, and cost tracking".into(),
            author: Some("synapse".into()),
            license: None,
            capabilities: vec![PluginCapability::Hooks],
            slot: None,
        }
    }

    async fn register(&self, api: &mut PluginApi<'_>) -> Result<(), SynapticError> {
        api.register_event_subscriber(Arc::new(TracingSubscriber::new()), -80);
        api.register_event_subscriber(Arc::new(ThinkingSubscriber::new(None)), -70);
        api.register_event_subscriber(Arc::new(LoopDetectionSubscriber::new(3)), -85);
        api.register_event_subscriber(
            Arc::new(CostTrackingSubscriber::new(
                self.cost_tracker.clone(),
                self.usage_tracker.clone(),
            )),
            -60,
        );
        Ok(())
    }
}
