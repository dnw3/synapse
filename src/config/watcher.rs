use std::path::PathBuf;

use notify::{Event as NotifyEvent, RecursiveMode, Watcher};
use tokio::sync::mpsc;

/// Watches a config file for changes and triggers hot reload with debouncing.
pub struct ConfigWatcher {
    config_path: PathBuf,
}

impl ConfigWatcher {
    pub fn new(config_path: PathBuf) -> Self {
        Self { config_path }
    }

    /// Start watching config file for changes. Calls `on_change` when the file is modified.
    ///
    /// Includes a 500ms debounce to handle rapid successive writes (e.g. editor saves).
    /// The watcher runs until the channel is closed or an error occurs.
    pub async fn watch<F>(&self, on_change: F) -> crate::error::Result<()>
    where
        F: Fn(super::SynapseConfig) + Send + 'static,
    {
        let (tx, mut rx) = mpsc::channel::<()>(1);
        let path = self.config_path.clone();

        std::thread::spawn(move || {
            let tx_clone = tx.clone();
            let mut watcher = notify::recommended_watcher(move |res: Result<NotifyEvent, _>| {
                if let Ok(event) = res {
                    if event.kind.is_modify() || event.kind.is_create() {
                        let _ = tx_clone.blocking_send(());
                    }
                }
            })
            .expect("failed to create file watcher");

            if let Err(e) = watcher.watch(&path, RecursiveMode::NonRecursive) {
                tracing::warn!(
                    error = %e,
                    path = %path.display(),
                    "failed to watch config file"
                );
                return;
            }

            tracing::info!(path = %path.display(), "config file watcher started");

            // Keep the watcher (and thread) alive indefinitely.
            loop {
                std::thread::park();
            }
        });

        while rx.recv().await.is_some() {
            // Debounce: wait 500ms, then drain any queued events.
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            while rx.try_recv().is_ok() {}

            match self.reload_config() {
                Ok(new_config) => {
                    tracing::info!(
                        path = %self.config_path.display(),
                        "config file changed, applying hot reload"
                    );
                    on_change(new_config);
                    // TODO: emit ConfigReloaded event via EventBus
                }
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        path = %self.config_path.display(),
                        "failed to reload config from file"
                    );
                }
            }
        }

        Ok(())
    }

    fn reload_config(&self) -> crate::error::Result<super::SynapseConfig> {
        let content = std::fs::read_to_string(&self.config_path)?;
        let config: super::SynapseConfig = toml::from_str(&content)
            .map_err(|e| crate::error::SynapseError::Config(e.to_string()))?;
        Ok(config)
    }
}

/// Discover the config file path using the same search order as `SynapseConfig::load`.
///
/// Search order:
/// 1. `./synapse.toml`
/// 2. `~/.synapse/config.toml`
pub fn find_config_path() -> Option<PathBuf> {
    let candidates = [
        PathBuf::from("synapse.toml"),
        PathBuf::from("synapse.json"),
        PathBuf::from("synapse.yaml"),
        PathBuf::from("synapse.yml"),
    ];

    for p in &candidates {
        if p.exists() {
            return Some(p.clone());
        }
    }

    if let Some(home) = dirs::home_dir() {
        let home_candidates = [
            home.join(".synapse").join("config.toml"),
            home.join(".synapse").join("config.json"),
            home.join(".synapse").join("config.yaml"),
            home.join(".synapse").join("config.yml"),
        ];
        for p in &home_candidates {
            if p.exists() {
                return Some(p.clone());
            }
        }
    }

    None
}
