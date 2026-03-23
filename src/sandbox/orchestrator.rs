use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;
use synaptic::core::SynapticError;
use synaptic::deep::sandbox::{
    SandboxCreateRequest, SandboxInstance, SandboxInstanceInfo, SandboxProviderRegistry,
    SandboxWorkspace,
};

use super::config::*;
use super::explain::{SandboxExplanation, SandboxSecuritySummary};
use super::registry::{RegistryEntry, SandboxPersistentRegistry};

pub enum ResolvedBackend {
    Host,
    Sandboxed(Arc<SandboxInstance>),
}

pub enum SandboxFilter {
    All,
    BySession(String),
    ByAgent(String),
}

pub struct SandboxOrchestrator {
    registry: Arc<SandboxProviderRegistry>,
    config: SandboxConfig,
    instances: RwLock<HashMap<String, Arc<SandboxInstance>>>,
    persistent: SandboxPersistentRegistry,
}

impl SandboxOrchestrator {
    pub fn new(
        registry: Arc<SandboxProviderRegistry>,
        config: SandboxConfig,
        persistent: SandboxPersistentRegistry,
    ) -> Self {
        Self {
            registry,
            config,
            instances: RwLock::new(HashMap::new()),
            persistent,
        }
    }

    /// Resolve whether a session should be sandboxed.
    pub async fn resolve_backend(
        &self,
        session_key: &str,
        agent_id: &str,
        agent_config: Option<&SandboxConfig>,
    ) -> Result<ResolvedBackend, SynapticError> {
        let effective = self.merge_config(agent_config);

        match effective.mode {
            SandboxMode::Off => return Ok(ResolvedBackend::Host),
            SandboxMode::NonMain => {
                if self.is_main_session(session_key, agent_id) {
                    return Ok(ResolvedBackend::Host);
                }
            }
            SandboxMode::All => {}
        }

        let scope_key = self.compute_scope_key(&effective.scope, session_key, agent_id);
        let instance = self.get_or_create(&scope_key, &effective).await?;
        Ok(ResolvedBackend::Sandboxed(instance))
    }

    fn compute_scope_key(
        &self,
        scope: &SandboxScope,
        session_key: &str,
        agent_id: &str,
    ) -> String {
        match scope {
            SandboxScope::Session => format!("session:{session_key}"),
            SandboxScope::Agent => format!("agent:{agent_id}"),
            SandboxScope::Shared => "shared".to_string(),
        }
    }

    fn is_main_session(&self, session_key: &str, _agent_id: &str) -> bool {
        session_key == "main"
    }

    fn merge_config(&self, agent_config: Option<&SandboxConfig>) -> SandboxConfig {
        match agent_config {
            Some(ac) => ac.clone(),
            None => self.config.clone(),
        }
    }

    async fn get_or_create(
        &self,
        scope_key: &str,
        config: &SandboxConfig,
    ) -> Result<Arc<SandboxInstance>, SynapticError> {
        // Check in-memory pool
        {
            let instances = self.instances.read().await;
            if let Some(instance) = instances.get(scope_key) {
                let _ = self.persistent.touch(&instance.runtime_id);
                return Ok(instance.clone());
            }
        }

        // Create new instance via provider
        let provider = self.registry.get(&config.backend).ok_or_else(|| {
            SynapticError::Tool(format!(
                "sandbox provider '{}' not found",
                config.backend
            ))
        })?;

        let workspace = SandboxWorkspace {
            host_dir: std::env::current_dir().unwrap_or_default(),
            access: config.workspace_access,
        };

        let req = SandboxCreateRequest {
            scope_key: scope_key.to_string(),
            workspace,
            security: config.security.clone().unwrap_or_default(),
            resources: config.resources.clone().unwrap_or_default(),
            extra_mounts: config.mounts.clone(),
            setup_command: None,
            env: HashMap::new(),
        };

        let instance = provider.create(req).await?;
        let arc_instance = Arc::new(instance);

        // Store in pool
        {
            let mut instances = self.instances.write().await;
            instances.insert(scope_key.to_string(), arc_instance.clone());
        }

        // Persist
        let _ = self.persistent.add(RegistryEntry {
            runtime_id: arc_instance.runtime_id.clone(),
            provider_id: config.backend.clone(),
            scope_key: scope_key.to_string(),
            image: arc_instance.info.image.clone(),
            config_hash: String::new(),
            created_at: arc_instance.info.created_at,
            last_used_at: arc_instance.info.last_used_at,
        });

        Ok(arc_instance)
    }

    /// List all sandbox instances across all providers.
    pub async fn list_all(&self) -> Result<Vec<SandboxInstanceInfo>, SynapticError> {
        let mut all = Vec::new();
        for provider_id in self.registry.list_ids() {
            if let Some(provider) = self.registry.get(&provider_id) {
                if let Ok(instances) = provider.list().await {
                    all.extend(instances);
                }
            }
        }
        Ok(all)
    }

    /// Destroy sandbox instances by filter and recreate on next use.
    pub async fn recreate(&self, filter: &SandboxFilter) -> Result<u32, SynapticError> {
        let instances = self.list_all().await?;
        let mut count = 0;

        for info in &instances {
            let matches = match filter {
                SandboxFilter::All => true,
                SandboxFilter::BySession(key) => {
                    info.scope_key == format!("session:{key}")
                }
                SandboxFilter::ByAgent(id) => {
                    info.scope_key.starts_with(&format!("agent:{id}"))
                }
            };

            if matches {
                if let Some(provider) = self.registry.get(&info.provider_id) {
                    let _ = provider.destroy(&info.runtime_id).await;
                    let _ = self.persistent.remove(&info.runtime_id);
                    count += 1;
                }
            }
        }

        // Clear from in-memory pool
        {
            let mut pool = self.instances.write().await;
            pool.retain(|_, inst| {
                !instances.iter().any(|i| i.runtime_id == inst.runtime_id)
            });
        }

        Ok(count)
    }

    /// Explain effective sandbox config for a session/agent pair.
    pub fn explain(&self, session_key: &str, agent_id: &str) -> SandboxExplanation {
        let effective = self.merge_config(None);
        let is_sandboxed = match effective.mode {
            SandboxMode::Off => false,
            SandboxMode::NonMain => !self.is_main_session(session_key, agent_id),
            SandboxMode::All => true,
        };

        SandboxExplanation {
            agent_id: agent_id.to_string(),
            session_key: session_key.to_string(),
            mode: format!("{:?}", effective.mode),
            scope: format!("{:?}", effective.scope),
            workspace_access: format!("{:?}", effective.workspace_access),
            backend: effective.backend.clone(),
            is_sandboxed,
            scope_key: self.compute_scope_key(&effective.scope, session_key, agent_id),
            security: SandboxSecuritySummary::from_config(
                &effective.security.clone().unwrap_or_default(),
            ),
        }
    }

    /// Prune idle/expired instances.
    pub async fn maybe_prune(&self) -> Result<u32, SynapticError> {
        let prunable = self.persistent.prunable(
            self.config.prune.idle_hours,
            self.config.prune.max_age_days,
        )?;

        let mut count = 0;
        for entry in &prunable {
            if let Some(provider) = self.registry.get(&entry.provider_id) {
                let _ = provider.destroy(&entry.runtime_id).await;
                let _ = self.persistent.remove(&entry.runtime_id);
                count += 1;
            }
        }
        Ok(count)
    }

    /// Destroy all instances (for graceful shutdown).
    pub async fn destroy_all(&self) -> Result<(), SynapticError> {
        self.recreate(&SandboxFilter::All).await?;
        Ok(())
    }
}
f.recreate(&SandboxFilter::All).await?;
        Ok(())
    }
}
